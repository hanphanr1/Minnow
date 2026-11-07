//! Workspace inspection, editing, search, and Git actions for a worker worktree.
//!
//! Every entry point resolves caller-supplied paths against the canonical
//! workspace root before touching the file system. Absolute paths, drive and
//! UNC prefixes, alternate data streams, `..` components, symlink escapes, and
//! Git/Minnow metadata are rejected. Git work goes through the bounded runner in
//! [`crate::git`], so there is one process-execution convention with
//! `CREATE_NO_WINDOW` and the 2 MiB output cap.

use crate::git;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

#[path = "workspace_search.rs"]
mod search_impl;

const MAX_LIST_ENTRIES: usize = 1000;
const MAX_TEXT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SEARCH_HITS: usize = 500;
const MAX_SEARCH_CANDIDATES: usize = 20_000;
const MAX_SEARCH_BYTES: u64 = 32 * 1024 * 1024;
const MAX_SEARCH_LINE_CHARS: usize = 400;
const DEFAULT_HISTORY_LIMIT: usize = 30;
const MAX_HISTORY_LIMIT: usize = 100;

/// Serializes revision-checked saves and Git mutations originating in the app.
static MUTATION_LOCK: Mutex<()> = Mutex::new(());

fn mutation_guard() -> MutexGuard<'static, ()> {
    MUTATION_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/* ------------------------------------------------------------------------- */
/* serialized results                                                        */
/* ------------------------------------------------------------------------- */

#[derive(Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
}

#[derive(Serialize)]
pub struct DirectoryListing {
    pub entries: Vec<DirectoryEntry>,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct FileDocument {
    pub path: String,
    pub content: String,
    pub revision: String,
}

#[derive(Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub text: String,
}

#[derive(Serialize)]
pub struct SearchResults {
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct FileList {
    pub paths: Vec<String>,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct GitFileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
}

#[derive(Serialize)]
pub struct GitStatus {
    pub branch: String,
    pub staged: Vec<GitFileChange>,
    pub unstaged: Vec<GitFileChange>,
    pub untracked: Vec<GitFileChange>,
}

#[derive(Serialize)]
pub struct GitCommit {
    pub id: String,
    pub short_id: String,
    pub subject: String,
    pub author: String,
    pub timestamp: i64,
}

/* ------------------------------------------------------------------------- */
/* path resolution                                                           */
/* ------------------------------------------------------------------------- */

struct ResolvedPath {
    relative: String,
    full: PathBuf,
}

fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    git::canonical_existing_dir(root, "workspace root")
}

fn canonical_child(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path)
        .map(|canonical| git::display_path(&canonical))
        .map_err(|error| format!("{} is not accessible: {error}", path.display()))
}

fn inside_workspace(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else { return false; };
    !relative.components().any(|component| match component {
        Component::Normal(name) => is_metadata_name(&name.to_string_lossy()),
        _ => false,
    })
}

fn relative_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(windows)]
fn is_reserved_windows_name(lower: &str) -> bool {
    let stem = lower.split('.').next().unwrap_or(lower).trim_end_matches(' ');
    if matches!(stem, "con" | "prn" | "aux" | "nul" | "conin$" | "conout$") {
        return true;
    }
    let digits = stem
        .strip_prefix("com")
        .or_else(|| stem.strip_prefix("lpt"));
    match digits {
        Some(rest) => {
            let mut chars = rest.chars();
            matches!(chars.next(), Some(digit) if ('1'..='9').contains(&digit))
                && chars.next().is_none()
        }
        None => false,
    }
}

/// Validates a caller-supplied workspace-relative path and returns its
/// components. Both separators are accepted; the result never contains `.`
/// or `..` components.
fn path_segments(raw: &str) -> Result<Vec<String>, String> {
    if raw.contains('\0') {
        return Err("a workspace path cannot contain a NUL byte".to_string());
    }
    if raw.starts_with('/') || raw.starts_with('\\') {
        return Err("absolute and UNC paths are not allowed in the workspace".to_string());
    }
    if raw.len() >= 2 && raw.as_bytes()[1] == b':' {
        return Err("drive-qualified paths are not allowed in the workspace".to_string());
    }
    let mut segments = Vec::new();
    for part in raw.split(|character| character == '/' || character == '\\') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err("parent directory components are not allowed in the workspace".to_string());
        }
        if is_metadata_name(part) {
            return Err("Git and Minnow metadata are not exposed through the workspace".to_string());
        }
        if part.len() > 255 {
            return Err("a workspace path component is too long".to_string());
        }
        #[cfg(windows)]
        {
            if part.contains(':') {
                return Err("alternate data streams are not allowed in the workspace".to_string());
            }
            if part
                .chars()
                .any(|character| character.is_control() || matches!(character, '<' | '>' | '"' | '|' | '?' | '*'))
            {
                return Err("the path contains characters Windows does not allow".to_string());
            }
            if part.ends_with(' ') || part.ends_with('.') {
                return Err("a Windows path component cannot end with a space or a dot".to_string());
            }
            if is_reserved_windows_name(&part.to_ascii_lowercase()) {
                return Err("reserved Windows device names are not allowed in the workspace".to_string());
            }
        }
        segments.push(part.to_string());
    }
    Ok(segments)
}

