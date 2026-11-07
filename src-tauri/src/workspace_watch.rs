use crate::git;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::{collections::HashMap, path::Path, sync::{mpsc, Mutex}, thread, time::{Duration, Instant}};
use tauri::{AppHandle, Emitter, Manager};

const COALESCE: Duration = Duration::from_millis(500);

struct WindowWatch {
    lease: String,
    _watcher: RecommendedWatcher,
}

#[derive(Default)]
pub struct WorkspaceWatches {
    windows: Mutex<HashMap<String, WindowWatch>>,
}

#[derive(Clone, Serialize)]
struct Changed { worker_id: String }

impl WorkspaceWatches {
    pub fn stop(&self, window: &str, lease: &str) {
        let mut windows = self.windows.lock().unwrap();
        if windows.get(window).is_some_and(|watch| watch.lease == lease) {
            windows.remove(window);
        }
    }

    pub fn start(&self, app: AppHandle, window: String, lease: String, worker_id: String, root: &Path) -> Result<(), String> {
        let root = git::canonical_existing_dir(root, "workspace root")?;
        let git_directory = git::git_ok(Some(&root), &["rev-parse", "--absolute-git-dir"], "Locate workspace Git metadata")?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            let changed = result.map_or(true, |event| !matches!(event.kind, EventKind::Access(_)));
            if changed { let _ = sender.try_send(()); }
        }).map_err(|error| format!("Cannot watch workspace files: {error}"))?;
        watcher.watch(&root, RecursiveMode::Recursive)
            .map_err(|error| format!("Cannot watch workspace files: {error}"))?;
        watcher.watch(Path::new(git_directory.trim()), RecursiveMode::NonRecursive)
            .map_err(|error| format!("Cannot watch workspace Git index: {error}"))?;
        let payload = Changed { worker_id };
        let target_window = window.clone();
        thread::Builder::new().name("workspace-changes".into()).spawn(move || {
            while receiver.recv().is_ok() {
                let deadline = Instant::now() + COALESCE;
                loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() { break; }
                    match receiver.recv_timeout(remaining) {
                        Ok(()) => continue,
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                if let Some(window) = app.get_webview_window(&target_window) {
                    if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(true) {
                        let _ = app.emit_to(target_window.as_str(), "workspace-files-changed", &payload);
                    }
                }
            }
        }).map_err(|error| format!("Cannot start the workspace change observer: {error}"))?;
        self.windows.lock().unwrap().insert(window, WindowWatch { lease, _watcher: watcher });
        Ok(())
    }
}
