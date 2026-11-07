//! Git worktree lifecycle for Minnow workers.
//!
//! Every git invocation uses an argument array, never a shell, and sets the
//! Windows `CREATE_NO_WINDOW` creation flag so background git work never flashes
//! a console window. Destructive operations validate that the path really is the
//! worktree Minnow created for that worker before touching it.

use crate::model::{Project, Worker, STATUS_EXITED};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const EXCLUDE_PATTERN: &str = "/.minnow/";
const MAX_GIT_OUTPUT: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct GitOutput {
    pub(crate) ok: bool,
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

/// Inspects a local repository path and builds the project record for it.
///
/// The input may point at any directory inside a work tree; the repository root
/// is resolved with `git rev-parse --show-toplevel`. Remote URLs, bare
/// repositories, repositories without an initial commit, and detached HEADs are
/// rejected because they cannot host worker worktrees or merge targets.
pub fn inspect_repository(input: &str) -> Result<Project, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("repository path is empty".to_string());
    }

    let requested = canonical_existing_dir(Path::new(input), "repository path")?;
    let root = repository_toplevel(&requested)?;

    let head = git_at(Some(&root), &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])?;
    if !head.ok {
        return Err(format!("repository {} has no initial commit", root.display()));
    }

    let branch = current_branch(&root)
        .map_err(|error| format!("{error}; a checked-out branch is required"))?;

    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().trim().to_string())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("cannot derive a project name from {}", root.display()))?;

    Ok(Project {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        path: display_string(&root),
        base_ref: branch,
    })
}

/// Creates the worker worktree, branch, and session directory.
///
/// All git resources are created in one operation. Any failure after the
/// worktree branch exists rolls back only the resources this call created and
/// preserves pre-existing files and branches.
pub fn create_worktree(project: &Project, name: &str, id: &str) -> Result<Worker, String> {
    validate_name(name)?;
    validate_worker_id(id)?;
    let short = short_id(id)?;
    let repo = verified_project_root(project)?;

    let base_ref = short_branch(project.base_ref.trim());
    if base_ref.is_empty() {
        return Err("project has no base branch".to_string());
    }
    let base_commit = resolve_branch(&repo, base_ref)?;

    let branch = format!("minnow/{name}-{short}");
    let worktree = worktree_path(&repo, name)?;
    let session = session_path(&repo, id)?;

    if fs::symlink_metadata(&worktree).is_ok() {
        return Err(format!("worktree path {} already exists", worktree.display()));
    }
    if ref_exists(&repo, &branch_ref(&branch))? {
        return Err(format!("branch '{branch}' already exists in {}", repo.display()));
    }
    if is_registered(&repo, &worktree)? {
        return Err(format!(
            "worktree path {} is already registered with git",
            worktree.display()
        ));
    }

    // Ignore only the metadata directory through the repository-local exclude
    // so tracked .gitignore files are never modified.
    ensure_local_exclude(&repo)?;

    let minnow = repo.join(".minnow");
    let worktrees_dir = minnow.join("worktrees");
    let dirs = CreatedDirs {
        minnow_existed: minnow.exists(),
        worktrees_existed: worktrees_dir.exists(),
        minnow,
        worktrees: worktrees_dir.clone(),
        session: session.clone(),
    };

    fs::create_dir_all(&worktrees_dir).map_err(|error| {
        dirs.cleanup();
        format!(
            "cannot create worktree directory {}: {error}",
            worktrees_dir.display()
        )
    })?;
    if let Err(error) = fs::create_dir_all(&session) {
        dirs.cleanup();
        return Err(format!(
            "cannot create session directory {}: {error}",
            session.display()
        ));
    }

    let worktree_text = display_string(&worktree);
    let add = match git_at(
        Some(&repo),
        &[
            "worktree",
            "add",
            "-b",
            branch.as_str(),
            worktree_text.as_str(),
            base_commit.as_str(),
        ],
    ) {
        Ok(output) => output,
        Err(error) => {
            return Err(creation_failure(
                &repo,
                &worktree,
                &branch,
                &base_commit,
                &dirs,
                error,
            ))
        }
    };

    if !add.ok {
        return Err(creation_failure(
            &repo,
            &worktree,
            &branch,
            &base_commit,
            &dirs,
            failure_message("cannot create git worktree", &add),
        ));
    }

    let created_head = match git_at(Some(&worktree), &["rev-parse", "HEAD"]) {
        Ok(output) if output.ok => output.stdout.trim().to_string(),
        Ok(output) => {
            return Err(creation_failure(
                &repo,
                &worktree,
                &branch,
                &base_commit,
                &dirs,
                failure_message("cannot verify the new worktree", &output),
            ))
        }
        Err(error) => {
            return Err(creation_failure(
                &repo,
                &worktree,
                &branch,
                &base_commit,
                &dirs,
                error,
            ))
        }
    };
    if created_head != base_commit {
        return Err(creation_failure(
            &repo,
            &worktree,
            &branch,
            &base_commit,
            &dirs,
            format!(
                "new worktree points at {created_head} but {base_commit} was requested"
            ),
        ));
    }

    Ok(Worker {
        id: id.to_string(),
        project_id: project.id.clone(),
        name: name.to_string(),
        path: worktree_text,
        branch,
        base_ref: base_ref.to_string(),
        base_commit,
        session_path: display_string(&session),
        status: STATUS_EXITED.to_string(),
        pid: None,
        exit_code: None,
        last_error: None,
        desired_running: true,
    })
}

