use super::*;

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    git::git_ok(Some(root), &["init", "--initial-branch=main"], "initialize fixture").unwrap();
    git::git_ok(Some(root), &["config", "user.name", "Minnow Tests"], "configure fixture").unwrap();
    git::git_ok(Some(root), &["config", "user.email", "tests@minnow.invalid"], "configure fixture").unwrap();
    git::git_ok(Some(root), &["config", "commit.gpgsign", "false"], "configure fixture").unwrap();
    git::git_ok(Some(root), &["commit", "--allow-empty", "-m", "Initial fixture"], "initialize fixture").unwrap();
    directory
}

#[test]
fn save_refuses_stale_content_and_create_only_collisions() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("document.txt"), "original\r\n").unwrap();
    let original = read_file(root, "document.txt").unwrap();
    fs::write(root.join("document.txt"), "external edit\r\n").unwrap();
    assert!(write_file(root, "document.txt", "lost update", &original.revision).is_err());
    assert!(write_file(root, "document.txt", "create over existing", "").is_err());
    assert_eq!(fs::read_to_string(root.join("document.txt")).unwrap(), "external edit\r\n");
    let latest = read_file(root, "document.txt").unwrap();
    let saved = write_file(root, "document.txt", "accepted edit\r\n", &latest.revision).unwrap();
    assert_eq!(fs::read_to_string(root.join("document.txt")).unwrap(), saved.content);
    assert_eq!(read_file(root, "document.txt").unwrap().revision, saved.revision);
    let created = write_file(root, "new.txt", "created", "").unwrap();
    assert_eq!(read_file(root, "new.txt").unwrap().content, created.content);
}

#[test]
fn workspace_paths_cannot_escape_or_expose_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("workspace");
    fs::create_dir(&root).unwrap();
    fs::write(directory.path().join("outside.txt"), "outside").unwrap();
    fs::create_dir(root.join(".git")).unwrap();
    fs::write(root.join(".git/config"), "metadata").unwrap();
    for path in ["../outside.txt", ".git/config", ".minnow/state", "C:\\outside.txt", "\\\\host\\share\\file"] {
        assert!(read_file(&root, path).is_err(), "read permitted {path}");
        assert!(write_file(&root, path, "unexpected", "").is_err(), "write permitted {path}");
    }
    let absolute = directory.path().join("outside.txt");
    assert!(read_file(&root, absolute.to_str().unwrap()).is_err());
    assert_eq!(fs::read_to_string(absolute).unwrap(), "outside");
}

#[test]
fn text_search_is_bounded_on_one_long_line() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("long.txt"), format!("éé{}", "a".repeat(1_000_000))).unwrap();
    let result = search(root, "a", true).unwrap();
    assert_eq!(result.matches.len(), MAX_SEARCH_HITS);
    assert!(result.truncated);
    assert_eq!(result.matches[0].column, 3);
    assert_eq!(result.matches[MAX_SEARCH_HITS - 1].column, (MAX_SEARCH_HITS + 2) as u32);
}

#[test]
fn insensitive_search_reports_original_unicode_columns_and_literal_patterns() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("unicode.txt"), "İaZ\n[A]+ literal\n").unwrap();
    let insensitive = search(root, "z", false).unwrap();
    assert_eq!(insensitive.matches.iter().map(|hit| (hit.line, hit.column)).collect::<Vec<_>>(), vec![(1, 3)]);
    let literal = search(root, "[A]+", true).unwrap();
    assert_eq!(literal.matches.iter().map(|hit| (hit.line, hit.column)).collect::<Vec<_>>(), vec![(2, 1)]);
    assert!(search(root, "z", true).unwrap().matches.is_empty());
}

#[test]
fn staging_is_literal_and_unstaging_preserves_working_files() {
    let directory = repository();
    let root = directory.path();
    fs::write(root.join("[one].txt"), "selected").unwrap();
    fs::write(root.join("o.txt"), "not selected").unwrap();
    stage_paths(root, Some(vec!["[one].txt".into()])).unwrap();
    let status = git_status(root).unwrap();
    assert_eq!(status.staged.iter().map(|file| file.path.as_str()).collect::<Vec<_>>(), vec!["[one].txt"]);
    assert_eq!(status.untracked.iter().map(|file| file.path.as_str()).collect::<Vec<_>>(), vec!["o.txt"]);
    unstage_paths(root, Some(vec!["[one].txt".into()])).unwrap();
    assert!(git_status(root).unwrap().staged.is_empty());
    assert_eq!(fs::read_to_string(root.join("[one].txt")).unwrap(), "selected");
}

