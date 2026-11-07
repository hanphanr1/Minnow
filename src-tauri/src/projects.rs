use crate::git;
use std::{fs, path::{Path, PathBuf}};

fn directory_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if name.is_empty() || name == "." || name == ".." || name.len() > 120
        || name.ends_with('.') || name.ends_with(' ') || reserved
        || name.eq_ignore_ascii_case(".git") || name.eq_ignore_ascii_case(".minnow")
        || name.chars().any(|c| c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err("Choose a valid project folder name without path separators or reserved Windows names".into());
    }
    Ok(name)
}

fn destination(parent: &str, name: &str) -> Result<PathBuf, String> {
    let parent = git::display_path(&fs::canonicalize(parent.trim()).map_err(|e| format!("Cannot open parent folder: {e}"))?);
    if !parent.is_dir() { return Err("Project parent must be a directory".into()); }
    let path = parent.join(directory_name(name)?);
    if fs::symlink_metadata(&path).is_ok() {
        return Err(format!("{} already exists; choose another project name", path.display()));
    }
    Ok(path)
}

pub fn create(parent: &str, name: &str) -> Result<PathBuf, String> {
    let path = destination(parent, name)?;
    fs::create_dir(&path).map_err(|e| format!("Cannot create project folder: {e}"))?;
    let result = (|| {
        git::git_ok(Some(&path), &["init", "--initial-branch=main"], "Initialize repository")?;
        git::git_ok(Some(&path), &["commit", "--allow-empty", "-m", "Initial commit"], "Create initial commit")?;
        Ok::<_, String>(())
    })();
    result.map_err(|error| format!("{error}. The new folder at {} was preserved. Configure your Git identity if needed, then finish initialization there.", path.display()))?;
    Ok(path)
}

pub fn clone(parent: &str, name: &str, source: &str) -> Result<PathBuf, String> {
    let source = source.trim();
    if source.is_empty() || source.chars().any(char::is_control) || source.starts_with('-') {
        return Err("Enter a Git repository URL or an existing local repository path".into());
    }
    let network = ["https://", "http://", "ssh://", "git://"].iter().any(|prefix| source.starts_with(prefix));
    let scp = source.split_once(':').is_some_and(|(host, path)| {
        host.contains('@') && !host.contains(['/', '\\', ' ']) && !path.is_empty()
    });
    let local = Path::new(source).is_absolute() && Path::new(source).is_dir();
    if !network && !scp && !local {
        return Err("Use an HTTP(S), SSH or Git URL, or an absolute local repository path".into());
    }
    let path = destination(parent, name)?;
    let destination = path.to_str().ok_or("The destination is not a Unicode path")?;
    git::git_ok(None, &["-c", "protocol.ext.allow=never", "clone", "--", source, destination], "Clone repository")?;
    Ok(path)
}