/// Renders the worker's committed and uncommitted changes against its base
/// commit, plus a readable patch for every untracked file.
pub fn diff_worker(worker: &Worker) -> Result<String, String> {
    let worktree = canonical_existing_dir(Path::new(&worker.path), "worker worktree")?;
    let root = repository_toplevel(&worktree)?;
    if !same_path(&root, &worktree) {
        return Err(format!(
            "worker path {} is not a git worktree root",
            worktree.display()
        ));
    }
    if !is_owned_worktree(&worktree, &worker.name) {
        return Err(format!(
            "worker path {} is not the Minnow worktree for '{}'",
            worktree.display(),
            worker.name
        ));
    }

    let base_commit = worker.base_commit.trim();
    if base_commit.len() < 4
        || base_commit.len() > 64
        || !base_commit.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err("worker has no valid recorded base commit".to_string());
    }

    // `git diff <commit>` covers both staged and unstaged edits of tracked
    // files. Untracked files are handled separately because git omits them.
    let tracked = git_at(
        Some(&worktree),
        &["diff", "--no-color", "--no-ext-diff", base_commit, "--"],
    )?;
    if !tracked.ok {
        return Err(failure_message("cannot read tracked diff", &tracked));
    }

    let mut output = tracked.stdout;
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }

    let untracked = git_ok(
        Some(&worktree),
        &["ls-files", "--others", "--exclude-standard", "-z"],
        "cannot list untracked files",
    )?;
    for relative in untracked.split('\0').filter(|entry| !entry.is_empty()) {
        let patch = git_at(
            Some(&worktree),
            &[
                "diff",
                "--no-index",
                "--no-color",
                "--no-ext-diff",
                "--",
                "/dev/null",
                relative,
            ],
        )?;
        match patch.code {
            Some(0) | Some(1) => {}
            _ => {
                return Err(failure_message(
                    &format!("cannot render untracked file '{relative}'"),
                    &patch,
                ))
            }
        }
        if patch.stdout.is_empty() {
            continue;
        }
        if output.len().saturating_add(patch.stdout.len()) > MAX_GIT_OUTPUT {
            return Err("Diff exceeds the 2 MiB display limit; inspect it with git in the worker worktree".into());
        }
        output.push_str(&patch.stdout);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    Ok(output)
}

/// Merges the worker branch into the project's captured base branch.
///
/// Both trees must be clean and the project must have the base branch checked
/// out. The merge never stages, commits, or removes anything by force. A
/// conflict leaves a clean project tree and returns diagnostics listing the
/// conflicting files: the merge is aborted because it was started here and no
/// pre-existing merge existed.
pub fn merge_worker(project: &Project, worker: &Worker) -> Result<String, String> {
    let repo = verified_project_root(project)?;
    verify_worker_identity(worker)?;
    let worktree = canonical_existing_dir(Path::new(&worker.path), "worker worktree")?;
    if !is_owned_worktree(&worktree, &worker.name) {
        return Err(format!(
            "worker worktree {} is not managed by Minnow",
            worktree.display()
        ));
    }
    let expected = worktree_path(&repo, &worker.name)?;
    if !same_path(&canonical_or_self(&expected), &worktree) {
        return Err(format!(
            "worker worktree {} is not inside this project's .minnow/worktrees directory",
            worktree.display()
        ));
    }
    if !is_registered(&repo, &worktree)? {
        return Err(format!(
            "worker worktree {} is not registered with git",
            worktree.display()
        ));
    }

    let branch = short_branch(worker.branch.trim());
    if branch.is_empty() {
        return Err("worker has no branch".to_string());
    }
    resolve_branch(&repo, branch)?;

    let worktree_branch = current_branch(&worktree)?;
    if worktree_branch != branch {
        return Err(format!(
            "worker worktree is on branch '{worktree_branch}', expected '{branch}'"
        ));
    }
    let worktree_status = status_entries(&worktree, false)?;
    if !worktree_status.is_empty() {
        return Err(format!(
            "worker worktree has uncommitted changes ({}); commit or discard them first",
            summarize(&worktree_status)
        ));
    }

    let base_ref = short_branch(project.base_ref.trim());
    if base_ref.is_empty() {
        return Err("project has no base branch".to_string());
    }
    resolve_branch(&repo, base_ref)?;

    let project_branch = current_branch(&repo)?;
    if project_branch != base_ref {
        return Err(format!(
            "project repository is on '{project_branch}', but '{base_ref}' must be checked out to merge"
        ));
    }
    if ref_exists(&repo, "MERGE_HEAD")? {
        return Err("a merge is already in progress in the project repository".to_string());
    }
    let project_status = status_entries(&repo, false)?;
    if !project_status.is_empty() {
        return Err(format!(
            "project repository has uncommitted changes ({}); commit or stash them first",
            summarize(&project_status)
        ));
    }

    let merge = git_at(Some(&repo), &["merge", "--no-edit", branch])?;
    if merge.ok {
        let summary = merge.stdout.trim();
        if summary.is_empty() {
            return Ok(format!("Merged '{branch}' into '{base_ref}'."));
        }
        return Ok(summary.to_string());
    }

    // A merge we started is the only merge that may be aborted here: the check
    // above proved that no merge was in progress before this call.
    let merge_in_progress = ref_exists(&repo, "MERGE_HEAD").unwrap_or(false);
    let unmerged = git_at(Some(&repo), &["diff", "--name-only", "--diff-filter=U"])
        .map(|output| output.stdout)
        .unwrap_or_default();
    let conflicts: Vec<&str> = unmerged
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if merge_in_progress || !conflicts.is_empty() {
        let abort = git_at(Some(&repo), &["merge", "--abort"]);
        let abort_note = match abort {
            Ok(output) if output.ok => {
                "The merge was aborted and the project tree was restored.".to_string()
            }
            Ok(output) => format!(
                "The merge could not be aborted automatically ({}); run `git merge --abort` in the project repository.",
                failure_message("merge abort", &output)
            ),
            Err(error) => format!(
                "The merge could not be aborted automatically ({error}); run `git merge --abort` in the project repository."
            ),
        };
        let files = if conflicts.is_empty() {
            "unknown files".to_string()
        } else {
            conflicts.join(", ")
        };
        return Err(format!(
            "Merge conflict in {files}. {abort_note} Resolve the conflict in the worker branch or merge it manually. Git reported: {}",
            failure_message("merge", &merge)
        ));
    }

    Err(failure_message("merge", &merge))
}

