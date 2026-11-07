mod git;
mod model;
mod process_tree;
mod pty;
mod storage;
mod projects;
mod workspace;
mod workspace_watch;

use model::{Project, Settings, Snapshot, Worker};
use pty::{Manager as WorkerManager, TerminalChunk, TerminalSnapshot};
use std::{path::PathBuf, sync::Arc};
use tauri::{ipc::Channel, Manager, State};

type Engine = Arc<WorkerManager>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

async fn in_workspace<T: Send + 'static>(
    engine: Engine,
    worker_id: String,
    operation: impl FnOnce(&std::path::Path) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    blocking(move || operation(&engine.worktree_path(&worker_id)?)).await
}

#[tauri::command]
async fn pick_project_folder(window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    blocking(move || {
        window.app_handle().dialog().file()
            .set_parent(&window)
            .set_title("Choose project folder")
            .blocking_pick_folder()
            .map(|selected| {
                selected.into_path().map_err(|error| error.to_string())?
                    .into_os_string().into_string()
                    .map_err(|_| "The selected folder is not a Unicode path".to_string())
            }).transpose()
    }).await
}

#[tauri::command]
async fn create_project(engine: State<'_, Engine>, parent_path: String, name: String) -> Result<Project, String> {
    let engine = engine.inner().clone();
    blocking(move || {
        let path = projects::create(&parent_path, &name)?;
        engine.add_project(path.to_str().ok_or("The project path is not Unicode")?)
    }).await
}

#[tauri::command]
async fn clone_project(engine: State<'_, Engine>, parent_path: String, name: String, url: String) -> Result<Project, String> {
    let engine = engine.inner().clone();
    blocking(move || {
        let path = projects::clone(&parent_path, &name, &url)?;
        engine.add_project(path.to_str().ok_or("The project path is not Unicode")?)
    }).await
}

#[tauri::command]
async fn list_workspace_directory(engine: State<'_, Engine>, worker_id: String, path: String) -> Result<workspace::DirectoryListing, String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::list_directory(root, &path)).await
}

#[tauri::command]
async fn read_workspace_file(engine: State<'_, Engine>, worker_id: String, path: String) -> Result<workspace::FileDocument, String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::read_file(root, &path)).await
}

#[tauri::command]
async fn write_workspace_file(engine: State<'_, Engine>, worker_id: String, path: String, content: String, revision: String) -> Result<workspace::FileDocument, String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::write_file(root, &path, &content, &revision)).await
}

#[tauri::command]
async fn search_workspace(engine: State<'_, Engine>, worker_id: String, query: String, case_sensitive: bool) -> Result<workspace::SearchResults, String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::search(root, &query, case_sensitive)).await
}

#[tauri::command]
async fn workspace_git_status(engine: State<'_, Engine>, worker_id: String) -> Result<workspace::GitStatus, String> {
    in_workspace(engine.inner().clone(), worker_id, workspace::git_status).await
}

#[tauri::command]
async fn workspace_git_history(engine: State<'_, Engine>, worker_id: String, limit: usize) -> Result<Vec<workspace::GitCommit>, String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::git_history(root, limit)).await
}

#[tauri::command]
async fn stage_workspace_paths(engine: State<'_, Engine>, worker_id: String, paths: Option<Vec<String>>) -> Result<(), String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::stage_paths(root, paths)).await
}

#[tauri::command]
async fn unstage_workspace_paths(engine: State<'_, Engine>, worker_id: String, paths: Option<Vec<String>>) -> Result<(), String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::unstage_paths(root, paths)).await
}

#[tauri::command]
async fn commit_workspace(engine: State<'_, Engine>, worker_id: String, message: String) -> Result<String, String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::commit(root, &message)).await
}

#[tauri::command]
async fn workspace_file_diff(engine: State<'_, Engine>, worker_id: String, path: String, staged: bool) -> Result<String, String> {
    in_workspace(engine.inner().clone(), worker_id, move |root| workspace::file_diff(root, &path, staged)).await
}

#[tauri::command]
async fn watch_workspace(
    engine: State<'_, Engine>,
    watches: State<'_, Arc<workspace_watch::WorkspaceWatches>>,
    window: tauri::WebviewWindow,
    worker_id: Option<String>,
    lease: String,
) -> Result<(), String> {
    let engine = engine.inner().clone();
    let watches = watches.inner().clone();
    let app = window.app_handle().clone();
    let label = window.label().to_owned();
    blocking(move || {
        if let Some(id) = worker_id {
            let root = engine.worktree_path(&id)?;
            watches.start(app, label, lease, id, &root)
        } else {
            watches.stop(&label, &lease);
            Ok(())
        }
    }).await
}

#[tauri::command]
async fn bootstrap(engine: State<'_, Engine>) -> Result<Snapshot, String> {
    let engine = engine.inner().clone();
    blocking(move || engine.snapshot()).await
}

