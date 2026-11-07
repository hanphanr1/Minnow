use crate::{git, model::{Project, Settings, Snapshot, Worker, STATUS_RUNNING, STATUS_IDLE, STATUS_WAITING_FOR_INPUT, STATUS_EXITED}, process_tree::ProcessTree, storage::Store};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::{collections::{HashMap, VecDeque}, io::{Read, Write}, path::Path, process::{Command, Stdio}, sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Arc, Condvar, Mutex}, thread, time::{Duration, Instant}};
use tauri::{ipc::Channel, AppHandle, Emitter};

const BUFFER_BYTES: usize = 2 * 1024 * 1024;
const IN_FLIGHT_BYTES: usize = 64 * 1024;
const IDLE_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Serialize)]
pub struct TerminalChunk {
    worker_id: String,
    start: u64,
    end: u64,
    data: Vec<u8>,
}

#[derive(Serialize)]
pub struct TerminalSnapshot { data: Vec<u8>, end: u64 }

struct OutputRing { bytes: VecDeque<u8>, end: u64 }

impl OutputRing {
    fn new() -> Self { Self { bytes: VecDeque::new(), end: 0 } }
    fn append(&mut self, data: &[u8]) {
        self.end += data.len() as u64;
        let keep = data.len().min(BUFFER_BYTES);
        let overflow = self.bytes.len().saturating_add(keep).saturating_sub(BUFFER_BYTES);
        self.bytes.drain(..overflow);
        self.bytes.extend(&data[data.len() - keep..]);
    }
    fn from(&self, offset: u64, limit: usize) -> (u64, Vec<u8>) {
        let start = offset.max(self.end - self.bytes.len() as u64).min(self.end);
        let skip = (start - (self.end - self.bytes.len() as u64)) as usize;
        (start, self.bytes.iter().skip(skip).take(limit).copied().collect())
    }
}

struct Subscriber { channel: Channel<TerminalChunk>, sent: u64, acknowledged: u64 }
struct Stream {
    ring: OutputRing,
    subscriber: Option<Subscriber>,
    activity: Instant,
    prompt: PromptDetector,
}

impl Stream {
    fn send_pending(&mut self, id: &str) {
        let Some(sub) = self.subscriber.as_mut() else { return; };
        let outstanding = sub.sent.saturating_sub(sub.acknowledged) as usize;
        if outstanding >= IN_FLIGHT_BYTES || sub.sent == self.ring.end { return; }
        let (start, data) = self.ring.from(sub.sent, IN_FLIGHT_BYTES - outstanding);
        let end = start + data.len() as u64;
        let payload = TerminalChunk { worker_id: id.to_owned(), start, end, data };
        if sub.channel.send(payload).is_err() { self.subscriber = None; }
        else { sub.sent = end; }
    }
}

#[derive(Default)]
struct PromptDetector { text: VecDeque<u8>, escape: u8 }

impl PromptDetector {
    fn push(&mut self, data: &[u8]) {
        for &byte in data {
            match self.escape {
                1 => self.escape = match byte { b'[' => 2, b']' => 3, _ => 0 },
                2 => { if (0x40..=0x7e).contains(&byte) { self.escape = 0; } },
                3 => { if byte == 7 { self.escape = 0; } else if byte == 27 { self.escape = 4; } },
                4 => self.escape = if byte == b'\\' { 0 } else { 3 },
                _ => {
                    if byte == 27 { self.escape = 1; }
                    else if byte == b'\n' || byte == b'\r' || byte.is_ascii_graphic() || byte == b' ' {
                        if self.text.len() == 2048 { self.text.pop_front(); }
                        self.text.push_back(byte.to_ascii_lowercase());
                    }
                }
            }
        }
    }
    fn is_waiting(&mut self) -> bool {
        let text = self.text.make_contiguous();
        [b"[y/n]".as_slice(), b"(y/n)", b"press enter", b"press any key", b"allow this", b"approve this", b"waiting for input", b"permission required"]
            .iter().any(|needle| text.windows(needle.len()).any(|window| window == *needle))
    }
    fn clear(&mut self) { self.text.clear(); self.escape = 0; }
}

#[cfg(windows)]
#[derive(Default)]
struct StartupCursor { matched: usize, finished: bool }

