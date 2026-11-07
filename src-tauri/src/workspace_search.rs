//! Bounded literal content search with explicitly loaded Git ignore rules.
//!
//! The previous walker handed the tree to `ignore::WalkBuilder`, which reads
//! every `.gitignore`, `.ignore`, global and parent rule file with an unbounded
//! `BufRead::lines` loop before a candidate or byte bound can apply. This
//! module instead loads only the ignore files Git would consult for the
//! workspace root, reads each one through a hard byte/rule cap, and walks the
//! tree lazily with an explicit depth-first stack. Oversized ignore
//! configuration is an explicit error, never a silent "everything is
//! searchable" fallback.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;
use regex::RegexBuilder;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::{
    canonical_root, clip_line, is_metadata_name, read_bounded, relative_slash, SearchMatch,
    SearchResults, MAX_SEARCH_BYTES, MAX_SEARCH_CANDIDATES, MAX_SEARCH_HITS, MAX_TEXT_BYTES,
};

/// Hard bounds for ignore configuration. Each file is capped independently and
/// the whole search shares an aggregate budget so a tree of hostile
/// `.gitignore` files cannot turn into unbounded reading or glob compilation.
const MAX_SEARCH_DIRECTORIES: usize = 20_000;
const MAX_IGNORE_FILE_BYTES: u64 = 512 * 1024;
const MAX_IGNORE_RULES: usize = 10_000;
const MAX_IGNORE_RULE_BYTES: usize = 8 * 1024;
const MAX_IGNORE_TOTAL_BYTES: u64 = 4 * 1024 * 1024;
const MAX_IGNORE_TOTAL_RULES: usize = 50_000;