fn is_metadata_name(name: &str) -> bool {
    name.eq_ignore_ascii_case(".git") || name.eq_ignore_ascii_case(".minnow")
        || name.get(..13).is_some_and(|prefix| prefix.eq_ignore_ascii_case(".minnow-save-"))
}

fn joined(root: &Path, segments: &[String]) -> PathBuf {
    let mut full = root.to_path_buf();
    for segment in segments {
        full.push(segment);
    }
    full
}

fn resolve_existing(root: &Path, raw: &str) -> Result<ResolvedPath, String> {
    let segments = path_segments(raw)?;
    let full = joined(root, &segments);
    let canonical = canonical_child(&full)?;
    if !inside_workspace(root, &canonical) {
        return Err("the path resolves outside the workspace".to_string());
    }
    Ok(ResolvedPath {
        relative: segments.join("/"),
        full: canonical,
    })
}

/// Resolves a path that may not exist yet (new files, deleted diffs).
///
/// The nearest existing ancestor is canonicalized, so a link anywhere on the
/// way out of the workspace is refused. A symlink leaf must resolve to a target
/// inside the workspace; a dangling link is refused rather than written through.
fn resolve_mutable(root: &Path, raw: &str) -> Result<ResolvedPath, String> {
    let segments = path_segments(raw)?;
    if segments.is_empty() {
        return Err("a file path is required".to_string());
    }
    let relative = segments.join("/");
    let full = joined(root, &segments);
    match fs::symlink_metadata(&full) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                let canonical = canonical_child(&full)
                    .map_err(|_| format!("{relative} is a link whose target is not accessible"))?;
                if !inside_workspace(root, &canonical) {
                    return Err(format!("{relative} is a link that points outside the workspace"));
                }
                return Ok(ResolvedPath { relative, full: canonical });
            }
            let canonical = canonical_child(&full)?;
            if !inside_workspace(root, &canonical) {
                return Err(format!("{relative} resolves outside the workspace"));
            }
            Ok(ResolvedPath { relative, full: canonical })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut ancestor = full.parent();
            while let Some(candidate) = ancestor {
                if candidate.exists() {
                    let canonical = canonical_child(candidate)?;
                    if !inside_workspace(root, &canonical) {
                        return Err(format!("{relative} resolves outside the workspace"));
                    }
                    return Ok(ResolvedPath { relative, full });
                }
                ancestor = candidate.parent();
            }
            Err(format!("{relative} cannot be resolved inside the workspace"))
        }
        Err(error) => Err(format!("{relative} is not accessible: {error}")),
    }
}

/* ------------------------------------------------------------------------- */
/* text helpers                                                              */
/* ------------------------------------------------------------------------- */

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut encoded = String::with_capacity(64);
    let digest = hasher.finalize();
    for byte in digest.iter().copied() {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn decode_text(bytes: Vec<u8>, label: &str) -> Result<String, String> {
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        return Err(format!("{label} exceeds the 2 MiB text limit"));
    }
    if bytes.contains(&0) {
        return Err(format!("{label} is a binary file"));
    }
    String::from_utf8(bytes).map_err(|_| format!("{label} is not valid UTF-8 text"))
}

fn clip_line(line: &str) -> String {
    let mut clipped: String = line.chars().take(MAX_SEARCH_LINE_CHARS).collect();
    if line.chars().nth(MAX_SEARCH_LINE_CHARS).is_some() {
        clipped.push('…');
    }
    clipped
}

fn read_bounded(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|error| format!("cannot read {label}: {error}"))?;
    let metadata = file.metadata().map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{label} is not a regular file"));
    }
    if metadata.len() > MAX_TEXT_BYTES {
        return Err(format!("{label} exceeds the 2 MiB text limit"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_TEXT_BYTES + 1).read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {label}: {error}"))?;
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        return Err(format!("{label} exceeds the 2 MiB text limit"));
    }
    Ok(bytes)
}

/* ------------------------------------------------------------------------- */
/* directory listing                                                         */
/* ------------------------------------------------------------------------- */