#[test]
fn commit_preserves_unstaged_changes_and_history_control_characters() {
    let directory = repository();
    let root = directory.path();
    fs::write(root.join("document.txt"), "staged\n").unwrap();
    stage_paths(root, Some(vec!["document.txt".into()])).unwrap();
    fs::write(root.join("document.txt"), "unstaged\n").unwrap();
    let status = git_status(root).unwrap();
    assert_eq!(status.staged[0].path, "document.txt");
    assert_eq!(status.unstaged[0].path, "document.txt");
    let subject = "Save staged version \u{1f} without losing history fields";
    let id = commit(root, subject).unwrap();
    let history = git_history(root, 1).unwrap();
    assert_eq!(history[0].id, id);
    assert_eq!(history[0].subject, subject);
    assert_eq!(history[0].author, "Minnow Tests");
    assert_eq!(git::git_ok(Some(root), &["show", "HEAD:document.txt"], "inspect commit").unwrap(), "staged\n");
    assert_eq!(fs::read_to_string(root.join("document.txt")).unwrap(), "unstaged\n");
    let status = git_status(root).unwrap();
    assert!(status.staged.is_empty());
    assert_eq!(status.unstaged[0].status, "M");
}

#[test]
fn untracked_file_diff_uses_actual_content_and_literal_ignore_paths() {
    let directory = repository();
    let root = directory.path();
    fs::write(root.join("[draft].txt"), "actual untracked text\n").unwrap();
    let diff = file_diff(root, "[draft].txt", false).unwrap();
    assert!(diff.contains("+++ b/[draft].txt\n"));
    assert!(diff.contains("+actual untracked text\n"));
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.join("ignored.txt"), "ignored content\n").unwrap();
    assert_eq!(file_diff(root, "ignored.txt", false).unwrap(), "");
}

#[test]
fn staging_renamed_destination_does_not_stage_recreated_source() {
    let directory = repository();
    let root = directory.path();
    fs::write(root.join("original.txt"), "tracked content\n").unwrap();
    stage_paths(root, Some(vec!["original.txt".into()])).unwrap();
    commit(root, "track original").unwrap();
    git::git_ok(Some(root), &["mv", "original.txt", "renamed.txt"], "rename fixture").unwrap();
    fs::write(root.join("original.txt"), "unrelated replacement\n").unwrap();
    fs::write(root.join("renamed.txt"), "tracked content\nselected edit\n").unwrap();
    stage_paths(root, Some(vec!["renamed.txt".into()])).unwrap();
    let status = git_status(root).unwrap();
    assert_eq!(status.untracked.iter().map(|entry| entry.path.as_str()).collect::<Vec<_>>(), vec!["original.txt"]);
    assert!(!status.staged.iter().any(|entry| entry.path == "original.txt"));
    assert!(file_diff(root, "renamed.txt", true).unwrap().contains("+selected edit"));
}

#[cfg(unix)]
#[test]
fn untracked_symlink_diff_reports_link_not_target_contents() {
    let directory = repository();
    let root = directory.path();
    fs::write(root.join("target.txt"), "target contents must not be exposed\n").unwrap();
    std::os::unix::fs::symlink("target.txt", root.join("link.txt")).unwrap();
    let diff = file_diff(root, "link.txt", false).unwrap();
    assert!(diff.contains("new file mode 120000"));
    assert!(diff.contains("+target.txt\n"));
    assert!(!diff.contains("target contents must not be exposed"));
}

#[test]
fn ignored_paths_follow_gitignore_and_reject_unsafe_paths() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    crate::git::git_ok(Some(root), &["init", "--initial-branch=main"], "init").unwrap();
    fs::write(root.join(".gitignore"), "dist/\n*.log\n").unwrap();
    fs::create_dir_all(root.join("dist")).unwrap();
    fs::write(root.join("dist/out.js"), "x").unwrap();
    fs::write(root.join("a.log"), "x").unwrap();
    fs::write(root.join("keep.txt"), "x").unwrap();

    let wanted = ["dist", "dist/out.js", "a.log", "keep.txt"].map(String::from).to_vec();
    let mut ignored = git_ignored_paths(root, &wanted).unwrap();
    ignored.sort();
    assert_eq!(ignored, ["a.log", "dist", "dist/out.js"]);
    assert!(git_ignored_paths(root, &["keep.txt".to_string()]).unwrap().is_empty());
    assert!(git_ignored_paths(root, &["../escape".to_string()]).is_err());
    assert!(git_ignored_paths(root, &["C:/x".to_string()]).is_err());
}