/// Removes a worker worktree and its merged branch.
///
/// Deletion is refused while the worktree contains modified, untracked, or
/// ignored files, and while the worker branch has commits that are not
/// ancestors of the project base branch. The session directory and its history
/// are never deleted here.
pub fn delete_worktree(project: &Project, worker: &Worker) -> Result<(), String> {
    let repo = verified_project_root(project)?;
    verify_worker_identity(worker)?;
    let expected = worktree_path(&repo, &worker.name)?;
    let declared = Path::new(&worker.path);
    let declared_canonical = canonical_or_self(declared);
    if !same_path(&declared_canonical, &canonical_or_self(&expected)) {
        return Err(format!(
            "worker worktree path {} is not the managed location {}",
            declared.display(),
            expected.display()
        ));
    }

    let branch = short_branch(worker.branch.trim());
    if branch.is_empty() {
        return Err("worker has no branch".to_string());
    }
    let base_ref = short_branch(project.base_ref.trim());
    if base_ref.is_empty() {
        return Err("project has no base branch".to_string());
    }
    resolve_branch(&repo, base_ref)?;
    let path_exists = fs::symlink_metadata(declared).is_ok();
    let branch_name = branch_ref(branch);
    if !path_exists && !ref_exists(&repo, &branch_name)? {
        // A previous delete removed both git resources (for example, the
        // process stopped before the caller removed its database row).
        if is_registered(&repo, &canonical_or_self(&expected))? {
            git_ok(
                Some(&repo),
                &["worktree", "prune"],
                "cannot prune the missing worktree registration",
            )?;
        }
        return Ok(());
    }
    let branch_commit = resolve_branch(&repo, branch)?;

    let worktree = if path_exists {
        let worktree = canonical_existing_dir(declared, "worker worktree")?;
        if !is_owned_worktree(&worktree, &worker.name) {
            return Err(format!(
                "worker worktree {} is not managed by Minnow",
                worktree.display()
            ));
        }
        if !is_registered(&repo, &worktree)? {
            return Err(format!(
                "worker worktree {} is not registered with git",
                worktree.display()
            ));
        }

        let worktree_branch = current_branch(&worktree)?;
        if worktree_branch != branch {
            return Err(format!(
                "worker worktree is on branch '{worktree_branch}', expected '{branch}'; refusing to delete it"
            ));
        }

        // `--ignored` is essential: git's own worktree removal deletes ignored
        // files without complaint, so any ignored file is treated as data.
        let status = status_entries(&worktree, true)?;
        if !status.is_empty() {
            return Err(format!(
                "worker worktree contains modified, untracked, or ignored files ({}); refusing to delete it. Commit them or remove them manually first.",
                summarize(&status)
            ));
        }
        Some(worktree)
    } else {
        // The directory is already gone. Only a stale registration may remain;
        // pruning it is metadata cleanup because no files exist at the path.
        if is_registered(&repo, &canonical_or_self(&expected))? {
            git_ok(
                Some(&repo),
                &["worktree", "prune"],
                "cannot prune the missing worktree registration",
            )?;
        }
        None
    };

    let base_name = branch_ref(base_ref);
    let ancestor = git_at(
        Some(&repo),
        &["merge-base", "--is-ancestor", branch_name.as_str(), base_name.as_str()],
    )?;
    match ancestor.code {
        Some(0) => {}
        Some(1) => {
            return Err(format!(
                "worker branch '{branch}' has commits that are not merged into '{base_ref}'; merge it before deleting the worker"
            ))
        }
        _ => return Err(failure_message("cannot verify merge status", &ancestor)),
    }

    if let Some(worktree) = &worktree {
        let worktree_text = display_string(worktree);
        // No --force: git's native guards stay active in addition to the checks
        // above.
        let remove = git_at(
            Some(&repo),
            &["worktree", "remove", worktree_text.as_str()],
        )?;
        if !remove.ok {
            return Err(failure_message("cannot remove worker worktree", &remove));
        }
    }

    let mut delete = git_at(Some(&repo), &["branch", "-d", branch])?;
    if !delete.ok {
        // `git branch -d` compares against HEAD rather than the base branch.
        // Ancestry against the base branch was already proven; confirm that the
        // branch did not move and delete it with the same safety guarantee.
        match resolve_branch(&repo, branch) {
            Err(_) => return Ok(()),
            Ok(tip) if tip == branch_commit => {
                let recheck = git_at(
                    Some(&repo),
                    &["merge-base", "--is-ancestor", branch_name.as_str(), base_name.as_str()],
                )?;
                if recheck.code == Some(0) {
                    delete = git_at(Some(&repo), &["branch", "-D", branch])?;
                    if delete.ok {
                        return Ok(());
                    }
                }
            }
            Ok(_) => {
                return Err(format!(
                    "worker branch '{branch}' moved while deleting it; refusing to force-delete it"
                ))
            }
        }
        return Err(format!(
            "worker worktree was removed but branch '{branch}' could not be deleted: {}",
            failure_message("branch delete", &delete)
        ));
    }

    Ok(())
}