/// Lists one directory level, sorted with directories first and capped at 1000
/// entries. Git and Minnow metadata are never listed.
pub fn list_directory(root: &Path, path: &str) -> Result<DirectoryListing, String> {
    let root = canonical_root(root)?;
    let resolved = resolve_existing(&root, path)?;
    let metadata = fs::metadata(&resolved.full)
        .map_err(|error| format!("{} is not accessible: {error}", resolved.relative))?;
    if !metadata.is_dir() {
        return Err(if resolved.relative.is_empty() {
            "the workspace root is not a directory".to_string()
        } else {
            format!("{} is not a directory", resolved.relative)
        });
    }
    let read_dir = fs::read_dir(&resolved.full)
        .map_err(|error| format!("cannot list {}: {error}", resolved.relative))?;

    let mut entries = Vec::new();
    let mut truncated = false;
    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_metadata_name(&name) {
            continue;
        }
        if entries.len() == MAX_LIST_ENTRIES {
            truncated = true;
            break;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        let is_symlink = file_type.is_symlink();
        let mut is_dir = file_type.is_dir();
        let mut size = 0;
        if is_symlink {
            // Only report a target type or size when the link stays inside the
            // workspace, so listing never probes outside it.
            let target_inside = fs::canonicalize(entry.path())
                .map(|canonical| inside_workspace(&root, &git::display_path(&canonical)))
                .unwrap_or(false);
            if target_inside {
                if let Ok(target) = fs::metadata(entry.path()) {
                    is_dir = target.is_dir();
                    if target.is_file() {
                        size = target.len();
                    }
                }
            } else {
                is_dir = false;
            }
        } else {
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.is_file() {
                size = metadata.len();
            }
        }
        let child = if resolved.relative.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", resolved.relative, name)
        };
        entries.push(DirectoryEntry {
            name,
            path: child,
            is_dir,
            is_symlink,
            size,
        });
    }

    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase(), entry.name.clone()));
    Ok(DirectoryListing { entries, truncated })
}

/* ------------------------------------------------------------------------- */
/* files                                                                     */
/* ------------------------------------------------------------------------- */

/// Reads a UTF-8 text file up to 2 MiB and returns its SHA-256 revision.
pub fn read_file(root: &Path, path: &str) -> Result<FileDocument, String> {
    let root = canonical_root(root)?;
    let resolved = resolve_existing(&root, path)?;
    if resolved.relative.is_empty() {
        return Err("a file path is required".to_string());
    }
    let metadata = fs::metadata(&resolved.full)
        .map_err(|error| format!("{} is not accessible: {error}", resolved.relative))?;
    if metadata.is_dir() {
        return Err(format!("{} is a directory, not a file", resolved.relative));
    }
    if metadata.len() > MAX_TEXT_BYTES {
        return Err(format!("{} exceeds the 2 MiB text limit", resolved.relative));
    }
    let bytes = read_bounded(&resolved.full, &resolved.relative)?;
    let revision = sha256_hex(&bytes);
    let content = decode_text(bytes, &resolved.relative)?;
    Ok(FileDocument {
        path: resolved.relative,
        content,
        revision,
    })
}

/// Saves UTF-8 text with optimistic concurrency: `revision` must be the SHA-256
/// the caller read, or empty only when the file does not exist yet. The check
/// and the write are serialized in-process so two saves cannot interleave.
pub fn write_file(root: &Path, path: &str, content: &str, revision: &str) -> Result<FileDocument, String> {
    if content.len() as u64 > MAX_TEXT_BYTES {
        return Err("the new content exceeds the 2 MiB text limit".to_string());
    }
    if content.as_bytes().contains(&0) {
        return Err("binary content cannot be written through the workspace".to_string());
    }
    let root = canonical_root(root)?;
    let resolved = resolve_mutable(&root, path)?;
    let bytes = content.as_bytes();

    let _guard = mutation_guard();
    let permissions = match fs::symlink_metadata(&resolved.full) {
        Ok(metadata) => {
            if metadata.is_dir() {
                return Err(format!("{} is a directory, not a file", resolved.relative));
            }
            if metadata.len() > MAX_TEXT_BYTES {
                return Err(format!("{} exceeds the 2 MiB text limit", resolved.relative));
            }
            let current = read_bounded(&resolved.full, &resolved.relative)?;
            if !revision.eq_ignore_ascii_case(&sha256_hex(&current)) {
                return Err(format!(
                    "{} changed on disk since it was read; reopen it before saving",
                    resolved.relative
                ));
            }
            if metadata.permissions().readonly() {
                return Err(format!("{} is read-only", resolved.relative));
            }
            Some(metadata.permissions())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !revision.is_empty() {
                return Err(format!(
                    "{} no longer exists on disk; reopen it before saving",
                    resolved.relative
                ));
            }
            None
        }
        Err(error) => return Err(format!("cannot inspect {}: {error}", resolved.relative)),
    };

    let parent = resolved.full.parent().ok_or("a file parent directory is required")?;
    let mut temporary = tempfile::Builder::new().prefix(".minnow-save-").tempfile_in(parent)
        .map_err(|error| format!("cannot prepare {} for saving: {error}", resolved.relative))?;
    temporary.write_all(bytes).and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("cannot write {}: {error}", resolved.relative))?;
    if let Some(permissions) = permissions {
        temporary.as_file().set_permissions(permissions)
            .map_err(|error| format!("cannot retain file permissions: {error}"))?;
    }
    if revision.is_empty() {
        temporary.persist_noclobber(&resolved.full)
            .map_err(|error| format!("cannot create {} without overwriting another file: {error}", resolved.relative))?;
    } else {
        let latest_path = canonical_child(&resolved.full)?;
        if latest_path != resolved.full || !inside_workspace(&root, &latest_path)
            || !fs::symlink_metadata(&latest_path).map_err(|error| error.to_string())?.is_file()
            || !revision.eq_ignore_ascii_case(&sha256_hex(&read_bounded(&latest_path, &resolved.relative)?))
        {
            return Err(format!("{} changed while preparing the save; reload before saving", resolved.relative));
        }
        temporary.persist(&resolved.full)
            .map_err(|error| format!("cannot replace {} safely: {error}", resolved.relative))?;
    }
    Ok(FileDocument {
        path: resolved.relative,
        content: content.to_string(),
        revision: sha256_hex(bytes),
    })
}