/// Literal text search under Git ignore rules. Never follows links, skips
/// `.git`/`.minnow` and temporary Minnow metadata, bounds directories,
/// candidates, scanned bytes, per-file text and results, and reports every
/// bound that cut the result set through `truncated`.
pub fn search(root: &Path, query: &str, case_sensitive: bool) -> Result<SearchResults, String> {
    if query.is_empty() {
        return Err("search text is empty".to_string());
    }
    let root = canonical_root(root)?;
    let pattern = RegexBuilder::new(&regex::escape(query))
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|error| format!("cannot prepare the literal search: {error}"))?;

    // `.gitignore`, `.git/info/exclude` and the global excludes file only
    // apply inside a work tree. `.ignore` applies everywhere, like ripgrep.
    let git_rules = has_git_metadata(&root);
    let mut budget = IgnoreBudget::default();
    let mut exclude_matcher = None;
    let mut global_matcher = None;
    if git_rules {
        if let Some(exclude_path) = info_exclude_path(&root)? {
            exclude_matcher =
                load_ignore_file(&exclude_path, ".git/info/exclude", &root, &mut budget)?;
        }
        if let Some(global_path) = global_excludes_path(&root)? {
            let label = format!("global excludes {}", global_path.display());
            global_matcher = load_ignore_file(&global_path, &label, &root, &mut budget)?;
        }
    }

    let mut ignore_layers: Vec<Gitignore> = Vec::new();
    let mut gitignore_layers: Vec<Gitignore> = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut matches: Vec<SearchMatch> = Vec::new();
    let mut truncated = false;
    let mut candidates = 0usize;
    let mut directories = 1usize;
    let mut scanned_bytes = 0u64;

    if let Some(frame) = open_frame(
        &root,
        root.clone(),
        String::new(),
        git_rules,
        &mut ignore_layers,
        &mut gitignore_layers,
        &mut budget,
    )? {
        frames.push(frame);
    }

    'walk: while !frames.is_empty() {
        let index = frames.len() - 1;
        let next = frames[index].entries.next();
        let entry = match next {
            None => {
                let frame = frames.pop().expect("the frame index exists");
                if frame.gitignore_pushed {
                    gitignore_layers.pop();
                }
                if frame.ignore_pushed {
                    ignore_layers.pop();
                }
                continue;
            }
            Some(Err(_)) => continue,
            Some(Ok(entry)) => entry,
        };

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_metadata_name(&name) {
            continue;
        }
        let is_dir = file_type.is_dir();
        if !is_dir && !file_type.is_file() {
            continue;
        }

        let path = entry.path();
        if is_ignored(
            &path,
            is_dir,
            &ignore_layers,
            &gitignore_layers,
            exclude_matcher.as_ref(),
            global_matcher.as_ref(),
        ) {
            continue;
        }

        if is_dir {
            if directories >= MAX_SEARCH_DIRECTORIES {
                truncated = true;
                break 'walk;
            }
            directories += 1;
            let rel = {
                let prefix = frames[index].rel.as_str();
                if prefix.is_empty() {
                    name
                } else {
                    format!("{prefix}/{name}")
                }
            };
            if let Some(frame) = open_frame(
                &root,
                path,
                rel,
                git_rules,
                &mut ignore_layers,
                &mut gitignore_layers,
                &mut budget,
            )? {
                frames.push(frame);
            }
            continue;
        }

        candidates += 1;
        if candidates > MAX_SEARCH_CANDIDATES {
            truncated = true;
            break 'walk;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if !metadata.is_file() {
            continue;
        }
        if metadata.len() > MAX_TEXT_BYTES {
            truncated = true;
            continue;
        }
        if metadata.len() > MAX_SEARCH_BYTES.saturating_sub(scanned_bytes) {
            truncated = true;
            break 'walk;
        }
        let bytes = match read_bounded(&path, "search candidate") {
            Ok(bytes) => bytes,
            Err(_) => {
                truncated = true;
                continue;
            }
        };
        scanned_bytes += bytes.len() as u64;
        if bytes.len() as u64 > MAX_TEXT_BYTES {
            truncated = true;
            continue;
        }
        if bytes.contains(&0) {
            continue;
        }
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => continue,
        };

        let rel = {
            let prefix = frames[index].rel.as_str();
            if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            }
        };

        let mut limit_reached = false;
        for (line_index, line) in text.lines().enumerate() {
            // One forward scan over the line keeps Unicode columns exact and
            // avoids re-counting the prefix for every match on a huge line.
            let mut characters = line.char_indices().peekable();
            let mut column = 1u32;
            for found in pattern.find_iter(line) {
                while let Some(&(byte_index, _)) = characters.peek() {
                    if byte_index >= found.start() {
                        break;
                    }
                    characters.next();
                    column += 1;
                }
                matches.push(SearchMatch {
                    path: rel.clone(),
                    line: (line_index + 1) as u32,
                    column,
                    text: clip_line(line),
                });
                if matches.len() >= MAX_SEARCH_HITS {
                    truncated = true;
                    limit_reached = true;
                    break;
                }
            }
            if limit_reached {
                break;
            }
        }
        if limit_reached {
            break 'walk;
        }
    }

    matches.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.column.cmp(&right.column))
    });
    Ok(SearchResults { matches, truncated })
}

/* ------------------------------------------------------------------------- */
/* lazy directory walk                                                       */
/* ------------------------------------------------------------------------- */

struct Frame {
    rel: String,
    entries: fs::ReadDir,
    ignore_pushed: bool,
    gitignore_pushed: bool,
}

fn open_frame(
    root: &Path,
    dir: PathBuf,
    rel: String,
    git_rules: bool,
    ignore_layers: &mut Vec<Gitignore>,
    gitignore_layers: &mut Vec<Gitignore>,
    budget: &mut IgnoreBudget,
) -> Result<Option<Frame>, String> {
    let dotignore_label = ignore_label(root, &dir, ".ignore");
    let mut ignore_pushed = false;
    if let Some(matcher) =
        load_ignore_file(&dir.join(".ignore"), &dotignore_label, &dir, budget)?
    {
        ignore_layers.push(matcher);
        ignore_pushed = true;
    }

    let mut gitignore_pushed = false;
    if git_rules {
        let label = ignore_label(root, &dir, ".gitignore");
        match load_ignore_file(&dir.join(".gitignore"), &label, &dir, budget) {
            Ok(Some(matcher)) => {
                gitignore_layers.push(matcher);
                gitignore_pushed = true;
            }
            Ok(None) => {}
            Err(error) => {
                if ignore_pushed {
                    ignore_layers.pop();
                }
                return Err(error);
            }
        }
    }

    match fs::read_dir(&dir) {
        Ok(entries) => Ok(Some(Frame {
            rel,
            entries,
            ignore_pushed,
            gitignore_pushed,
        })),
        Err(_) => {
            if gitignore_pushed {
                gitignore_layers.pop();
            }
            if ignore_pushed {
                ignore_layers.pop();
            }
            Ok(None)
        }
    }
}