/* ------------------------------------------------------------------------- */
/* git process helpers                                                       */
/* ------------------------------------------------------------------------- */

fn new_git() -> Command {
    let mut command = Command::new("git");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub(crate) fn git_at(repo: Option<&Path>, args: &[&str]) -> Result<GitOutput, String> {
    let mut command = new_git();
    command.arg("-c").arg("core.quotepath=false");
    if let Some(repo) = repo {
        command.arg("-C").arg(repo);
    }
    command.args(args);

    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot run git {}: {error}", args.join(" ")))?;
    let mut stderr = child.stderr.take().ok_or("Git stderr pipe is missing")?;
    let error_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.by_ref().take(64 * 1024).read_to_end(&mut bytes);
        let _ = std::io::copy(&mut stderr, &mut std::io::sink());
        result.map(|_| bytes)
    });
    let mut bytes = Vec::new();
    let read_result = child.stdout.take().ok_or("Git stdout pipe is missing")?
        .take(MAX_GIT_OUTPUT as u64 + 1).read_to_end(&mut bytes);
    let too_large = bytes.len() > MAX_GIT_OUTPUT;
    if too_large || read_result.is_err() { let _ = child.kill(); }
    let status = child.wait().map_err(|error| error.to_string())?;
    let errors = error_reader.join().map_err(|_| "Git stderr reader failed")?
        .map_err(|error| error.to_string())?;
    read_result.map_err(|error| error.to_string())?;
    if too_large {
        return Err("Git output exceeds the 2 MiB display limit; inspect it in the worker worktree".into());
    }
    let decode = |bytes| String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
    Ok(GitOutput {
        ok: status.success(),
        code: status.code(),
        stdout: decode(bytes),
        stderr: decode(errors),
    })
}

pub(crate) fn git_ok(repo: Option<&Path>, args: &[&str], action: &str) -> Result<String, String> {
    let output = git_at(repo, args)?;
    if !output.ok {
        return Err(failure_message(action, &output));
    }
    Ok(output.stdout)
}

fn failure_message(action: &str, output: &GitOutput) -> String {
    let detail = if !output.stderr.trim().is_empty() {
        output.stderr.trim()
    } else {
        output.stdout.trim()
    };
    if detail.is_empty() {
        match output.code {
            Some(code) => format!("{action} failed (git exited with status {code})"),
            None => format!("{action} failed"),
        }
    } else {
        format!("{action} failed: {detail}")
    }
}

/* ------------------------------------------------------------------------- */
/* path helpers                                                              */
/* ------------------------------------------------------------------------- */

pub(crate) fn display_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

fn display_string(path: &Path) -> String {
    display_path(path).to_string_lossy().replace('/', "\\")
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = display_string(left);
    let right = display_string(right);
    #[cfg(windows)]
    {
        left.eq_ignore_ascii_case(&right)
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn canonical_existing(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        format!("{label} {} is not accessible: {error}", path.display())
    })?;
    Ok(display_path(&canonical))
}