/* ------------------------------------------------------------------------- */
/* file operations                                                           */
/* ------------------------------------------------------------------------- */

/// Creates an empty file or a folder at `path`. The parent folder must already
/// exist and nothing may be there: creating never overwrites.
pub fn create_entry(root: &Path, path: &str, is_dir: bool) -> Result<String, String> {
    let root = canonical_root(root)?;
    let resolved = resolve_mutable(&root, path)?;
    let parent = resolved.full.parent().ok_or("a parent folder is required")?;
    if !parent.is_dir() {
        return Err(format!("the folder for {} does not exist", resolved.relative));
    }
    let _guard = mutation_guard();
    if is_dir {
        fs::create_dir(&resolved.full)
    } else {
        fs::OpenOptions::new().write(true).create_new(true).open(&resolved.full).map(|_| ())
    }
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            format!("{} already exists", resolved.relative)
        } else {
            format!("cannot create {}: {error}", resolved.relative)
        }
    })?;
    Ok(resolved.relative)
}

/// Renames or moves a file or folder inside the workspace. The destination must
/// not exist (a case-only rename of the same entry is allowed), a folder cannot
/// move into itself, and the workspace root cannot be renamed.
pub fn rename_entry(root: &Path, from: &str, to: &str) -> Result<String, String> {
    let root = canonical_root(root)?;
    let source = resolve_existing(&root, from)?;
    if source.relative.is_empty() {
        return Err("the workspace root cannot be renamed".to_string());
    }
    let target = resolve_mutable(&root, to)?;
    if target.relative == source.relative {
        return Ok(target.relative);
    }
    let same_entry_case_only = target.relative.eq_ignore_ascii_case(&source.relative)
        && target.full.exists()
        && fs::canonicalize(&target.full).ok() == fs::canonicalize(&source.full).ok();
    if target.full.exists() && !same_entry_case_only {
        return Err(format!("{} already exists", target.relative));
    }
    if source.full.is_dir()
        && target.relative.to_lowercase().starts_with(&format!("{}/", source.relative.to_lowercase()))
    {
        return Err("a folder cannot be moved into itself".to_string());
    }
    let parent = target.full.parent().ok_or("a parent folder is required")?;
    if !parent.is_dir() {
        return Err(format!("the folder for {} does not exist", target.relative));
    }
    let _guard = mutation_guard();
    fs::rename(&source.full, &target.full)
        .map_err(|error| format!("cannot rename {} to {}: {error}", source.relative, target.relative))?;
    Ok(target.relative)
}

/// Moves a file or folder to the system trash, so a delete can be undone from
/// there. A symlink is trashed itself, never its target.
pub fn delete_entry(root: &Path, path: &str) -> Result<(), String> {
    let root = canonical_root(root)?;
    let segments = path_segments(path)?;
    if segments.is_empty() {
        return Err("the workspace root cannot be deleted".to_string());
    }
    let relative = segments.join("/");
    let full = joined(&root, &segments);
    let metadata = fs::symlink_metadata(&full)
        .map_err(|error| format!("{relative} is not accessible: {error}"))?;
    if !metadata.file_type().is_symlink() {
        let canonical = canonical_child(&full)?;
        if !inside_workspace(&root, &canonical) {
            return Err(format!("{relative} resolves outside the workspace"));
        }
    } else if let Some(parent) = full.parent() {
        // A link leaf is removed as itself, but its folder must still be inside.
        let canonical = canonical_child(parent)?;
        if canonical != root && !inside_workspace(&root, &canonical) {
            return Err(format!("{relative} is outside the workspace"));
        }
    }
    let _guard = mutation_guard();
    trash::delete(&full).map_err(|error| format!("cannot move {relative} to the trash: {error}"))
}