fn ignore_label(root: &Path, dir: &Path, file: &str) -> String {
    let relative = relative_slash(dir.strip_prefix(root).unwrap_or(dir));
    if relative.is_empty() {
        file.to_string()
    } else {
        format!("{relative}/{file}")
    }
}

/* ------------------------------------------------------------------------- */
/* ignore matching                                                           */
/* ------------------------------------------------------------------------- */

/// Category precedence follows the `ignore` crate the application used before:
/// a deeper `.ignore` beats a shallower one, `.ignore` beats `.gitignore`, and
/// `.gitignore` beats `info/exclude`, which beats the global excludes file.
/// Within one matcher the last matching rule wins, so `!` negation works as in
/// Git. Ignored directories are never entered, which is why a negation cannot
/// re-include a file below an ignored parent.
fn is_ignored(
    path: &Path,
    is_dir: bool,
    ignore_layers: &[Gitignore],
    gitignore_layers: &[Gitignore],
    exclude_matcher: Option<&Gitignore>,
    global_matcher: Option<&Gitignore>,
) -> bool {
    if let Some(decision) = deepest_layer_match(ignore_layers, path, is_dir) {
        return decision;
    }
    if let Some(decision) = deepest_layer_match(gitignore_layers, path, is_dir) {
        return decision;
    }
    if let Some(matcher) = exclude_matcher {
        match matcher.matched(path, is_dir) {
            Match::Ignore(_) => return true,
            Match::Whitelist(_) => return false,
            Match::None => {}
        }
    }
    if let Some(matcher) = global_matcher {
        match matcher.matched(path, is_dir) {
            Match::Ignore(_) => return true,
            Match::Whitelist(_) => return false,
            Match::None => {}
        }
    }
    false
}

fn deepest_layer_match(layers: &[Gitignore], path: &Path, is_dir: bool) -> Option<bool> {
    // Layers are pushed from the root towards the current directory, so the
    // last entry is the nearest `.ignore`/`.gitignore` and takes precedence.
    for matcher in layers.iter().rev() {
        match matcher.matched(path, is_dir) {
            Match::Ignore(_) => return Some(true),
            Match::Whitelist(_) => return Some(false),
            Match::None => {}
        }
    }
    None
}

#[derive(Default)]
struct IgnoreBudget {
    bytes: u64,
    rules: usize,
}