pub(crate) fn canonical_existing_dir(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = canonical_existing(path, label)?;
    if !canonical.is_dir() {
        return Err(format!("{label} {} is not a directory", canonical.display()));
    }
    Ok(canonical)
}

fn canonical_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path)
        .map(|canonical| display_path(&canonical))
        .unwrap_or_else(|_| display_path(path))
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 48
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err(
            "worker name must be 1-48 characters using only ASCII letters, digits, '-' or '_'"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_worker_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(
            "worker id must be a UUID-like string of ASCII letters, digits, or '-'".to_string(),
        );
    }
    Ok(())
}

fn short_id(id: &str) -> Result<String, String> {
    validate_worker_id(id)?;
    let prefix: String = id.chars().take(8).collect();
    if prefix.len() != 8 || !prefix.chars().all(|character| character.is_ascii_alphanumeric()) {
        return Err(
            "worker id must begin with at least 8 ASCII alphanumeric characters".to_string(),
        );
    }
    Ok(prefix.to_ascii_lowercase())
}

/// Confirms that the stored branch is exactly the branch Minnow derives from
/// the worker's name and id. Destructive operations call this so a corrupted or
/// substituted worker record cannot point them at somebody else's worktree.
fn verify_worker_identity(worker: &Worker) -> Result<(), String> {
    let expected = format!("minnow/{}-{}", worker.name, short_id(&worker.id)?);
    let actual = short_branch(worker.branch.trim());
    if actual != expected.as_str() {
        return Err(format!(
            "worker branch '{actual}' does not match the expected Minnow branch '{expected}'"
        ));
    }
    Ok(())
}

fn worktree_path(repo: &Path, name: &str) -> Result<PathBuf, String> {
    validate_name(name)?;
    Ok(repo.join(".minnow").join("worktrees").join(name))
}

fn session_path(repo: &Path, id: &str) -> Result<PathBuf, String> {
    validate_worker_id(id)?;
    Ok(repo.join(".minnow").join("sessions").join(id))
}

fn is_owned_worktree(path: &Path, name: &str) -> bool {
    let parts: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    if parts.len() < 3 {
        return false;
    }
    let last = parts.len();
    parts[last - 3].eq_ignore_ascii_case(".minnow")
        && parts[last - 2].eq_ignore_ascii_case("worktrees")
        && parts[last - 1] == name
}

/* ------------------------------------------------------------------------- */
/* repository and ref helpers                                                */
/* ------------------------------------------------------------------------- */

fn repository_toplevel(path: &Path) -> Result<PathBuf, String> {
    let bare = git_at(Some(path), &["rev-parse", "--is-bare-repository"])?;
    if !bare.ok {
        return Err(format!("{} is not a git repository", path.display()));
    }
    if bare.stdout.trim() == "true" {
        return Err(format!(
            "{} is a bare repository; a working tree is required",
            path.display()
        ));
    }

    let inside = git_at(Some(path), &["rev-parse", "--is-inside-work-tree"])?;
    if !inside.ok || inside.stdout.trim() != "true" {
        return Err(format!("{} is not inside a git work tree", path.display()));
    }

    let toplevel = git_at(Some(path), &["rev-parse", "--show-toplevel"])?;
    if !toplevel.ok {
        return Err(failure_message("cannot locate the repository root", &toplevel));
    }
    let reported = toplevel.stdout.trim();
    if reported.is_empty() {
        return Err("git did not report a repository root".to_string());
    }
    canonical_existing_dir(Path::new(reported), "repository root")
}

fn verified_project_root(project: &Project) -> Result<PathBuf, String> {
    let project_path = canonical_existing_dir(Path::new(&project.path), "project path")?;
    let root = repository_toplevel(&project_path)?;
    if !same_path(&root, &project_path) {
        return Err(format!(
            "project path {} is not the git repository root {}",
            project_path.display(),
            root.display()
        ));
    }
    Ok(root)
}

fn short_branch(branch: &str) -> &str {
    branch.strip_prefix("refs/heads/").unwrap_or(branch)
}

fn branch_ref(branch: &str) -> String {
    format!("refs/heads/{}", short_branch(branch))
}

fn ref_exists(repo: &Path, refname: &str) -> Result<bool, String> {
    let output = git_at(Some(repo), &["rev-parse", "--verify", "--quiet", refname])?;
    match output.code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(failure_message("cannot inspect git reference", &output)),
    }
}

fn resolve_branch(repo: &Path, branch: &str) -> Result<String, String> {
    let refname = branch_ref(branch);
    let spec = format!("{refname}^{{commit}}");
    let output = git_at(
        Some(repo),
        &["rev-parse", "--verify", "--quiet", spec.as_str()],
    )?;
    match output.code {
        Some(0) => {
            let commit = output.stdout.trim().to_string();
            if commit.is_empty() {
                Err(format!("branch '{branch}' does not resolve to a commit"))
            } else {
                Ok(commit)
            }
        }
        Some(1) => Err(format!("branch '{branch}' does not exist")),
        _ => Err(failure_message(
            &format!("cannot resolve branch '{branch}'"),
            &output,
        )),
    }
}