/// Shows a workspace entry in the system file manager. Returns once the
/// manager was started; it is never waited for.
pub fn reveal_entry(root: &Path, path: &str) -> Result<(), String> {
    let root = canonical_root(root)?;
    let resolved = if path_segments(path)?.is_empty() {
        ResolvedPath { relative: String::new(), full: root.clone() }
    } else {
        resolve_existing(&root, path)?
    };
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("explorer.exe");
        if resolved.relative.is_empty() {
            command.arg(&resolved.full);
        } else {
            // `/select,` and the path are one argument; explorer rejects them split.
            command.arg(format!("/select,{}", resolved.full.display()));
        }
        command.creation_flags(0x08000000);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(if resolved.full.is_dir() { resolved.full.clone() } else { resolved.full.parent().unwrap_or(&root).to_path_buf() });
        command
    };
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot open the file manager: {error}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/* ------------------------------------------------------------------------- */
/* search                                                                    */
/* ------------------------------------------------------------------------- */

/// Literal text search under Git ignore rules. Never follows links, skips
/// `.git`/`.minnow`, bounds candidates to 20 000 files, files to 2 MiB, and
/// results to 500 matches. `truncated` reports every bound that cut results.
pub fn search(root: &Path, query: &str, case_sensitive: bool) -> Result<SearchResults, String> {
    search_impl::search(root, query, case_sensitive)
}

pub fn find_files(root: &Path, query: &str, include_dotfiles: bool) -> Result<FileList, String> {
    search_impl::find_files(root, query, include_dotfiles)
}

/* ------------------------------------------------------------------------- */
/* git status and history                                                    */
/* ------------------------------------------------------------------------- */

