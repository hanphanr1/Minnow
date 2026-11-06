mod git;
mod model;
mod process_tree;
mod pty;
mod storage;

use model::{Project, Settings, Snapshot, Worker};
use pty::{Manager as WorkerManager, TerminalChunk, TerminalSnapshot};
use std::{path::PathBuf, sync::Arc};
use tauri::{ipc::Channel, Manager, State};

type Engine = Arc<WorkerManager>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
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
    let app = tauri::Builder::default()
        .setup(|app| {
            let directory = std::env::var_os("MINNOW_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_local_data_dir()?);
            let engine = WorkerManager::new(app.handle().clone(), &directory.join("workspace.sqlite"))?;
            app.manage(engine.clone());
            engine.start_background();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap, add_project, create_workers, restart_worker, kill_worker,
            delete_worker, diff_worker, merge_worker, open_editor, save_settings,
            attach_terminal, detach_terminal, resize_terminal, write_terminal, ack_terminal
        ])
        .build(tauri::generate_context!())
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