fn current_branch(repo: &Path) -> Result<String, String> {
    let output = git_at(Some(repo), &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    match output.code {
        Some(0) => {
            let branch = output.stdout.trim();
            if branch.is_empty() {
                Err("HEAD is not on a branch".to_string())
            } else {
                Ok(branch.to_string())
            }
        }
        Some(1) => Err("HEAD is detached".to_string()),
        _ => Err(failure_message(
            "cannot determine the checked-out branch",
            &output,
        )),
    }
}

fn registered_worktrees(repo: &Path) -> Result<Vec<PathBuf>, String> {
    let text = git_ok(
        Some(repo),
        &["worktree", "list", "--porcelain"],
        "cannot list git worktrees",
    )?;
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect())
}

fn is_registered(repo: &Path, path: &Path) -> Result<bool, String> {
    let entries = registered_worktrees(repo)?;
    let target = canonical_or_self(path);
    Ok(entries
        .iter()
        .any(|entry| same_path(&canonical_or_self(entry), &target)))
}

fn status_entries(repo: &Path, include_ignored: bool) -> Result<Vec<String>, String> {
    let mut args = vec!["status", "--porcelain", "--untracked-files=normal"];
    if include_ignored {
        args.push("--ignored=traditional");
    }
    let text = git_ok(
        Some(repo),
        &args,
        "cannot inspect the working tree status",
    )?;
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect())
}

fn summarize(entries: &[String]) -> String {
    let shown = entries
        .iter()
        .take(6)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if entries.len() > 6 {
        format!("{shown}; and {} more", entries.len() - 6)
    } else {
        shown
    }
}

fn ensure_local_exclude(repo: &Path) -> Result<(), String> {
    let common = git_ok(
        Some(repo),
        &["rev-parse", "--git-common-dir"],
        "cannot locate git metadata",
    )?;
    let common = common.trim();
    if common.is_empty() {
        return Err("git did not report a common directory".to_string());
    }
    let common_path = PathBuf::from(common);
    let common_path = if common_path.is_absolute() {
        common_path
    } else {
        repo.join(common_path)
    };
    let common_path = canonical_existing_dir(&common_path, "git metadata directory")?;

    let info_dir = common_path.join("info");
    fs::create_dir_all(&info_dir)
        .map_err(|error| format!("cannot create {}: {error}", info_dir.display()))?;
    let exclude = info_dir.join("exclude");

    let existing = match fs::read_to_string(&exclude) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", exclude.display())),
    };
    let already_present = existing.lines().any(|line| {
        let line = line.trim();
        line == EXCLUDE_PATTERN || line == ".minnow/" || line == "/.minnow" || line == ".minnow"
    });
    if already_present {
        return Ok(());
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(EXCLUDE_PATTERN);
    updated.push('\n');
    fs::write(&exclude, updated)
        .map_err(|error| format!("cannot update {}: {error}", exclude.display()))?;
    Ok(())
}

/* ------------------------------------------------------------------------- */
/* create rollback                                                           */
/* ------------------------------------------------------------------------- */

struct CreatedDirs {
    minnow: PathBuf,
    worktrees: PathBuf,
    session: PathBuf,
    minnow_existed: bool,
    worktrees_existed: bool,
}

impl CreatedDirs {
    fn cleanup(&self) {
        let _ = fs::remove_dir(&self.session);
        if !self.worktrees_existed {
            let _ = fs::remove_dir(&self.worktrees);
        }
        if !self.minnow_existed {
            let _ = fs::remove_dir(&self.minnow);
        }
    }
}

fn creation_failure(
    repo: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
    dirs: &CreatedDirs,
    message: String,
) -> String {
    let mut errors = vec![message];
    rollback_create(repo, worktree, branch, base_commit, &mut errors);
    dirs.cleanup();
    errors.join("; ")
}

fn rollback_create(
    repo: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
    errors: &mut Vec<String>,
) {
    let worktree_text = display_string(worktree);
    let registered = is_registered(repo, worktree).unwrap_or(false);
    if registered {
        match git_at(
            Some(repo),
            &["worktree", "remove", "--force", worktree_text.as_str()],
        ) {
            Ok(output) if !output.ok => errors.push(failure_message(
                "cannot roll back the partially created worktree",
                &output,
            )),
            Err(error) => errors.push(error),
            _ => {}
        }
    } else {
        // `git worktree add` can fail after creating an empty directory. Only an
        // empty directory is removed; anything containing files is left alone.
        let _ = fs::remove_dir(worktree);
    }

    let branch_name = branch_ref(branch);
    if ref_exists(repo, &branch_name).unwrap_or(false) {
        let tip = git_at(
            Some(repo),
            &["rev-parse", "--verify", "--quiet", branch_name.as_str()],
        )
        .ok()
        .filter(|output| output.ok)
        .map(|output| output.stdout.trim().to_string());

        if tip.as_deref() == Some(base_commit) {
            match git_at(Some(repo), &["branch", "-D", branch]) {
                Ok(output) if !output.ok => errors.push(failure_message(
                    "cannot roll back the partially created branch",
                    &output,
                )),
                Err(error) => errors.push(error),
                _ => {}
            }
        } else {
            errors.push(format!(
                "left branch '{branch}' in place because it no longer points at the creation base commit"
            ));
        }
    }
}