fn current_branch(root: &Path) -> Result<String, String> {
    let named = git::git_at(Some(root), &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if named.ok {
        let branch = named.stdout.trim();
        if !branch.is_empty() {
            return Ok(branch.to_string());
        }
    }
    let detached = git::git_at(Some(root), &["rev-parse", "--short", "HEAD"])?;
    if detached.ok {
        let id = detached.stdout.trim();
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }
    Ok("HEAD".to_string())
}

fn git_change(path: &str, old_path: Option<&str>, status: char) -> GitFileChange {
    GitFileChange {
        path: path.to_string(),
        old_path: old_path.map(str::to_string),
        status: status.to_string(),
    }
}

/// Parses `git status --porcelain -z`. Rename and copy entries carry the
/// destination in the entry token and the source in the following token; that
/// order is preserved as `path`/`old_path`.
fn parse_porcelain_status(
    text: &str,
) -> Result<(Vec<GitFileChange>, Vec<GitFileChange>, Vec<GitFileChange>), String> {
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    let mut tokens = text.split('\0');

    while let Some(token) = tokens.next() {
        if token.is_empty() {
            continue;
        }
        let bytes = token.as_bytes();
        if bytes.len() < 4 || bytes[2] != b' ' {
            return Err("git returned unexpected working tree status output".to_string());
        }
        let index_status = bytes[0] as char;
        let worktree_status = bytes[1] as char;
        let path = &token[3..];
        if path.is_empty() {
            return Err("git returned a status entry without a path".to_string());
        }

        let mut old_path = None;
        let rename_in_index = matches!(index_status, 'R' | 'C');
        let rename_in_worktree = matches!(worktree_status, 'R' | 'C');
        if rename_in_index || rename_in_worktree {
            match tokens.next() {
                Some(source) if !source.is_empty() => old_path = Some(source),
                _ => {
                    return Err("git returned a rename entry without its original path".to_string())
                }
            }
        }

        if index_status == '?' {
            untracked.push(git_change(path, None, '?'));
            continue;
        }
        if index_status == '!' {
            continue;
        }

        let unmerged = matches!(
            (index_status, worktree_status),
            ('D', 'D')
                | ('A', 'U')
                | ('U', 'D')
                | ('U', 'A')
                | ('D', 'U')
                | ('A', 'A')
                | ('U', 'U')
        );
        if unmerged {
            // Conflicts are surfaced once, in the working tree bucket, with a
            // distinct status so the UI can offer resolution instead of a
            // normal stage/unstage action.
            unstaged.push(git_change(path, old_path.as_deref(), 'U'));
            continue;
        }
        if index_status != ' ' {
            let source = if rename_in_index { old_path.as_deref() } else { None };
            staged.push(git_change(path, source, index_status));
        }
        if worktree_status != ' ' {
            let source = if rename_in_worktree { old_path.as_deref() } else { None };
            unstaged.push(git_change(path, source, worktree_status));
        }
    }

    Ok((staged, unstaged, untracked))
}

fn read_status(root: &Path) -> Result<GitStatus, String> {
    let branch = current_branch(root)?;
    let text = git::git_ok(
        Some(root),
        &["--no-optional-locks", "status", "--porcelain", "-z", "--untracked-files=all"],
        "cannot inspect the working tree status",
    )?;
    let (staged, unstaged, untracked) = parse_porcelain_status(&text)?;
    Ok(GitStatus {
        branch,
        staged,
        unstaged,
        untracked,
    })
}

/// The subset of `paths` (workspace-relative, forward slashes) that Git ignores.
/// Paths are literal; the answer reflects `.gitignore`, `info/exclude` and the
/// global excludes file exactly as Git applies them. A non-Git folder has no
/// ignore rules, so nothing is reported.
pub fn git_ignored_paths(root: &Path, paths: &[String]) -> Result<Vec<String>, String> {
    const MAX_PATHS: usize = 5000;
    const CHUNK: usize = 200;
    if paths.len() > MAX_PATHS {
        return Err(format!("cannot check more than {MAX_PATHS} paths for ignore rules at once"));
    }
    let root = canonical_root(root)?;
    let mut cleaned = Vec::with_capacity(paths.len());
    for raw in paths {
        cleaned.push(path_segments(raw)?.join("/"));
    }
    let mut ignored = Vec::new();
    for chunk in cleaned.chunks(CHUNK) {
        let mut args: Vec<&str> = vec!["check-ignore", "--"];
        args.extend(chunk.iter().map(String::as_str));
        let output = git::git_at(Some(root.as_path()), &args)?;
        match output.code {
            // 0: at least one path is ignored; 1: none is.
            // Git quotes names with quotes or backslashes; those cannot be matched back
            // to a request path, so only exact echoes of requested paths are reported.
            Some(0) => ignored.extend(
                output.stdout.lines().filter(|line| chunk.iter().any(|path| path == line)).map(str::to_owned),
            ),
            Some(1) => {}
            // 128 outside a repository: there are no ignore rules to apply.
            Some(128) if output.stderr.contains("not a git repository") => return Ok(Vec::new()),
            _ => {
                return Err(format!(
                    "cannot inspect ignore rules: {}",
                    output.stderr.trim()
                ))
            }
        }
    }
    Ok(ignored)
}

pub fn git_status(root: &Path) -> Result<GitStatus, String> {
    let root = canonical_root(root)?;
    read_status(&root)
}

/// Real commit history, bounded to 1–100 entries (30 when the caller passes 0).
pub fn git_history(root: &Path, limit: usize) -> Result<Vec<GitCommit>, String> {
    let root = canonical_root(root)?;
    let limit = if limit == 0 {
        DEFAULT_HISTORY_LIMIT
    } else {
        limit.min(MAX_HISTORY_LIMIT)
    };
    let head = git::git_at(Some(root.as_path()), &["rev-parse", "--verify", "--quiet", "HEAD"])?;
    if !head.ok {
        if head.code == Some(1) && head.stderr.trim().is_empty() {
            // Unborn HEAD: the repository is real but has no commits yet.
            return Ok(Vec::new());
        }
        let detail = head.stderr.trim();
        return Err(if detail.is_empty() {
            "cannot read the commit history".to_string()
        } else {
            format!("cannot read the commit history: {detail}")
        });
    }
    let count = format!("-n{limit}");
    let text = git::git_ok(
        Some(root.as_path()),
        &[
            "log",
            "--no-color",
            "--no-decorate",
            "--no-show-signature",
            "-z",
            count.as_str(),
            "--format=%H%x00%h%x00%s%x00%an%x00%at",
        ],
        "cannot read the commit history",
    )?;

    let mut commits = Vec::new();
    let mut fields = text.split('\0');
    while let Some(id) = fields.next() {
        if id.is_empty() { continue; }
        let short_id = fields.next().ok_or("Git history has an incomplete commit ID")?;
        let subject = fields.next().ok_or("Git history has an incomplete subject")?;
        let author = fields.next().ok_or("Git history has an incomplete author")?;
        let timestamp = fields.next().ok_or("Git history has an incomplete date")?.trim();
        commits.push(GitCommit {
            id: id.to_string(),
            short_id: short_id.to_string(),
            subject: subject.to_string(),
            author: author.to_string(),
            timestamp: timestamp.parse::<i64>().map_err(|_| "Git history contains an invalid timestamp")?,
        });
    }
    Ok(commits)
}

/* ------------------------------------------------------------------------- */
/* git staging and commits                                                   */
/* ------------------------------------------------------------------------- */

/// Validates caller paths as literal repository paths. `None` selects every
/// path; `Some([])` is a no-op rather than staging everything.
fn literal_paths(paths: &[String]) -> Result<Vec<String>, String> {
    let mut literal = Vec::new();
    for raw in paths {
        let segments = path_segments(raw)?;
        if segments.is_empty() {
            return Err("an empty path cannot be staged or unstaged".to_string());
        }
        literal.push(segments.join("/"));
    }
    Ok(literal)
}

fn run_git(root: &Path, args: &[String], action: &str) -> Result<(), String> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    git::git_ok(Some(root), &refs, action)?;
    Ok(())
}

