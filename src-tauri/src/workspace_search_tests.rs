use super::*;
use std::fs;
use std::path::Path;

/// A temporary work tree with a repository-local ignore configuration. The
/// local `core.excludesFile` keeps the developer's global Git configuration out
/// of every assertion without writing anything outside the temporary folder.
fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    crate::git::git_ok(
        Some(root),
        &["init", "--initial-branch=main"],
        "initialize search fixture",
    )
    .unwrap();
    crate::git::git_ok(
        Some(root),
        &["config", "user.name", "Minnow Tests"],
        "configure search fixture",
    )
    .unwrap();
    crate::git::git_ok(
        Some(root),
        &["config", "user.email", "tests@minnow.invalid"],
        "configure search fixture",
    )
    .unwrap();
    let global = root.join("fixture-global-ignore");
    fs::write(&global, "").unwrap();
    crate::git::git_ok(
        Some(root),
        &[
            "config",
            "core.excludesFile",
            global.to_str().expect("temporary paths are Unicode"),
        ],
        "configure search fixture",
    )
    .unwrap();
    directory
}

fn write(root: &Path, path: &str, content: &str) {
    let full = root.join(path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, content).unwrap();
}

fn search_paths(root: &Path, query: &str) -> Vec<String> {
    search(root, query, true)
        .unwrap()
        .matches
        .into_iter()
        .map(|hit| hit.path)
        .collect()
}

fn expected(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| (*path).to_string()).collect()
}

#[test]
fn nested_anchored_and_negated_rules_keep_git_precedence() {
    let directory = repository();
    let root = directory.path();
    write(
        root,
        ".gitignore",
        "*.tmp\n!keep.tmp\n*.data\ntarget/\nignored-dir/\n!ignored-dir/keep.txt\n",
    );
    write(
        root,
        "nested/.gitignore",
        "/local.txt\n!local.keep.txt\n!keep.data\n",
    );
    write(root, "root.txt", "needle\n");
    write(root, "drop.tmp", "needle\n");
    write(root, "keep.tmp", "needle\n");
    write(root, "keep.data", "needle\n");
    write(root, "target/inside.txt", "needle\n");
    write(root, "ignored-dir/keep.txt", "needle\n");
    write(root, "nested/local.txt", "needle\n");
    write(root, "nested/local.keep.txt", "needle\n");
    write(root, "nested/keep.data", "needle\n");
    write(root, "nested/other.txt", "needle\n");
    write(root, "nested/deeper/local.txt", "needle\n");

    assert_eq!(
        search_paths(root, "needle"),
        expected(&[
            "keep.tmp",
            "nested/deeper/local.txt",
            "nested/keep.data",
            "nested/local.keep.txt",
            "nested/other.txt",
            "root.txt",
        ])
    );
}

#[test]
fn ignore_files_outrank_git_exclude_and_global_excludes() {
    let directory = repository();
    let root = directory.path();
    fs::write(
        root.join("fixture-global-ignore"),
        "from-global.txt\nglobalonly.txt\n",
    )
    .unwrap();
    write(
        root,
        ".gitignore",
        "!from-exclude.txt\n!from-global.txt\ngitonly.txt\n",
    );
    write(root, ".ignore", "!gitonly.txt\n");
    write(root, ".git/info/exclude", "from-exclude.txt\nexcluded.txt\n");
    for name in [
        "gitonly.txt",
        "excluded.txt",
        "from-exclude.txt",
        "from-global.txt",
        "globalonly.txt",
        "visible.txt",
    ] {
        write(root, name, "needle\n");
    }

    assert_eq!(
        search_paths(root, "needle"),
        expected(&[
            "from-exclude.txt",
            "from-global.txt",
            "gitonly.txt",
            "visible.txt",
        ])
    );
}

#[test]
fn oversized_ignore_files_are_explicit_errors() {
    let directory = repository();
    let root = directory.path();

    write(
        root,
        ".gitignore",
        &"x".repeat(MAX_IGNORE_FILE_BYTES as usize + 1),
    );
    let error = search(root, "needle", true)
        .err()
        .expect("an oversized ignore file must fail the search");
    assert!(error.contains(".gitignore"), "{error}");
    assert!(error.contains("exceeds"), "{error}");

    fs::remove_file(root.join(".gitignore")).unwrap();
    write(
        root,
        ".gitignore",
        &format!("{}\n", "y".repeat(MAX_IGNORE_RULE_BYTES + 1)),
    );
    let error = search(root, "needle", true)
        .err()
        .expect("an oversized ignore rule must fail the search");
    assert!(error.contains("longer than"), "{error}");

    fs::remove_file(root.join(".gitignore")).unwrap();
    write(
        root,
        ".ignore",
        &"z".repeat(MAX_IGNORE_FILE_BYTES as usize + 1),
    );
    let error = search(root, "needle", true)
        .err()
        .expect("an oversized .ignore file must fail the search");
    assert!(error.contains(".ignore"), "{error}");
    assert!(error.contains("exceeds"), "{error}");
}

#[cfg(unix)]
fn link_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn link_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}

#[cfg(unix)]
fn link_symlink_dir(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn link_symlink_dir(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, link).is_ok()
}

#[test]
fn search_skips_git_minnow_and_symlink_entries() {
    let directory = repository();
    let root = directory.path();
    write(root, "visible.txt", "needle\n");
    write(root, ".git/needle-probe.txt", "needle\n");
    write(root, ".minnow/needle-probe.txt", "needle\n");
    write(root, ".minnow-save-123/needle-probe.txt", "needle\n");

    assert_eq!(search_paths(root, "needle"), expected(&["visible.txt"]));

    // Symlink creation needs extra privileges on some Windows hosts. When it
    // works, the link target must still never be read or listed. Targets live
    // in a second temporary directory so they are genuinely outside the root.
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("outside-needle.txt");
    fs::write(&outside_file, "needle\n").unwrap();
    if link_symlink_file(&outside_file, &root.join("linked.txt")) {
        assert_eq!(search_paths(root, "needle"), expected(&["visible.txt"]));
    }

    let outside_dir = outside.path().join("outside-needle-dir");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(outside_dir.join("nested-needle.txt"), "needle\n").unwrap();
    if link_symlink_dir(&outside_dir, &root.join("linked-dir")) {
        assert_eq!(search_paths(root, "needle"), expected(&["visible.txt"]));
    }
}

#[test]
fn literal_search_reports_unicode_columns_clips_lines_and_skips_binary() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let long = format!("{}needle\n", "z".repeat(450));
    write(
        root,
        "notes.txt",
        &format!("héllo needle\nsecond needle needle\n{long}"),
    );
    write(root, "binary.bin", "needle\0needle");

    let result = search(root, "needle", true).unwrap();
    let positions: Vec<(u32, u32)> = result
        .matches
        .iter()
        .map(|hit| (hit.line, hit.column))
        .collect();
    assert_eq!(positions, vec![(1, 7), (2, 8), (2, 15), (3, 451)]);
    assert_eq!(result.matches[3].text.chars().count(), 401);
    assert!(result.matches[3].text.ends_with('…'));
    assert!(!result.truncated);

    let insensitive = search(root, "NEEDLE", false).unwrap();
    assert_eq!(insensitive.matches.len(), 4);

    let empty = search(root, "", true)
        .err()
        .expect("an empty query must be rejected");
    assert!(empty.contains("empty"));
}