#[cfg(windows)]
impl StartupCursor {
    fn feed<'a>(&mut self, data: &'a [u8]) -> (&'static [u8], &'a [u8], bool) {
        const QUERY: &[u8] = b"\x1b[6n";
        if self.finished { return (&[], data, false); }
        for (index, &byte) in data.iter().enumerate() {
            if byte != QUERY[self.matched] {
                self.finished = true;
                return (&QUERY[..self.matched], &data[index..], false);
            }
            self.matched += 1;
            if self.matched == QUERY.len() {
                self.finished = true;
                return (&[], &data[index + 1..], true);
            }
        }
        (&[], &[], false)
    }
}

struct PtyIo {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    tree: ProcessTree,
}

struct Session {
    worker: Mutex<Worker>,
    io: Mutex<Option<PtyIo>>,
    stream: Mutex<Stream>,
    alive: AtomicBool,
    generation: AtomicU64,
    completed: (Mutex<bool>, Condvar),
}

impl Session {
    fn new(worker: Worker) -> Self {
        Self {
            worker: Mutex::new(worker), io: Mutex::new(None),
            stream: Mutex::new(Stream { ring: OutputRing::new(), subscriber: None, activity: Instant::now(), prompt: PromptDetector::default() }),
            alive: AtomicBool::new(false), generation: AtomicU64::new(0),
            completed: (Mutex::new(true), Condvar::new()),
        }
    }
    fn worker(&self) -> Worker { self.worker.lock().unwrap().clone() }
}

pub struct Manager {
    app: AppHandle,
    store: Store,
    _instance_lock: std::fs::File,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    operations: Mutex<()>,
    shutting_down: AtomicBool,
}