#[test]
fn create_rename_delete_stay_inside_the_workspace() {
    let directory = repository();
    let root = directory.path();
    fs::create_dir(root.join("src")).unwrap();

    assert_eq!(create_entry(root, "src/new.txt", false).unwrap(), "src/new.txt");
    assert!(root.join("src/new.txt").is_file());
    assert_eq!(create_entry(root, "src/inner", true).unwrap(), "src/inner");
    assert!(root.join("src/inner").is_dir());
    assert!(create_entry(root, "src/new.txt", false).unwrap_err().contains("already exists"));
    assert!(create_entry(root, "missing/dir/file.txt", false).unwrap_err().contains("does not exist"));
    assert!(create_entry(root, "../escape.txt", false).is_err());
    assert!(create_entry(root, ".git/config2", false).is_err());

    assert_eq!(rename_entry(root, "src/new.txt", "src/renamed.txt").unwrap(), "src/renamed.txt");
    assert!(!root.join("src/new.txt").exists() && root.join("src/renamed.txt").is_file());
    fs::write(root.join("other.txt"), "x").unwrap();
    assert!(rename_entry(root, "other.txt", "src/renamed.txt").unwrap_err().contains("already exists"));
    assert!(rename_entry(root, "src", "src/inner/src").unwrap_err().contains("into itself"));
    assert!(rename_entry(root, "other.txt", "../outside.txt").is_err());
    assert!(rename_entry(root, "", "x").is_err());
    // Case-only rename of the same entry is a real rename, not a collision.
    assert_eq!(rename_entry(root, "other.txt", "Other.txt").unwrap(), "Other.txt");

    assert!(delete_entry(root, "").is_err());
    assert!(delete_entry(root, "../x").is_err());
    assert!(delete_entry(root, "nope.txt").is_err());
    delete_entry(root, "src/renamed.txt").unwrap();
    assert!(!root.join("src/renamed.txt").exists());
}

#[test]
fn discard_restores_tracked_files_and_trashes_untracked_ones() {
    let directory = repository();
    let root = directory.path();
    fs::write(root.join("tracked.txt"), "original\n").unwrap();
    git::git_ok(Some(root), &["add", "tracked.txt"], "stage fixture").unwrap();
    git::git_ok(Some(root), &["commit", "-m", "add tracked"], "commit fixture").unwrap();

    fs::write(root.join("tracked.txt"), "edited\n").unwrap();
    fs::write(root.join("new.txt"), "scratch\n").unwrap();
    discard_paths(root, &["tracked.txt".to_string()], &["new.txt".to_string()]).unwrap();
    // Git may check files out with CRLF (core.autocrlf), so compare the text, not the line ending.
    let text = |name: &str| fs::read_to_string(root.join(name)).unwrap().replace("\r\n", "\n");
    assert_eq!(text("tracked.txt"), "original\n");
    assert!(!root.join("new.txt").exists());

    // Staged work survives a discard of the later working-tree edit.
    fs::write(root.join("tracked.txt"), "staged\n").unwrap();
    git::git_ok(Some(root), &["add", "tracked.txt"], "stage edit").unwrap();
    fs::write(root.join("tracked.txt"), "later\n").unwrap();
    discard_paths(root, &["tracked.txt".to_string()], &[]).unwrap();
    assert_eq!(text("tracked.txt"), "staged\n");

    // A deleted tracked file comes back; escapes are refused.
    fs::remove_file(root.join("tracked.txt")).unwrap();
    discard_paths(root, &["tracked.txt".to_string()], &[]).unwrap();
    assert!(root.join("tracked.txt").is_file());
    assert!(discard_paths(root, &["../x".to_string()], &[]).is_err());
    assert!(discard_paths(root, &[], &["../x".to_string()]).is_err());
}