/* ------------------------------------------------------------------------- */
/* tests                                                                     */
/* ------------------------------------------------------------------------- */

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn git(cwd: &Path, args: &[&str]) -> String {
        let output = git_at(Some(cwd), args)
            .unwrap_or_else(|error| panic!("git {args:?} could not run: {error}"));
        assert!(
            output.ok,
            "git {args:?} failed:\nstdout: {}\nstderr: {}",
            output.stdout, output.stderr
        );
        output.stdout
    }

    fn commit(cwd: &Path, message: &str) {
        git(cwd, &["add", "-A"]);
        git(cwd, &["commit", "-qm", message]);
    }

    fn init_repo() -> (TempDir, Project) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let repo = directory.path().join("repo with space");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.name", "Minnow test"]);
        git(&repo, &["config", "user.email", "minnow@example.invalid"]);
        git(&repo, &["config", "core.autocrlf", "false"]);
        fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
        fs::write(repo.join("tracked.txt"), "base\n").unwrap();
        commit(&repo, "initial");
        let project = inspect_repository(repo.to_str().unwrap()).expect("inspect repository");
        (directory, project)
    }

    fn repo_path(project: &Project) -> PathBuf {
        PathBuf::from(&project.path)
    }

    fn worker_id(value: u8) -> String {
        format!("{value:08x}-1111-4111-8111-111111111111")
    }

    fn assert_ref_missing(repo: &Path, refname: &str) {
        let output = git_at(Some(repo), &["rev-parse", "--verify", "--quiet", refname]).unwrap();
        assert!(!output.ok, "expected {refname} to be gone");
    }

    #[test]
    fn inspect_resolves_a_local_repository() {
        let (_directory, project) = init_repo();
        let repo = repo_path(&project);

        assert_eq!(project.name, "repo with space");
        assert_eq!(
            project.base_ref,
            git(&repo, &["symbolic-ref", "--short", "HEAD"]).trim()
        );
        assert_eq!(Path::new(&project.path), repo.as_path());
        assert!(!project.id.is_empty());

        let not_a_repo = tempfile::tempdir().unwrap();
        let error = inspect_repository(not_a_repo.path().to_str().unwrap()).unwrap_err();
        assert!(error.contains("not a git repository"), "{error}");

        let empty = tempfile::tempdir().unwrap();
        let empty_repo = empty.path().join("empty");
        fs::create_dir_all(&empty_repo).unwrap();
        git(&empty_repo, &["init", "-q"]);
        let error = inspect_repository(empty_repo.to_str().unwrap()).unwrap_err();
        assert!(error.contains("no initial commit"), "{error}");
    }

    #[test]
    fn create_builds_worktree_branch_and_local_exclude() {
        let (_directory, project) = init_repo();
        let repo = repo_path(&project);

        let worker = create_worktree(&project, "alpha", &worker_id(1)).unwrap();
        assert_eq!(worker.name, "alpha");
        assert_eq!(worker.branch, "minnow/alpha-00000001");
        assert_eq!(worker.base_ref, project.base_ref);
        assert_eq!(worker.status, STATUS_EXITED);
        assert_eq!(worker.pid, None);
        assert_eq!(worker.exit_code, None);
        assert_eq!(worker.last_error, None);
        assert!(worker.desired_running);
        assert!(Path::new(&worker.path).is_dir());
        assert!(Path::new(&worker.session_path).is_dir());
        assert!(worker
            .path
            .replace('\\', "/")
            .ends_with(".minnow/worktrees/alpha"));

        let head = git(Path::new(&worker.path), &["rev-parse", "HEAD"]);
        assert_eq!(head.trim(), worker.base_commit);

        // Metadata is ignored through .git/info/exclude, never .gitignore.
        assert_eq!(git(&repo, &["status", "--porcelain"]).trim(), "");
        let common = git(&repo, &["rev-parse", "--git-common-dir"]);
        let exclude = repo.join(common.trim()).join("info").join("exclude");
        let exclude_text = fs::read_to_string(exclude).unwrap();
        assert!(exclude_text.lines().any(|line| line.trim() == EXCLUDE_PATTERN));
        let ignore_text = fs::read_to_string(repo.join(".gitignore")).unwrap();
        assert_eq!(ignore_text, "*.log\n");

        // A duplicate name is refused before any filesystem work.
        let error = create_worktree(&project, "alpha", &worker_id(2)).unwrap_err();
        assert!(error.contains("already exists"), "{error}");

        // An unrelated file at the target path is never deleted.
        let intruder = repo
            .join(".minnow")
            .join("worktrees")
            .join("beta");
        fs::create_dir_all(&intruder).unwrap();
        fs::write(intruder.join("keep.txt"), "keep me").unwrap();
        let error = create_worktree(&project, "beta", &worker_id(3)).unwrap_err();
        assert!(error.contains("already exists"), "{error}");
        assert_eq!(
            fs::read_to_string(intruder.join("keep.txt")).unwrap(),
            "keep me"
        );

        assert!(create_worktree(&project, "bad name", &worker_id(4)).is_err());
    }

    #[test]
    fn diff_shows_committed_uncommitted_and_untracked_changes() {
        let (_directory, project) = init_repo();
        let worker = create_worktree(&project, "diff", &worker_id(2)).unwrap();
        let worktree = PathBuf::from(&worker.path);

        fs::write(worktree.join("committed.txt"), "committed content\n").unwrap();
        commit(&worktree, "worker commit");
        fs::write(worktree.join("tracked.txt"), "uncommitted edit\n").unwrap();
        fs::write(worktree.join("untracked note.txt"), "untracked content\n").unwrap();

        let diff = diff_worker(&worker).unwrap();
        assert!(diff.contains("diff --git"), "{diff}");
        assert!(diff.contains("committed content"), "{diff}");
        assert!(diff.contains("uncommitted edit"), "{diff}");
        assert!(diff.contains("untracked note.txt"), "{diff}");
        assert!(diff.contains("untracked content"), "{diff}");
    }

    #[test]
    fn oversized_diff_is_refused_without_modifying_worktree() {
        let (_directory, project) = init_repo();
        let worker = create_worktree(&project, "large-diff", &worker_id(8)).unwrap();
        let file = PathBuf::from(&worker.path).join("large.txt");
        let content = "a changed line\n".repeat(MAX_GIT_OUTPUT / 10);
        fs::write(&file, &content).unwrap();
        let error = diff_worker(&worker).unwrap_err();
        assert!(error.contains("2 MiB display limit"), "{error}");
        assert_eq!(fs::read_to_string(file).unwrap(), content);
    }

    #[test]
    fn merge_conflict_is_reported_and_project_tree_restored() {
        let (_directory, project) = init_repo();
        let repo = repo_path(&project);
        let worker = create_worktree(&project, "conflict", &worker_id(3)).unwrap();
        let worktree = PathBuf::from(&worker.path);

        fs::write(worktree.join("tracked.txt"), "worker version\n").unwrap();
        commit(&worktree, "worker change");

        fs::write(repo.join("tracked.txt"), "project version\n").unwrap();
        commit(&repo, "project change");

        let error = merge_worker(&project, &worker).unwrap_err();
        assert!(error.contains("conflict"), "{error}");
        assert!(error.contains("tracked.txt"), "{error}");
        assert_eq!(git(&repo, &["status", "--porcelain"]).trim(), "");
        assert!(!ref_exists(&repo, "MERGE_HEAD").unwrap());
        assert_eq!(
            fs::read_to_string(worktree.join("tracked.txt")).unwrap(),
            "worker version\n"
        );
    }

    #[test]
    fn delete_refuses_dirty_ignored_and_unmerged_work() {
        let (_directory, project) = init_repo();
        let repo = repo_path(&project);
        let worker = create_worktree(&project, "guard", &worker_id(4)).unwrap();
        let worktree = PathBuf::from(&worker.path);

        fs::write(worktree.join("scratch.txt"), "draft\n").unwrap();
        let error = delete_worktree(&project, &worker).unwrap_err();
        assert!(error.contains("refusing"), "{error}");
        assert!(worktree.join("scratch.txt").exists());
        fs::remove_file(worktree.join("scratch.txt")).unwrap();

        fs::write(worktree.join("debug.log"), "log\n").unwrap();
        let error = delete_worktree(&project, &worker).unwrap_err();
        assert!(error.contains("ignored"), "{error}");
        assert!(worktree.join("debug.log").exists());
        fs::remove_file(worktree.join("debug.log")).unwrap();

        fs::write(worktree.join("work.txt"), "worker work\n").unwrap();
        commit(&worktree, "worker work");
        let error = delete_worktree(&project, &worker).unwrap_err();
        assert!(error.contains("not merged"), "{error}");
        assert!(worktree.exists());

        merge_worker(&project, &worker).unwrap();

        // Deletion is judged against the captured base branch, not whatever is
        // checked out. Move the project to a branch without the merge so
        // `git branch -d` would normally refuse; the verified base ancestry
        // still allows a safe delete.
        git(
            &repo,
            &["checkout", "-q", "-b", "side", worker.base_commit.as_str()],
        );
        delete_worktree(&project, &worker).unwrap();
        assert!(!worktree.exists());
        assert_ref_missing(&repo, &branch_ref(&worker.branch));
        assert!(Path::new(&worker.session_path).is_dir());
    }
}