/// Adds the other side of a detected rename to a literal selection so staging
/// or unstaging one side keeps the rename together.
fn expand_renames(root: &Path, literal: &[String], staged: bool) -> Result<Vec<String>, String> {
    let status = read_status(root)?;
    let mut expanded = literal.to_vec();
    let changes = if staged { &status.staged } else { &status.unstaged };
    for change in changes {
        let Some(old_path) = change.old_path.as_deref() else {
            continue;
        };
        let old_literal = match path_segments(old_path) {
            Ok(segments) if !segments.is_empty() => segments.join("/"),
            _ => continue,
        };
        let selected_new = literal.iter().any(|selected| selected == &change.path);
        let selected_old = literal.iter().any(|selected| selected == &old_literal);
        if selected_new && !expanded.contains(&old_literal) {
            expanded.push(old_literal);
        } else if selected_old && !expanded.contains(&change.path) {
            expanded.push(change.path.clone());
        }
    }
    Ok(expanded)
}

pub fn stage_paths(root: &Path, paths: Option<Vec<String>>) -> Result<(), String> {
    let root = canonical_root(root)?;
    let _guard = mutation_guard();
    let mut args: Vec<String> = vec!["--literal-pathspecs".into(), "add".into()];
    match paths {
        None => args.push("--all".into()),
        Some(paths) => {
            let literal = expand_renames(&root, &literal_paths(&paths)?, false)?;
            if literal.is_empty() {
                return Ok(());
            }
            args.push("--".into());
            args.extend(literal);
        }
    }
    run_git(&root, &args, "cannot stage the selected paths")
}

pub fn unstage_paths(root: &Path, paths: Option<Vec<String>>) -> Result<(), String> {
    let root = canonical_root(root)?;
    let _guard = mutation_guard();
    let head = git::git_at(Some(root.as_path()), &["rev-parse", "--verify", "--quiet", "HEAD"])?;
    let has_head = head.ok;
    let args: Vec<String> = match paths {
        None => {
            if has_head {
                // Pathspec form keeps an in-progress merge and the worktree intact.
                vec!["reset".into(), "--quiet".into(), "HEAD".into(), "--".into(), ".".into()]
            } else {
                vec!["read-tree".into(), "--empty".into()]
            }
        }
        Some(paths) => {
            let literal = expand_renames(&root, &literal_paths(&paths)?, true)?;
            if literal.is_empty() {
                return Ok(());
            }
            let mut args: Vec<String> = if has_head {
                vec![
                    "--literal-pathspecs".into(),
                    "reset".into(),
                    "--quiet".into(),
                    "HEAD".into(),
                    "--".into(),
                ]
            } else {
                vec![
                    "--literal-pathspecs".into(),
                    "rm".into(),
                    "--cached".into(),
                    "-r".into(),
                    "--quiet".into(),
                    "--ignore-unmatch".into(),
                    "--".into(),
                ]
            };
            args.extend(literal);
            args
        }
    };
    run_git(&root, &args, "cannot unstage the selected paths")
}

/// Throws away working-tree changes. `tracked` paths are restored from the
/// index (staged work is kept); `untracked` paths are moved to the system trash,
/// so even an untracked file can be recovered from there.
pub fn discard_paths(root: &Path, tracked: &[String], untracked: &[String]) -> Result<(), String> {
    let root = canonical_root(root)?;
    let tracked = literal_paths(tracked)?;
    let untracked = literal_paths(untracked)?;
    if !tracked.is_empty() {
        let _guard = mutation_guard();
        let mut args: Vec<String> = vec!["--literal-pathspecs".into(), "restore".into(), "--worktree".into(), "--".into()];
        args.extend(tracked);
        run_git(&root, &args, "cannot discard the selected changes")?;
    }
    for path in &untracked {
        delete_entry(&root, path)?;
    }
    Ok(())
}

/// Creates a commit with the caller's message, respecting repository identity,
/// hooks, and signing configuration. Returns the new full commit hash.
pub fn commit(root: &Path, message: &str) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("a commit message is required".to_string());
    }
    if message.contains('\0') {
        return Err("a commit message cannot contain a NUL byte".to_string());
    }
    let root = canonical_root(root)?;
    let _guard = mutation_guard();
    let status = read_status(&root)?;
    if status.unstaged.iter().any(|change| change.status == "U") {
        return Err("cannot commit while paths are unmerged; resolve the conflicts first".to_string());
    }
    if status.staged.is_empty() {
        return Err("there are no staged changes to commit".to_string());
    }
    git::git_ok(Some(root.as_path()), &["commit", "-m", message], "cannot create the commit")?;
    let id = git::git_ok(Some(root.as_path()), &["rev-parse", "HEAD"], "cannot read the new commit")?;
    let id = id.trim();
    if id.is_empty() {
        return Err("git did not report the new commit".to_string());
    }
    Ok(id.to_string())
}

/* ------------------------------------------------------------------------- */
/* file diffs                                                                */
/* ------------------------------------------------------------------------- */

fn is_tracked(root: &Path, relative: &str) -> Result<bool, String> {
    let output = git::git_at(
        Some(root),
        &["--literal-pathspecs", "ls-files", "-z", "--", relative],
    )?;
    if !output.ok {
        return Err(format!(
            "cannot inspect the index for {relative}: {}",
            output.stderr.trim()
        ));
    }
    Ok(!output.stdout.is_empty())
}