/// Reads one ignore file through explicit byte and rule bounds. A missing file
/// is normal; an oversized, binary or non-UTF-8 file is an error because
/// silently skipping it would make ignored content searchable.
fn load_ignore_file(
    path: &Path,
    label: &str,
    match_root: &Path,
    budget: &mut IgnoreBudget,
) -> Result<Option<Gitignore>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot inspect {label}: {error}")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > MAX_IGNORE_FILE_BYTES {
        return Err(format!(
            "ignore file {label} is {} bytes and exceeds the {MAX_IGNORE_FILE_BYTES} byte search limit",
            metadata.len()
        ));
    }

    let file = fs::File::open(path).map_err(|error| format!("cannot read {label}: {error}"))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_IGNORE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {label}: {error}"))?;
    if bytes.len() as u64 > MAX_IGNORE_FILE_BYTES {
        return Err(format!(
            "ignore file {label} exceeds the {MAX_IGNORE_FILE_BYTES} byte search limit"
        ));
    }
    if bytes.contains(&0) {
        return Err(format!("ignore file {label} is a binary file"));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| format!("ignore file {label} is not valid UTF-8 text"))?;

    budget.bytes += text.len() as u64;
    if budget.bytes > MAX_IGNORE_TOTAL_BYTES {
        return Err(format!(
            "ignore configuration exceeds the {MAX_IGNORE_TOTAL_BYTES} byte search limit"
        ));
    }

    let mut builder = GitignoreBuilder::new(match_root);
    let mut rules = 0usize;
    for (index, raw_line) in text.lines().enumerate() {
        // Git strips a UTF-8 BOM from the first line of an ignore file.
        let line = if index == 0 {
            raw_line.trim_start_matches('\u{feff}')
        } else {
            raw_line
        };
        if line.starts_with('#') || line.trim_end().is_empty() {
            continue;
        }
        if line.len() > MAX_IGNORE_RULE_BYTES {
            return Err(format!(
                "ignore file {label} has a rule longer than {MAX_IGNORE_RULE_BYTES} bytes"
            ));
        }
        rules += 1;
        if rules > MAX_IGNORE_RULES {
            return Err(format!(
                "ignore file {label} has more than {MAX_IGNORE_RULES} rules"
            ));
        }
        // Git tolerates individual malformed globs; the bounds above still cap
        // the amount of parsing and glob compilation performed.
        let _ = builder.add_line(Some(path.to_path_buf()), line);
    }
    budget.rules += rules;
    if budget.rules > MAX_IGNORE_TOTAL_RULES {
        return Err(format!(
            "ignore configuration exceeds the {MAX_IGNORE_TOTAL_RULES} rule search limit"
        ));
    }

    let matcher = builder
        .build()
        .map_err(|error| format!("cannot parse ignore file {label}: {error}"))?;
    if matcher.is_empty() {
        Ok(None)
    } else {
        Ok(Some(matcher))
    }
}

/* ------------------------------------------------------------------------- */
/* Git configuration locations                                               */
/* ------------------------------------------------------------------------- */

/// True when the canonical workspace root itself is a Git work tree (a `.git`
/// directory or worktree pointer file). Ignore rules are still applied without
/// one: `.ignore` always applies, while Git-only rules require this marker,
/// mirroring the previous `require_git(true)` walker.
fn has_git_metadata(root: &Path) -> bool {
    match fs::symlink_metadata(root.join(".git")) {
        Ok(metadata) => {
            !metadata.file_type().is_symlink() && (metadata.is_dir() || metadata.is_file())
        }
        Err(_) => false,
    }
}

/// Resolve the worktree's exclude file without suppressing Git configuration errors.
fn info_exclude_path(root: &Path) -> Result<Option<PathBuf>, String> {
    let value = crate::git::git_ok(Some(root), &["rev-parse", "--git-path", "info/exclude"], "cannot locate workspace ignore rules")?;
    let value = value.trim_end_matches(&['\r', '\n'][..]);
    if value.is_empty() { return Err("Git did not return its exclude file path".into()); }
    let path = PathBuf::from(value);
    Ok(Some(if path.is_relative() { root.join(path) } else { path }))
}

/// Resolves `core.excludesFile` through Git so relative values and `~` follow
/// Git's own rules. When unset, the same `$XDG_CONFIG_HOME/git/ignore` or
/// `$HOME/.config/git/ignore` default the `ignore` crate used is tried.
fn global_excludes_path(root: &Path) -> Result<Option<PathBuf>, String> {
    let output = crate::git::git_at(Some(root), &["config", "--path", "--get", "core.excludesFile"])?;
    if output.code == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
        return Ok(default_global_excludes_path());
    }
    if !output.ok { return Err(format!("cannot read global ignore configuration: {}", output.stderr.trim())); }
    let value = output.stdout.trim_end_matches(&['\r', '\n'][..]);
    if value.is_empty() { return Ok(None); }
    let path = PathBuf::from(value);
    Ok(Some(if path.is_relative() { root.join(path) } else { path }))
}

fn default_global_excludes_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(xdg).join("git").join("ignore"));
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()))?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("git")
            .join("ignore"),
    )
}

#[cfg(test)]
#[path = "workspace_search_tests.rs"]
mod tests;