#[tauri::command]
async fn add_project(engine: State<'_, Engine>, path: String) -> Result<Project, String> {
    let engine = engine.inner().clone();
    blocking(move || engine.add_project(&path)).await
}

#[tauri::command]
async fn create_workers(engine: State<'_, Engine>, project_id: String, count: u32, name: String) -> Result<Vec<Worker>, String> {
    let engine = engine.inner().clone();
    blocking(move || engine.create_workers(&project_id, count, &name)).await
}

#[tauri::command]
async fn restart_worker(engine: State<'_, Engine>, worker_id: String) -> Result<Worker, String> {
    let engine = engine.inner().clone();
    blocking(move || engine.restart(&worker_id)).await
}

#[tauri::command]
async fn kill_worker(engine: State<'_, Engine>, worker_id: String) -> Result<Worker, String> {
    let engine = engine.inner().clone();
    blocking(move || engine.kill(&worker_id)).await
}

#[tauri::command]
async fn delete_worker(engine: State<'_, Engine>, worker_id: String) -> Result<(), String> {
    let engine = engine.inner().clone();
    blocking(move || engine.delete(&worker_id)).await
}

#[tauri::command]
async fn diff_worker(engine: State<'_, Engine>, worker_id: String) -> Result<String, String> {
    let engine = engine.inner().clone();
    blocking(move || engine.diff(&worker_id)).await
}

#[tauri::command]
async fn merge_worker(engine: State<'_, Engine>, worker_id: String) -> Result<String, String> {
    let engine = engine.inner().clone();
    blocking(move || engine.merge(&worker_id)).await
}

#[tauri::command]
async fn open_editor(engine: State<'_, Engine>, worker_id: String) -> Result<(), String> {
    let engine = engine.inner().clone();
    blocking(move || engine.open_editor(&worker_id)).await
}

#[tauri::command]
async fn save_settings(engine: State<'_, Engine>, settings: Settings) -> Result<Settings, String> {
    let engine = engine.inner().clone();
    blocking(move || engine.save_settings(settings)).await
}

#[tauri::command]
fn attach_terminal(engine: State<'_, Engine>, worker_id: String, cols: u16, rows: u16, on_output: Channel<TerminalChunk>) -> Result<TerminalSnapshot, String> {
    engine.attach(&worker_id, cols, rows, on_output)
}

#[tauri::command]
fn detach_terminal(engine: State<'_, Engine>, worker_id: String) -> Result<(), String> {
    engine.detach(&worker_id)
}

#[tauri::command]
fn resize_terminal(engine: State<'_, Engine>, worker_id: String, cols: u16, rows: u16) -> Result<(), String> {
    engine.resize(&worker_id, cols, rows)
}

#[tauri::command]
async fn write_terminal(engine: State<'_, Engine>, worker_id: String, data: String) -> Result<(), String> {
    let engine = engine.inner().clone();
    blocking(move || engine.write(&worker_id, &data)).await
}

#[tauri::command]
fn ack_terminal(engine: State<'_, Engine>, worker_id: String, end: u64) -> Result<(), String> {
    engine.ack(&worker_id, end)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut context = tauri::generate_context!();
    let isolated_windows = std::env::var_os("MINNOW_DATA_DIR").map(|directory| {
        let directory = std::path::absolute(directory).expect("Cannot resolve the isolated state directory");
        (std::mem::take(&mut context.config_mut().app.windows), directory.join("webview"))
    });
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let directory = std::env::var_os("MINNOW_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_local_data_dir()?);
            let engine = WorkerManager::new(app.handle().clone(), &directory.join("workspace.sqlite"))?;
            app.manage(engine.clone());
            app.manage(Arc::new(workspace_watch::WorkspaceWatches::default()));
            engine.start_background();
            if let Some((windows, directory)) = isolated_windows {
                for config in windows {
                    // Tauri 2.12's config-to-runtime conversion omits dataDirectory.
                    // The builder setter isolates WebView2 as well as SQLite.
                    tauri::WebviewWindowBuilder::from_config(app, &config)?
                        .data_directory(directory.clone())
                        .build()?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap, add_project, create_workers, restart_worker, kill_worker,
            delete_worker, diff_worker, merge_worker, open_editor, save_settings,
            attach_terminal, detach_terminal, resize_terminal, write_terminal, ack_terminal,
            pick_project_folder, create_project, clone_project,
            list_workspace_directory, read_workspace_file, write_workspace_file, search_workspace,
            workspace_git_status, workspace_git_history, stage_workspace_paths,
            unstage_workspace_paths, commit_workspace, workspace_file_diff, watch_workspace
        ])
        .build(context)
        .expect("Could not start Minnow");
    app.run(|handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            let engine = handle.state::<Engine>().inner().clone();
            if !engine.is_shutting_down() {
                api.prevent_exit();
                let handle = handle.clone();
                std::thread::spawn(move || {
                    engine.shutdown();
                    handle.exit(0);
                });
            }
        }
    });
}