fn is_ignored(root: &Path, relative: &str) -> Result<bool, String> {
    let output = git::git_at(
        Some(root),
        &["check-ignore", "-q", "--", relative],
    )?;
    match output.code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "cannot inspect ignore rules for {relative}: {}",
            output.stderr.trim()
        )),
    }
}

fn file_mode(path: &Path) -> &'static str {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(path) {
            if metadata.permissions().mode() & 0o111 != 0 {
                return "100755";
            }
        }
    }
    let _ = path;
    "100644"
}

fn untracked_diff(root: &Path, resolved: &ResolvedPath) -> Result<String, String> {
    let relative = &resolved.relative;
    let alias = joined(root, &path_segments(relative)?);
    let alias_metadata = fs::symlink_metadata(&alias)
        .map_err(|error| format!("{relative} is not accessible: {error}"))?;
    let (bytes, mode) = if alias_metadata.file_type().is_symlink() {
        let parent = canonical_child(alias.parent().ok_or("link parent is missing")?)?;
        if !inside_workspace(root, &parent) { return Err("link parent resolves outside the workspace".into()); }
        let target = fs::read_link(&alias).map_err(|error| format!("cannot read link {relative}: {error}"))?;
        let text = target.to_str().ok_or("link target is not valid Unicode")?;
        (text.as_bytes().to_vec(), "120000")
    } else {
        let full = canonical_child(&resolved.full)?;
        if !inside_workspace(root, &full) { return Err("file resolves outside the workspace".into()); }
        if fs::metadata(&full).map_err(|error| error.to_string())?.is_dir() { return Ok(String::new()); }
        (read_bounded(&full, relative)?, file_mode(&full))
    };

    let content = match std::str::from_utf8(&bytes) {
        Ok(text) if !bytes.contains(&0) => text,
        _ => return Ok(format!(
            "diff --git a/{relative} b/{relative}\nnew file mode {mode}\nBinary files /dev/null and b/{relative} differ\n"
        )),
    };

    let line_count = content.split_terminator('\n').count();
    if content.len().saturating_add(line_count) > MAX_TEXT_BYTES as usize {
        return Err(format!("{relative} exceeds the 2 MiB diff limit"));
    }
    let mut diff = String::with_capacity(content.len() + line_count + 256);
    let _ = write!(diff, "diff --git a/{relative} b/{relative}\n");
    if content.is_empty() {
        let _ = write!(diff, "new file mode {mode}\n");
        return Ok(diff);
    }
    let _ = write!(diff, "new file mode {mode}\n--- /dev/null\n+++ b/{relative}\n");
    let _ = write!(diff, "@@ -0,0 +1,{line_count} @@\n");
    for line in content.split_terminator('\n') {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    if !content.ends_with('\n') {
        diff.push_str("\\ No newline at end of file\n");
    }
    if diff.len() as u64 > MAX_TEXT_BYTES {
        return Err(format!("{relative} exceeds the 2 MiB diff limit"));
    }
    Ok(diff)
}

/// Both sides of a detected rename are given to `git diff`, so a rename renders
/// with its original path instead of as an unrelated addition and deletion.
fn diff_paths(root: &Path, relative: &str) -> Result<Vec<String>, String> {
    let mut paths = vec![relative.to_string()];
    let status = read_status(root)?;
    for change in status.staged.iter().chain(status.unstaged.iter()) {
        let Some(old_path) = change.old_path.as_deref() else {
            continue;
        };
        if change.path != relative && old_path != relative {
            continue;
        }
        for candidate in [change.path.clone(), old_path.to_string()] {
            if !paths.contains(&candidate) {
                paths.push(candidate);
            }
        }
    }
    Ok(paths)
}

/// Real diff for one path: index versus HEAD when `staged`, otherwise index
/// versus working tree. Untracked text files become a new-file diff from their
/// actual content; ignored files report no diff.
pub fn file_diff(root: &Path, path: &str, staged: bool) -> Result<String, String> {
    let root = canonical_root(root)?;
    let resolved = resolve_mutable(&root, path)?;
    let mut args: Vec<String> = vec![
        "--literal-pathspecs".into(),
        "diff".into(),
        "--no-ext-diff".into(),
        "--no-textconv".into(),
    ];
    if staged {
        args.push("--cached".into());
    }
    args.push("--".into());
    args.extend(diff_paths(&root, &resolved.relative)?);
    let diff = run_git_capture(&root, &args, "cannot render the file diff")?;
    if !staged && diff.is_empty() {
        if is_tracked(&root, &resolved.relative)? {
            return Ok(String::new());
        }
        if is_ignored(&root, &resolved.relative)? {
            return Ok(String::new());
        }
        return untracked_diff(&root, &resolved);
    }
    Ok(diff)
}

fn run_git_capture(root: &Path, args: &[String], action: &str) -> Result<String, String> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    git::git_ok(Some(root), &refs, action)
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