impl Manager {
    pub fn new(app: AppHandle, db: &Path) -> Result<Arc<Self>, String> {
        if let Some(parent) = db.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(windows)]
        { use std::os::windows::fs::OpenOptionsExt; options.share_mode(0); }
        let instance_lock = options.open(db.with_extension("lock"))
            .map_err(|e| format!("Cannot own workspace state (another Minnow instance may be open): {e}"))?;
        let store = Store::open(db)?;
        let sessions = store.workers()?.into_iter().map(|w| (w.id.clone(), Arc::new(Session::new(w)))).collect();
        Ok(Arc::new(Self { app, store, _instance_lock: instance_lock, sessions: Mutex::new(sessions), operations: Mutex::new(()), shutting_down: AtomicBool::new(false) }))
    }

    pub fn start_background(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        thread::spawn(move || {
            let Some(engine) = weak.upgrade() else { return; };
            {
                let _operation = engine.operations.lock().unwrap();
                let sessions: Vec<_> = engine.sessions.lock().unwrap().values().cloned().collect();
                for session in sessions {
                    if engine.shutting_down.load(Ordering::Acquire) { break; }
                    if session.worker().desired_running { engine.launch(&session); }
                }
            }
            drop(engine);
            loop {
                thread::sleep(Duration::from_secs(1));
                let Some(engine) = weak.upgrade() else { break; };
                if engine.shutting_down.load(Ordering::Acquire) { break; }
                let sessions = engine.sessions.lock().unwrap();
                for session in sessions.values() {
                    if !session.alive.load(Ordering::Acquire) { continue; }
                    let mut stream = session.stream.lock().unwrap();
                    if stream.activity.elapsed() >= IDLE_AFTER {
                        let status = if stream.prompt.is_waiting() { STATUS_WAITING_FOR_INPUT } else { STATUS_IDLE };
                        engine.set_status(session, status);
                    }
                }
            }
        });
    }

    fn session(&self, id: &str) -> Result<Arc<Session>, String> {
        self.sessions.lock().unwrap().get(id).cloned().ok_or_else(|| "Worker no longer exists".into())
    }

    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let _operation = self.operations.lock().unwrap();
        let mut workers: Vec<_> = self.sessions.lock().unwrap().values().map(|s| s.worker()).collect();
        workers.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Snapshot { projects: self.store.projects()?, workers, settings: self.store.settings()? })
    }

    pub fn add_project(&self, path: &str) -> Result<Project, String> {
        let _operation = self.operations.lock().unwrap();
        let project = git::inspect_repository(path)?;
        if let Some(existing) = self.store.projects()?.into_iter().find(|p| p.path == project.path) { return Ok(existing); }
        self.store.save_project(&project)?;
        Ok(project)
    }

    pub fn create_workers(self: &Arc<Self>, project_id: &str, count: u32, name: &str) -> Result<Vec<Worker>, String> {
        let _operation = self.operations.lock().unwrap();
        self.require_active()?;
        if !(1..=16).contains(&count) { return Err("Worker count must be between 1 and 16".into()); }
        let project = self.store.project(project_id)?;
        let existing = self.store.workers()?;
        let mut result = Vec::new();
        for index in 0..count {
            let id = uuid::Uuid::new_v4().to_string();
            let worker_name = if name.trim().is_empty() { format!("worker-{}", &id[..6]) }
                else if count == 1 { name.trim().to_owned() } else { format!("{}-{}", name.trim(), index + 1) };
            if existing.iter().any(|w| w.project_id == project_id && w.name == worker_name) {
                return Err(format!("Worker {worker_name} already exists"));
            }
            let worker = match git::create_worktree(&project, &worker_name, &id) {
                Ok(worker) => worker,
                Err(error) => return Err(format!("{error}. {} worker(s) were already created; they remain in the workspace.", result.len())),
            };
            if let Err(error) = self.store.save_worker(&worker) {
                let cleanup = git::delete_worktree(&project, &worker);
                return Err(format!("Could not save worker: {error}. Worktree cleanup: {cleanup:?}"));
            }
            let session = Arc::new(Session::new(worker));
            self.sessions.lock().unwrap().insert(id, session.clone());
            self.launch(&session);
            result.push(session.worker());
        }
        Ok(result)
    }

    fn launch(self: &Arc<Self>, session: &Arc<Session>) {
        let generation = session.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let launch_result = self.spawn(session, generation);
        if let Err(error) = launch_result {
            let mut worker = session.worker.lock().unwrap();
            worker.status = STATUS_EXITED.into(); worker.pid = None; worker.exit_code = None;
            worker.last_error = Some(error); worker.desired_running = false;
            if let Err(error) = self.store.save_worker(&worker) { worker.last_error = Some(format!("{}; persistence: {error}", worker.last_error.as_deref().unwrap_or_default())); }
            let _ = self.app.emit("worker-updated", &*worker);
        }
    }

    fn spawn(self: &Arc<Self>, session: &Arc<Session>, generation: u64) -> Result<(), String> {
        let worker = session.worker();
        let settings = self.store.settings()?;
        let executable = which::which(&settings.omp_path).map_err(|e| format!("Cannot find omp executable '{}': {e}. Set its full path in Settings.", settings.omp_path))?;
        if !Path::new(&worker.path).is_dir() { return Err(format!("Worktree is missing: {}", worker.path)); }
        std::fs::create_dir_all(&worker.session_path).map_err(|e| e.to_string())?;
        let resume = std::fs::read_dir(&worker.session_path).map_err(|e| e.to_string())?
            .filter_map(Result::ok).any(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"));
        let mut command = CommandBuilder::new(executable);
        command.cwd(&worker.path);
        command.args(["--session-dir", &worker.session_path, "--motion", "none", "--spinner-interval", "0", "--no-title"]);
        if resume { command.arg("--continue"); }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let pair = native_pty_system().openpty(PtySize { rows: 28, cols: 100, pixel_width: 0, pixel_height: 0 }).map_err(|e| e.to_string())?;
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let mut child = pair.slave.spawn_command(command).map_err(|e| e.to_string())?;
        drop(pair.slave);
        let Some(pid) = child.process_id() else {
            let _ = child.kill(); let _ = child.wait();
            return Err("PTY did not return a child PID".into());
        };
        let tree = match ProcessTree::attach(pid) {
            Ok(tree) => tree,
            Err(error) => { let _ = child.kill(); let _ = child.wait(); return Err(error); }
        };
        let killer = child.clone_killer();
        *session.io.lock().unwrap() = Some(PtyIo { master: pair.master, writer, killer, tree });
        *session.completed.0.lock().unwrap() = false;
        session.alive.store(true, Ordering::Release);
        {
            let mut stream = session.stream.lock().unwrap();
            stream.activity = Instant::now(); stream.prompt.clear();
            let marker = if resume { b"\r\n[Minnow: resuming session]\r\n".as_slice() } else { b"\r\n[Minnow: starting session]\r\n".as_slice() };
            stream.ring.append(marker); stream.send_pending(&worker.id);
        }
        {
            let mut record = session.worker.lock().unwrap();
            record.status = STATUS_RUNNING.into(); record.pid = Some(pid); record.exit_code = None;
            record.last_error = None; record.desired_running = true;
            if let Err(error) = self.store.save_worker(&record) { record.last_error = Some(format!("Session started, but could not persist resume state: {error}")); }
            let _ = self.app.emit("worker-updated", &*record);
        }
        let weak = Arc::downgrade(self);
        let output_session = session.clone();
        let output_id = worker.id.clone();
        thread::spawn(move || {
            let mut bytes = [0; 16 * 1024];
            #[cfg(windows)]
            let mut startup_cursor = StartupCursor::default();
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(count) => {
                        if output_session.generation.load(Ordering::Acquire) != generation { break; }
                        // ConPTY's inherited-cursor bootstrap must work without an attached xterm.
                        // Consume it once so replay cannot inject a stale cursor report into omp.
                        #[cfg(windows)]
                        let (prefix, data, reply) = startup_cursor.feed(&bytes[..count]);
                        #[cfg(not(windows))]
                        let (prefix, data, reply): (&[u8], &[u8], bool) = (&[], &bytes[..count], false);
                        if reply {
                            let result = {
                                let mut io = output_session.io.lock().unwrap();
                                io.as_mut().ok_or_else(|| "PTY closed during cursor negotiation".to_owned())
                                    .and_then(|io| io.writer.write_all(b"\x1b[1;1R").and_then(|_| io.writer.flush()).map_err(|e| e.to_string()))
                            };
                            if let Err(error) = result {
                                if let Some(engine) = weak.upgrade() {
                                    let mut record = output_session.worker.lock().unwrap();
                                    record.last_error = Some(format!("ConPTY cursor negotiation failed: {error}"));
                                    let _ = engine.app.emit("worker-updated", &*record);
                                }
                            }
                        }
                        if prefix.is_empty() && data.is_empty() { continue; }
                        let mut stream = output_session.stream.lock().unwrap();
                        stream.activity = Instant::now();
                        stream.prompt.push(prefix); stream.prompt.push(data);
                        stream.ring.append(prefix); stream.ring.append(data);
                        stream.send_pending(&output_id);
                        if let Some(engine) = weak.upgrade() {
                            if output_session.alive.load(Ordering::Acquire) { engine.set_status(&output_session, STATUS_RUNNING); }
                        }
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });
        let weak = Arc::downgrade(self);
        let session = session.clone();
        thread::spawn(move || {
            let result = child.wait();
            if session.generation.load(Ordering::Acquire) != generation { return; }
            session.alive.store(false, Ordering::Release);
            // Closing the job also terminates descendants that outlived the omp parent.
            session.io.lock().unwrap().take();
            if let Some(engine) = weak.upgrade() {
                let mut worker = session.worker.lock().unwrap();
                worker.status = STATUS_EXITED.into(); worker.pid = None;
                match result { Ok(status) => worker.exit_code = Some(status.exit_code()), Err(e) => worker.last_error = Some(e.to_string()) }
                // A natural exit is not an instruction to relaunch next time.
                worker.desired_running = false;
                if let Err(e) = engine.store.save_worker(&worker) { worker.last_error = Some(format!("Could not persist exit: {e}")); }
                let _ = engine.app.emit("worker-updated", &*worker);
                if !engine.is_shutting_down() { engine.toast(&worker.name, "Worker exited"); }
            }
            *session.completed.0.lock().unwrap() = true;
            session.completed.1.notify_all();
        });
        Ok(())
    }

    fn set_status(&self, session: &Session, status: &str) {
        let mut worker = session.worker.lock().unwrap();
        if worker.status == status || !session.alive.load(Ordering::Acquire) { return; }
        worker.status = status.into();
        let _ = self.app.emit("worker-updated", &*worker);
        if status == STATUS_IDLE && !self.is_shutting_down() { self.toast(&worker.name, "Worker is idle"); }
    }

    fn toast(&self, name: &str, message: &str) {
        let title = format!("Minnow: {name}");
        let message = message.to_owned();
        thread::spawn(move || {
            if let Err(error) = notify_rust::Notification::new().summary(&title).body(&message).appname("Minnow").show() {
                eprintln!("Windows notification failed: {error}");
            }
        });
    }

    fn stop(&self, session: &Arc<Session>, explicit: bool) -> Result<(), String> {
        if explicit {
            let mut worker = session.worker.lock().unwrap();
            worker.desired_running = false;
            self.store.save_worker(&worker)?;
        }
        let was_alive = session.alive.load(Ordering::Acquire);
        if was_alive {
            if let Some(io) = session.io.lock().unwrap().as_mut() {
                #[cfg(windows)]
                io.tree.kill().or_else(|_| io.killer.kill().map_err(|e| e.to_string()))?;
                #[cfg(not(windows))]
                io.killer.kill().map_err(|e| e.to_string())?;
            }
        }
        let finished = session.completed.0.lock().unwrap();
        let (finished, timeout) = session.completed.1.wait_timeout_while(finished, Duration::from_secs(10), |done| !*done).unwrap();
        if timeout.timed_out() && !*finished { return Err("Timed out waiting for worker process to exit; worktree retained".into()); }
        Ok(())
    }

    pub fn kill(&self, id: &str) -> Result<Worker, String> {
        let _operation = self.operations.lock().unwrap();
        let session = self.session(id)?;
        self.stop(&session, true)?;
        Ok(session.worker())
    }

    pub fn restart(self: &Arc<Self>, id: &str) -> Result<Worker, String> {
        let _operation = self.operations.lock().unwrap(); self.require_active()?;
        let session = self.session(id)?;
        self.stop(&session, true)?;
        self.launch(&session);
        Ok(session.worker())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let _operation = self.operations.lock().unwrap();
        let session = self.session(id)?;
        if session.alive.load(Ordering::Acquire) { return Err("Kill the worker before deleting its worktree".into()); }
        let worker = session.worker();
        git::delete_worktree(&self.store.project(&worker.project_id)?, &worker)?;
        self.store.delete_worker(id)?;
        self.sessions.lock().unwrap().remove(id);
        let _ = self.app.emit("worker-removed", id);
        Ok(())
    }

    pub fn diff(&self, id: &str) -> Result<String, String> { git::diff_worker(&self.session(id)?.worker()) }

    pub fn merge(&self, id: &str) -> Result<String, String> {
        let _operation = self.operations.lock().unwrap();
        let session = self.session(id)?;
        if session.alive.load(Ordering::Acquire) { return Err("Kill the worker before merging its committed changes".into()); }
        let worker = session.worker();
        git::merge_worker(&self.store.project(&worker.project_id)?, &worker)
    }

    pub fn open_editor(&self, id: &str) -> Result<(), String> {
        let worker = self.session(id)?.worker();
        let executable = which::which("code").map_err(|_| "VS Code's 'code' launcher was not found on PATH".to_owned())?;
        // Windows' code.cmd is a shell script; use the sibling native editor executable instead.
        #[cfg(windows)]
        let executable = if executable.extension().is_some_and(|ext| ext == "cmd") {
            executable.parent().and_then(Path::parent).map(|p| p.join("Code.exe")).filter(|p| p.is_file()).ok_or("Could not locate Code.exe beside code.cmd")?
        } else { executable };
        let mut command = Command::new(executable);
        command.arg("--new-window").arg(&worker.path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
        let mut child = command.spawn().map_err(|e| format!("Could not open VS Code: {e}"))?;
        thread::spawn(move || { let _ = child.wait(); });
        Ok(())
    }

    pub fn save_settings(&self, settings: Settings) -> Result<Settings, String> {
        if settings.omp_path.trim().is_empty() { return Err("Set an omp executable path or command name".into()); }
        if !(1..=16).contains(&settings.default_worker_count) { return Err("Default worker count must be between 1 and 16".into()); }
        self.store.save_settings(&settings)?;
        Ok(settings)
    }

    pub fn attach(&self, id: &str, cols: u16, rows: u16, channel: Channel<TerminalChunk>) -> Result<TerminalSnapshot, String> {
        let session = self.session(id)?;
        self.resize_session(&session, cols, rows)?;
        let mut stream = session.stream.lock().unwrap();
        let data = stream.ring.bytes.iter().copied().collect();
        let end = stream.ring.end;
        stream.subscriber = Some(Subscriber { channel, sent: end, acknowledged: end });
        Ok(TerminalSnapshot { data, end })
    }
    pub fn detach(&self, id: &str) -> Result<(), String> {
        self.session(id)?.stream.lock().unwrap().subscriber = None;
        Ok(())
    }
    pub fn ack(&self, id: &str, end: u64) -> Result<(), String> {
        let session = self.session(id)?;
        let mut stream = session.stream.lock().unwrap();
        if let Some(sub) = stream.subscriber.as_mut() { sub.acknowledged = end.min(sub.sent).max(sub.acknowledged); }
        stream.send_pending(id);
        Ok(())
    }
    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let session = self.session(id)?;
        self.resize_session(&session, cols, rows)
    }
    fn resize_session(&self, session: &Session, cols: u16, rows: u16) -> Result<(), String> {
        let cols = cols.clamp(2, 500); let rows = rows.clamp(2, 300);
        if let Some(io) = session.io.lock().unwrap().as_ref() {
            io.master.resize(PtySize { cols, rows, pixel_width: 0, pixel_height: 0 }).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn write(&self, id: &str, data: &str) -> Result<(), String> {
        let session = self.session(id)?;
        {
            let mut io = session.io.lock().unwrap();
            let io = io.as_mut().ok_or("Worker has exited; restart it before typing")?;
            io.writer.write_all(data.as_bytes()).and_then(|_| io.writer.flush()).map_err(|e| e.to_string())?;
        }
        let mut stream = session.stream.lock().unwrap();
        stream.activity = Instant::now(); stream.prompt.clear();
        self.set_status(&session, STATUS_RUNNING);
        Ok(())
    }
    fn require_active(&self) -> Result<(), String> { if self.is_shutting_down() { Err("Application is shutting down".into()) } else { Ok(()) } }
    pub fn is_shutting_down(&self) -> bool { self.shutting_down.load(Ordering::Acquire) }
    pub fn shutdown(&self) {
        if self.shutting_down.swap(true, Ordering::AcqRel) { return; }
        let _operation = self.operations.lock().unwrap();
        let sessions: Vec<_> = self.sessions.lock().unwrap().values().cloned().collect();
        for session in sessions {
            let resume = session.worker().desired_running;
            if let Err(error) = self.stop(&session, false) { eprintln!("Shutdown: {error}"); }
            // The exit waiter clears desired_running. Restore only workers active at close.
            if resume {
                let mut worker = session.worker.lock().unwrap(); worker.desired_running = true;
                if let Err(error) = self.store.save_worker(&worker) { eprintln!("Saving resume state: {error}"); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_ring_discards_old_bytes_without_losing_offsets() {
        let mut ring = OutputRing::new();
        ring.append(b"old");
        ring.append(&vec![b'x'; BUFFER_BYTES + 17]);
        assert_eq!(ring.bytes.len(), BUFFER_BYTES);
        let (start, tail) = ring.from(0, 32);
        assert_eq!(start, 20); assert_eq!(tail, vec![b'x'; 32]);
        let (start, tail) = ring.from(ring.end - 2, 32);
        assert_eq!(start, ring.end - 2); assert_eq!(tail, b"xx");
    }
    #[test]
    fn prompt_detector_handles_split_ansi_and_clears_on_input() {
        let mut prompt = PromptDetector::default();
        prompt.push(b"\x1b[3"); prompt.push(b"3mPermission required\x1b[0m\r\n[y/n]");
        assert!(prompt.is_waiting());
        prompt.clear(); prompt.push(b"ordinary model response");
        assert!(!prompt.is_waiting());
    }
    #[cfg(windows)]
    #[test]
    fn startup_cursor_query_is_consumed_once_across_reads() {
        let mut startup = StartupCursor::default();
        assert_eq!(startup.feed(b"\x1b["), (&b""[..], &b""[..], false));
        assert_eq!(startup.feed(b"6nready"), (&b""[..], &b"ready"[..], true));
        assert_eq!(startup.feed(b"\x1b[6n"), (&b""[..], &b"\x1b[6n"[..], false));
    }
    #[cfg(windows)]
    #[test]
    fn non_bootstrap_escape_bytes_are_not_discarded() {
        let mut startup = StartupCursor::default();
        let _ = startup.feed(b"\x1b[");
        assert_eq!(startup.feed(b"?25l"), (&b"\x1b["[..], &b"?25l"[..], false));
    }
}
