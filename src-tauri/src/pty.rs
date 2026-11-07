use crate::{git, model::{Project, Settings, Snapshot, TerminalSession, Worker, KIND_AGENT, KIND_SHELL, STATUS_RUNNING, STATUS_IDLE, STATUS_WAITING_FOR_INPUT, STATUS_EXITED}, process_tree::ProcessTree, storage::Store};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::{collections::{HashMap, VecDeque}, io::{Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Arc, Condvar, Mutex}, thread, time::{Duration, Instant}};
use tauri::{ipc::Channel, AppHandle, Emitter};

const BUFFER_BYTES: usize = 2 * 1024 * 1024;
const IN_FLIGHT_BYTES: usize = 64 * 1024;
const IDLE_AFTER: Duration = Duration::from_secs(3);
/// Longest OSC 7 payload the parser keeps; longer sequences are ignored whole.
const OSC7_MAX_BYTES: usize = 4096;

#[derive(Clone, Serialize)]
pub struct TerminalChunk {
    terminal_id: String,
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
    /// Releases the buffered bytes but keeps offsets monotonic for later output.
    fn release(&mut self) {
        self.bytes = VecDeque::new();
    }
}

struct Subscriber { channel: Channel<TerminalChunk>, sent: u64, acknowledged: u64 }
struct Stream {
    ring: OutputRing,
    subscriber: Option<Subscriber>,
    activity: Instant,
    prompt: PromptDetector,
    osc7: Osc7Parser,
}

impl Stream {
    fn send_pending(&mut self, id: &str) {
        let Some(sub) = self.subscriber.as_mut() else { return; };
        let outstanding = sub.sent.saturating_sub(sub.acknowledged) as usize;
        if outstanding >= IN_FLIGHT_BYTES || sub.sent == self.ring.end { return; }
        let (start, data) = self.ring.from(sub.sent, IN_FLIGHT_BYTES - outstanding);
        let end = start + data.len() as u64;
        let payload = TerminalChunk { terminal_id: id.to_owned(), start, end, data };
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

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum OscState { #[default] Ground, Esc, Osc, OscEsc }

/// Streaming observer for OSC 7 (`ESC ] 7 ; file://host/path BEL|ST`).
///
/// It only reads bytes; the terminal output is stored and forwarded untouched.
/// State survives chunk boundaries, payloads are bounded, and only the last
/// well-formed directory in a chunk is reported.
#[derive(Default)]
struct Osc7Parser { state: OscState, payload: Vec<u8>, overflow: bool }

impl Osc7Parser {
    fn feed(&mut self, data: &[u8], unc_host: Option<&str>) -> Option<String> {
        let mut found = None;
        for &byte in data {
            match self.state {
                OscState::Ground => if byte == 0x1b { self.state = OscState::Esc; },
                OscState::Esc => match byte {
                    b']' => self.start_osc(),
                    0x1b => {},
                    _ => self.state = OscState::Ground,
                },
                OscState::Osc => match byte {
                    0x07 => { found = self.finish(unc_host).or(found); self.state = OscState::Ground; },
                    0x1b => self.state = OscState::OscEsc,
                    _ => if !self.overflow {
                        if self.payload.len() >= OSC7_MAX_BYTES { self.overflow = true; self.payload.clear(); }
                        else { self.payload.push(byte); }
                    },
                },
                OscState::OscEsc => match byte {
                    b'\\' => { found = self.finish(unc_host).or(found); self.state = OscState::Ground; },
                    b']' => self.start_osc(),
                    0x1b => {},
                    _ => self.state = OscState::Ground,
                },
            }
        }
        found
    }
    fn start_osc(&mut self) { self.state = OscState::Osc; self.payload.clear(); self.overflow = false; }
    fn finish(&mut self, unc_host: Option<&str>) -> Option<String> {
        if self.overflow { return None; }
        parse_osc7_payload(&self.payload, unc_host)
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte { b'0'..=b'9' => Some(byte - b'0'), b'a'..=b'f' => Some(byte - b'a' + 10), b'A'..=b'F' => Some(byte - b'A' + 10), _ => None }
}

/// Mirrors Orca's `parseOsc7`: a `file://` URI whose path is percent-decoded.
/// Windows drive paths lose the URI's leading slash. A non-drive path is
/// accepted as a UNC share only when its host is the launch UNC server.
fn parse_osc7_payload(payload: &[u8], unc_host: Option<&str>) -> Option<String> {
    let text = std::str::from_utf8(payload.strip_prefix(b"7;")?).ok()?;
    let uri = text.strip_prefix("file://")?;
    let slash = uri.find('/')?;
    let (host, raw) = (&uri[..slash], uri[slash..].as_bytes());
    let mut decoded = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            if let (Some(high), Some(low)) = (raw.get(index + 1).and_then(|b| hex_value(*b)), raw.get(index + 2).and_then(|b| hex_value(*b))) {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(raw[index]);
        index += 1;
    }
    let path = String::from_utf8(decoded).ok()?;
    if path.is_empty() || path.contains('\0') { return None; }
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':';
    if let Some(unc) = unc_host {
        if !drive && host.eq_ignore_ascii_case(unc) {
            return Some(format!(r"\\{host}{}", path.replace('/', r"\")));
        }
    }
    if drive {
        let mut local = path[1..].replace('/', r"\");
        if local.len() == 2 { local.push('\\'); }
        return Some(local);
    }
    Some(path)
}

/// The server of a `\\server\share` launch directory, if it is one.
fn unc_host_of(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    let rest = text.strip_prefix(r"\\").filter(|rest| !rest.starts_with('?') && !rest.starts_with('.'))?;
    let host = rest.split('\\').next().filter(|host| !host.is_empty())?;
    Some(host.to_owned())
}

/// First existing directory among the candidates, else the worktree root.
/// A vanished candidate falls back to the root; a missing root is an error.
fn resolve_launch_dir(candidates: &[Option<&str>], root: &str) -> Result<PathBuf, String> {
    for candidate in candidates.iter().flatten() {
        let path = Path::new(candidate);
        if path.is_dir() { return Ok(path.to_path_buf()); }
    }
    let root_path = Path::new(root);
    if root_path.is_dir() { Ok(root_path.to_path_buf()) } else { Err(format!("Worktree is missing: {root}")) }
}

/// Real worktree status derived from its non-closed terminals:
/// waiting-for-input > running > idle > exited.
fn aggregate(mut worker: Worker, terminals: &[TerminalSession]) -> Worker {
    let open: Vec<&TerminalSession> = terminals.iter().filter(|t| !t.closed).collect();
    let alive: Vec<&&TerminalSession> = open.iter().filter(|t| t.status != STATUS_EXITED).collect();
    let has = |status: &str| alive.iter().any(|t| t.status == status);
    let status = if has(STATUS_WAITING_FOR_INPUT) { STATUS_WAITING_FOR_INPUT }
        else if has(STATUS_RUNNING) { STATUS_RUNNING }
        else if has(STATUS_IDLE) { STATUS_IDLE }
        else { STATUS_EXITED };
    worker.status = status.to_string();
    worker.pid = alive.iter().find_map(|t| t.pid);
    worker.exit_code = if alive.is_empty() {
        open.iter().find(|t| t.id == worker.id).or(open.first()).and_then(|t| t.exit_code)
    } else { None };
    worker.last_error = open.iter().find_map(|t| t.last_error.clone());
    worker.desired_running = open.iter().any(|t| t.desired_running);
    worker
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

/// Runtime owner of one terminal: its process, 2 MiB ring and subscriber lease.
struct Session {
    terminal: Mutex<TerminalSession>,
    /// Creation order, used to keep snapshots stable.
    seq: u64,
    io: Mutex<Option<PtyIo>>,
    stream: Mutex<Stream>,
    alive: AtomicBool,
    generation: AtomicU64,
    completed: (Mutex<bool>, Condvar),
}

impl Session {
    fn new(terminal: TerminalSession, seq: u64) -> Self {
        Self {
            terminal: Mutex::new(terminal), seq, io: Mutex::new(None),
            stream: Mutex::new(Stream { ring: OutputRing::new(), subscriber: None, activity: Instant::now(), prompt: PromptDetector::default(), osc7: Osc7Parser::default() }),
            alive: AtomicBool::new(false), generation: AtomicU64::new(0),
            completed: (Mutex::new(true), Condvar::new()),
        }
    }
    fn terminal(&self) -> TerminalSession { self.terminal.lock().unwrap().clone() }
}

pub struct Manager {
    app: AppHandle,
    store: Store,
    _instance_lock: std::fs::File,
    /// Worktree identity records. Runtime fields are derived from terminals.
    workers: Mutex<HashMap<String, Worker>>,
    /// Every terminal, keyed by its immutable terminal id.
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    next_seq: AtomicU64,
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
        let workers = store.workers()?.into_iter().map(|w| (w.id.clone(), w)).collect();
        let mut seq = 0;
        let sessions = store.terminals()?.into_iter().map(|t| { seq += 1; (t.id.clone(), Arc::new(Session::new(t, seq))) }).collect();
        Ok(Arc::new(Self { app, store, _instance_lock: instance_lock, workers: Mutex::new(workers), sessions: Mutex::new(sessions), next_seq: AtomicU64::new(seq + 1), operations: Mutex::new(()), shutting_down: AtomicBool::new(false) }))
    }

    pub fn start_background(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        thread::spawn(move || {
            let Some(engine) = weak.upgrade() else { return; };
            {
                let _operation = engine.operations.lock().unwrap();
                for session in engine.ordered_sessions(None) {
                    if engine.shutting_down.load(Ordering::Acquire) { break; }
                    let terminal = session.terminal();
                    if terminal.desired_running && !terminal.closed { engine.launch(&session); }
                }
            }
            drop(engine);
            loop {
                thread::sleep(Duration::from_secs(1));
                let Some(engine) = weak.upgrade() else { break; };
                if engine.shutting_down.load(Ordering::Acquire) { break; }
                // Never hold the session map while publishing: publishing aggregates it again.
                for session in engine.ordered_sessions(None) {
                    if !session.alive.load(Ordering::Acquire) { continue; }
                    let status = {
                        let mut stream = session.stream.lock().unwrap();
                        if stream.activity.elapsed() < IDLE_AFTER { continue; }
                        let agent = session.terminal.lock().unwrap().kind == KIND_AGENT;
                        if agent && stream.prompt.is_waiting() { STATUS_WAITING_FOR_INPUT } else { STATUS_IDLE }
                    };
                    engine.set_status(&session, status);
                }
            }
        });
    }

    fn session(&self, id: &str) -> Result<Arc<Session>, String> {
        self.sessions.lock().unwrap().get(id).cloned().ok_or_else(|| "Terminal no longer exists".into())
    }

    fn worker(&self, id: &str) -> Result<Worker, String> {
        self.workers.lock().unwrap().get(id).cloned().ok_or_else(|| "Worker no longer exists".into())
    }

    /// Sessions in creation order, optionally limited to one worktree.
    fn ordered_sessions(&self, worker_id: Option<&str>) -> Vec<Arc<Session>> {
        let mut sessions: Vec<_> = self.sessions.lock().unwrap().values().cloned().collect();
        if let Some(worker_id) = worker_id { sessions.retain(|s| s.terminal.lock().unwrap().worker_id == worker_id); }
        sessions.sort_by_key(|s| s.seq);
        sessions
    }

    /// The worker with runtime fields aggregated over its non-closed terminals.
    fn worker_view(&self, worker_id: &str) -> Option<Worker> {
        let worker = self.workers.lock().unwrap().get(worker_id).cloned()?;
        let terminals: Vec<_> = self.ordered_sessions(Some(worker_id)).iter().map(|s| s.terminal()).collect();
        Some(aggregate(worker, &terminals))
    }

    /// Emits the terminal and its worktree's aggregate. Callers must not hold a
    /// terminal lock or the session map.
    fn publish(&self, terminal: &TerminalSession) {
        let _ = self.app.emit("terminal-updated", terminal);
        if let Some(worker) = self.worker_view(&terminal.worker_id) { let _ = self.app.emit("worker-updated", &worker); }
    }

    /// Applies `change` to one terminal, optionally persists it, and returns the new record.
    fn mutate(&self, session: &Session, persist: bool, change: impl FnOnce(&mut TerminalSession)) -> TerminalSession {
        let mut terminal = session.terminal.lock().unwrap();
        change(&mut terminal);
        if persist {
            if let Err(error) = self.store.save_terminal(&terminal) {
                terminal.last_error = Some(match terminal.last_error.take() {
                    Some(previous) => format!("{previous}; persistence: {error}"),
                    None => format!("Could not persist terminal state: {error}"),
                });
            }
        }
        terminal.clone()
    }

    pub(crate) fn worktree_path(&self, id: &str) -> Result<PathBuf, String> {
        Ok(PathBuf::from(self.worker(id)?.path))
    }

    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let _operation = self.operations.lock().unwrap();
        let sessions = self.ordered_sessions(None);
        let terminals: Vec<TerminalSession> = sessions.iter().map(|s| s.terminal()).collect();
        let mut workers: Vec<_> = self.workers.lock().unwrap().values().cloned()
            .map(|worker| { let own: Vec<_> = terminals.iter().filter(|t| t.worker_id == worker.id).cloned().collect(); aggregate(worker, &own) })
            .collect();
        workers.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Snapshot { projects: self.store.projects()?, workers, settings: self.store.settings()?, terminals })
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
            // The primary agent terminal's id equals its worktree id, as in migrated state.
            let primary = TerminalSession {
                id: id.clone(), worker_id: id.clone(), name: worker.name.clone(), kind: KIND_AGENT.into(),
                cwd: worker.path.clone(), current_cwd: None, session_path: worker.session_path.clone(),
                status: STATUS_EXITED.into(), pid: None, exit_code: None, last_error: None,
                desired_running: true, closed: false,
            };
            let saved = self.store.save_worker(&worker).and_then(|_| self.store.save_terminal(&primary));
            if let Err(error) = saved {
                let cleanup = git::delete_worktree(&project, &worker);
                let _ = self.store.delete_worker(&id);
                return Err(format!("Could not save worker: {error}. Worktree cleanup: {cleanup:?}"));
            }
            self.workers.lock().unwrap().insert(id.clone(), worker);
            let session = Arc::new(Session::new(primary, self.next_seq.fetch_add(1, Ordering::AcqRel)));
            self.sessions.lock().unwrap().insert(id.clone(), session.clone());
            self.launch(&session);
            if let Some(view) = self.worker_view(&id) { result.push(view); }
        }
        Ok(result)
    }

    /// Creates another terminal inside an existing worktree. Never creates a worktree.
    pub fn create_terminal(self: &Arc<Self>, worker_id: &str, source_id: Option<&str>, kind: &str) -> Result<TerminalSession, String> {
        let _operation = self.operations.lock().unwrap();
        self.require_active()?;
        if kind != KIND_AGENT && kind != KIND_SHELL { return Err(format!("Unknown terminal kind '{kind}'")); }
        let worker = self.worker(worker_id)?;
        let source = match source_id {
            Some(id) => { let source = self.session(id)?.terminal(); if source.worker_id != worker_id { return Err("The source terminal belongs to another worktree".into()); } Some(source) }
            None => None,
        };
        let launch_dir = resolve_launch_dir(&[source.as_ref().and_then(|s| s.current_cwd.as_deref()), source.as_ref().map(|s| s.cwd.as_str())], &worker.path)?;
        let id = uuid::Uuid::new_v4().to_string();
        let existing = self.ordered_sessions(Some(worker_id));
        let same_kind = existing.iter().filter(|s| s.terminal.lock().unwrap().kind == kind).count();
        let name = if kind == KIND_SHELL { format!("Terminal {}", same_kind + 1) } else { format!("Agent {}", same_kind + 1) };
        let terminal = TerminalSession {
            session_path: unique_session_path(&worker, &id), id: id.clone(), worker_id: worker_id.to_owned(), name, kind: kind.to_owned(),
            cwd: launch_dir.to_string_lossy().into_owned(), current_cwd: None,
            status: STATUS_EXITED.into(), pid: None, exit_code: None, last_error: None,
            desired_running: true, closed: false,
        };
        self.store.save_terminal(&terminal)?;
        let session = Arc::new(Session::new(terminal, self.next_seq.fetch_add(1, Ordering::AcqRel)));
        self.sessions.lock().unwrap().insert(id, session.clone());
        self.launch(&session);
        Ok(session.terminal())
    }

    fn launch(self: &Arc<Self>, session: &Arc<Session>) {
        let generation = session.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if let Err(error) = self.spawn(session, generation) {
            let terminal = self.mutate(session, true, |t| {
                t.status = STATUS_EXITED.into(); t.pid = None; t.exit_code = None;
                t.last_error = Some(error); t.desired_running = false;
            });
            self.publish(&terminal);
        }
    }

    fn spawn(self: &Arc<Self>, session: &Arc<Session>, generation: u64) -> Result<(), String> {
        let terminal = session.terminal();
        let worker = self.worker(&terminal.worker_id)?;
        let launch_dir = resolve_launch_dir(&[Some(terminal.cwd.as_str())], &worker.path)?;
        let mut resume = false;
        let mut command = if terminal.kind == KIND_SHELL { shell_command()? } else {
            let settings = self.store.settings()?;
            let executable = which::which(&settings.omp_path).map_err(|e| format!("Cannot find omp executable '{}': {e}. Set its full path in Settings.", settings.omp_path))?;
            std::fs::create_dir_all(&terminal.session_path).map_err(|e| e.to_string())?;
            resume = std::fs::read_dir(&terminal.session_path).map_err(|e| e.to_string())?
                .filter_map(Result::ok).any(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"));
            let mut command = CommandBuilder::new(executable);
            command.args(["--session-dir", &terminal.session_path, "--motion", "none", "--spinner-interval", "0", "--no-title"]);
            if resume { command.arg("--continue"); }
            command
        };
        command.cwd(&launch_dir);
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
            stream.activity = Instant::now(); stream.prompt.clear(); stream.osc7 = Osc7Parser::default();
            let marker = match (terminal.kind.as_str(), resume) {
                (KIND_SHELL, _) => b"\r\n[Minnow: starting shell]\r\n".as_slice(),
                (_, true) => b"\r\n[Minnow: resuming session]\r\n".as_slice(),
                _ => b"\r\n[Minnow: starting session]\r\n".as_slice(),
            };
            stream.ring.append(marker); stream.send_pending(&terminal.id);
        }
        let started = self.mutate(session, true, |t| {
            t.status = STATUS_RUNNING.into(); t.pid = Some(pid); t.exit_code = None;
            t.last_error = None; t.desired_running = true; t.current_cwd = None;
        });
        self.publish(&started);
        let unc_host = unc_host_of(&launch_dir);
        let weak = Arc::downgrade(self);
        let output_session = session.clone();
        let output_id = terminal.id.clone();
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
                                    let terminal = engine.mutate(&output_session, false, |t| t.last_error = Some(format!("ConPTY cursor negotiation failed: {error}")));
                                    engine.publish(&terminal);
                                }
                            }
                        }
                        if prefix.is_empty() && data.is_empty() { continue; }
                        let reported_cwd = {
                            let mut stream = output_session.stream.lock().unwrap();
                            stream.activity = Instant::now();
                            stream.prompt.push(prefix); stream.prompt.push(data);
                            let from_prefix = stream.osc7.feed(prefix, unc_host.as_deref());
                            let reported = stream.osc7.feed(data, unc_host.as_deref()).or(from_prefix);
                            stream.ring.append(prefix); stream.ring.append(data);
                            stream.send_pending(&output_id);
                            reported
                        };
                        if let Some(engine) = weak.upgrade() {
                            if let Some(cwd) = reported_cwd {
                                let changed = output_session.terminal.lock().unwrap().current_cwd.as_deref() != Some(cwd.as_str());
                                if changed && output_session.generation.load(Ordering::Acquire) == generation {
                                    let terminal = engine.mutate(&output_session, false, |t| t.current_cwd = Some(cwd));
                                    engine.publish(&terminal);
                                }
                            }
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
            // Closing the job also terminates descendants that outlived the parent.
            session.io.lock().unwrap().take();
            if let Some(engine) = weak.upgrade() {
                let terminal = engine.mutate(&session, true, |t| {
                    t.status = STATUS_EXITED.into(); t.pid = None;
                    match result { Ok(status) => t.exit_code = Some(status.exit_code()), Err(e) => t.last_error = Some(e.to_string()) }
                    // A natural exit is not an instruction to relaunch next time.
                    t.desired_running = false;
                });
                engine.publish(&terminal);
            }
            *session.completed.0.lock().unwrap() = true;
            session.completed.1.notify_all();
        });
        Ok(())
    }

    fn set_status(&self, session: &Session, status: &str) {
        let alive = session.alive.load(Ordering::Acquire);
        let mut changed = None;
        let terminal = self.mutate(session, false, |t| {
            if t.status != status && alive { t.status = status.into(); changed = Some(t.clone()); }
        });
        let Some(_) = changed else { return; };
        self.publish(&terminal);
    }

    /// Shows a system notification. Which transitions notify is the UI's decision
    /// (Settings > Notifications), so the engine itself never raises one.
    pub fn notify(&self, title: &str, message: &str) {
        let title = title.to_owned();
        let message = message.to_owned();
        thread::spawn(move || {
            if let Err(error) = notify_rust::Notification::new().summary(&title).body(&message).appname("Minnow").show() {
                eprintln!("Windows notification failed: {error}");
            }
        });
    }

    /// Stops one terminal's process tree and waits for its exit to be recorded.
    fn stop(&self, session: &Arc<Session>, explicit: bool) -> Result<(), String> {
        if explicit {
            let snapshot = {
                let mut terminal = session.terminal.lock().unwrap();
                terminal.desired_running = false;
                self.store.save_terminal(&terminal)?;
                terminal.clone()
            };
            // Publishing the intent first lets the UI tell an explicit stop from a crash.
            self.publish(&snapshot);
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
        if timeout.timed_out() && !*finished { return Err("Timed out waiting for the terminal process to exit; worktree retained".into()); }
        Ok(())
    }

    /// Non-closed terminals of one worktree: the ones worktree actions operate on.
    fn open_sessions(&self, worker_id: &str) -> Vec<Arc<Session>> {
        self.ordered_sessions(Some(worker_id)).into_iter().filter(|s| !s.terminal.lock().unwrap().closed).collect()
    }

    fn any_alive(&self, worker_id: &str) -> bool {
        self.ordered_sessions(Some(worker_id)).iter().any(|s| s.alive.load(Ordering::Acquire))
    }

    pub fn kill_worker(&self, id: &str) -> Result<Worker, String> {
        let _operation = self.operations.lock().unwrap();
        self.worker(id)?;
        for session in self.open_sessions(id) { self.stop(&session, true)?; }
        self.worker_view(id).ok_or_else(|| "Worker no longer exists".into())
    }

    pub fn restart_worker(self: &Arc<Self>, id: &str) -> Result<Worker, String> {
        let _operation = self.operations.lock().unwrap(); self.require_active()?;
        let worker = self.worker(id)?;
        let open = self.open_sessions(id);
        for session in &open { self.stop(session, true)?; }
        if open.is_empty() {
            // An empty worktree intentionally gets a fresh primary agent.
            self.add_primary_agent(&worker)?;
        } else {
            for session in &open { self.launch(session); }
        }
        self.worker_view(id).ok_or_else(|| "Worker no longer exists".into())
    }

    fn add_primary_agent(self: &Arc<Self>, worker: &Worker) -> Result<(), String> {
        let id = uuid::Uuid::new_v4().to_string();
        let existing = self.ordered_sessions(Some(&worker.id));
        // The retained worktree session root continues the old conversation, unless a record owns it.
        let root_taken = existing.iter().any(|s| s.terminal.lock().unwrap().session_path == worker.session_path);
        let terminal = TerminalSession {
            session_path: if root_taken { unique_session_path(worker, &id) } else { worker.session_path.clone() },
            id: id.clone(), worker_id: worker.id.clone(), name: worker.name.clone(), kind: KIND_AGENT.into(),
            cwd: worker.path.clone(), current_cwd: None,
            status: STATUS_EXITED.into(), pid: None, exit_code: None, last_error: None,
            desired_running: true, closed: false,
        };
        self.store.save_terminal(&terminal)?;
        let session = Arc::new(Session::new(terminal, self.next_seq.fetch_add(1, Ordering::AcqRel)));
        self.sessions.lock().unwrap().insert(id, session.clone());
        self.launch(&session);
        Ok(())
    }

    pub fn kill_terminal(&self, id: &str) -> Result<TerminalSession, String> {
        let _operation = self.operations.lock().unwrap();
        let session = self.session(id)?;
        self.stop(&session, true)?;
        let terminal = session.terminal();
        self.publish(&terminal);
        Ok(terminal)
    }

    /// Restarts the same terminal identity and session; also reopens a closed one.
    pub fn restart_terminal(self: &Arc<Self>, id: &str) -> Result<TerminalSession, String> {
        let _operation = self.operations.lock().unwrap(); self.require_active()?;
        let session = self.session(id)?;
        self.stop(&session, true)?;
        self.mutate(&session, false, |t| { t.closed = false; t.current_cwd = None; });
        self.launch(&session);
        Ok(session.terminal())
    }

    /// Stops the terminal and keeps its durable record so it can be reopened.
    pub fn close_terminal(&self, id: &str) -> Result<TerminalSession, String> {
        let _operation = self.operations.lock().unwrap();
        let session = self.session(id)?;
        self.stop(&session, true)?;
        let terminal = self.mutate(&session, true, |t| {
            t.closed = true; t.desired_running = false; t.current_cwd = None;
            t.status = STATUS_EXITED.into(); t.pid = None;
        });
        {
            let mut stream = session.stream.lock().unwrap();
            stream.subscriber = None; stream.ring.release(); stream.prompt.clear(); stream.osc7 = Osc7Parser::default();
        }
        self.publish(&terminal);
        Ok(terminal)
    }

    /// The directory a live OSC 7 report confirmed, or empty when none was seen.
    pub fn terminal_cwd(&self, id: &str) -> Result<String, String> {
        Ok(self.session(id)?.terminal.lock().unwrap().current_cwd.clone().unwrap_or_default())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let _operation = self.operations.lock().unwrap();
        let worker = self.worker(id)?;
        if self.any_alive(id) { return Err("Stop every terminal in this worktree before deleting it".into()); }
        git::delete_worktree(&self.store.project(&worker.project_id)?, &worker)?;
        self.store.delete_worker(id)?;
        let removed: Vec<String> = {
            let mut sessions = self.sessions.lock().unwrap();
            let ids: Vec<String> = sessions.iter().filter(|(_, s)| s.terminal.lock().unwrap().worker_id == id).map(|(key, _)| key.clone()).collect();
            for key in &ids { sessions.remove(key); }
            ids
        };
        self.workers.lock().unwrap().remove(id);
        for terminal_id in &removed { let _ = self.app.emit("terminal-removed", terminal_id); }
        let _ = self.app.emit("worker-removed", id);
        Ok(())
    }

    pub fn diff(&self, id: &str) -> Result<String, String> { git::diff_worker(&self.worker_view(id).ok_or("Worker no longer exists")?) }

    pub fn merge(&self, id: &str) -> Result<String, String> {
        let _operation = self.operations.lock().unwrap();
        let worker = self.worker(id)?;
        if self.any_alive(id) { return Err("Stop every terminal in this worktree before merging its committed changes".into()); }
        git::merge_worker(&self.store.project(&worker.project_id)?, &worker)
    }

    pub fn open_editor(&self, id: &str) -> Result<(), String> {
        let worker = self.worker(id)?;
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
            let io = io.as_mut().ok_or("Terminal has exited; restart it before typing")?;
            io.writer.write_all(data.as_bytes()).and_then(|_| io.writer.flush()).map_err(|e| e.to_string())?;
        }
        {
            let mut stream = session.stream.lock().unwrap();
            stream.activity = Instant::now(); stream.prompt.clear();
        }
        self.set_status(&session, STATUS_RUNNING);
        Ok(())
    }
    fn require_active(&self) -> Result<(), String> { if self.is_shutting_down() { Err("Application is shutting down".into()) } else { Ok(()) } }
    pub fn is_shutting_down(&self) -> bool { self.shutting_down.load(Ordering::Acquire) }
    pub fn shutdown(&self) {
        if self.shutting_down.swap(true, Ordering::AcqRel) { return; }
        let _operation = self.operations.lock().unwrap();
        for session in self.ordered_sessions(None) {
            let resume = session.terminal.lock().unwrap().desired_running;
            if let Err(error) = self.stop(&session, false) { eprintln!("Shutdown: {error}"); }
            // The exit waiter clears desired_running. Restore only terminals active at close.
            if resume {
                let mut terminal = session.terminal.lock().unwrap(); terminal.desired_running = true;
                if let Err(error) = self.store.save_terminal(&terminal) { eprintln!("Saving resume state: {error}"); }
            }
        }
    }
}

/// A session directory no other terminal shares, so concurrent omp runs never
/// write into the same `--session-dir`.
fn unique_session_path(worker: &Worker, terminal_id: &str) -> String {
    let root = Path::new(&worker.session_path);
    root.parent().unwrap_or(root).join("terminals").join(terminal_id).to_string_lossy().into_owned()
}

/// A real local shell. No profile or prompt is forced on the user.
fn shell_command() -> Result<CommandBuilder, String> {
    #[cfg(windows)]
    {
        let system_root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let native = system_root.join("System32").join("WindowsPowerShell").join("v1.0").join("powershell.exe");
        let executable = if native.is_file() { native } else {
            which::which("powershell.exe").map_err(|e| format!("Cannot find Windows PowerShell: {e}"))?
        };
        let mut command = CommandBuilder::new(executable);
        command.arg("-NoLogo");
        Ok(command)
    }
    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        Ok(CommandBuilder::new(shell))
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
    fn released_ring_keeps_offsets_monotonic() {
        let mut ring = OutputRing::new();
        ring.append(b"hello");
        ring.release();
        assert_eq!(ring.bytes.len(), 0);
        assert_eq!(ring.end, 5);
        ring.append(b"x");
        assert_eq!(ring.from(0, 8), (5, b"x".to_vec()));
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

    #[test]
    fn osc7_accepts_bel_and_st_terminators() {
        let mut parser = Osc7Parser::default();
        assert_eq!(parser.feed(b"out\x1b]7;file:///C:/Users/me/proj\x07tail", None).as_deref(), Some(r"C:\Users\me\proj"));
        assert_eq!(parser.feed(b"\x1b]7;file://HOST/C:/a%20b\x1b\\", None).as_deref(), Some(r"C:\a b"));
    }
    #[test]
    fn osc7_survives_every_chunk_boundary() {
        let sequence = b"\x1b]7;file://PC/C:/Users/me/%C3%A9t%C3%A9\x1b\\";
        for split in 1..sequence.len() {
            let mut parser = Osc7Parser::default();
            let first = parser.feed(&sequence[..split], None);
            let second = parser.feed(&sequence[split..], None);
            assert_eq!(first.or(second).as_deref(), Some(r"C:\Users\me\été"), "split at {split}");
        }
    }
    #[test]
    fn osc7_reports_the_last_directory_and_ignores_other_oscs() {
        let mut parser = Osc7Parser::default();
        let data = b"\x1b]0;title\x07\x1b]7;file:///C:/one\x07\x1b]7;file:///C:/two\x07\x1b]8;;https://x\x07";
        assert_eq!(parser.feed(data, None).as_deref(), Some(r"C:\two"));
        assert_eq!(parser.feed(b"plain output", None), None);
    }
    #[test]
    fn osc7_rejects_malformed_nul_oversized_and_non_file_payloads() {
        let mut parser = Osc7Parser::default();
        assert_eq!(parser.feed(b"\x1b]7;http://host/C:/x\x07", None), None);
        assert_eq!(parser.feed(b"\x1b]7;file://host\x07", None), None);
        assert_eq!(parser.feed(b"\x1b]7;file:///C:/a%00b\x07", None), None);
        let mut long = b"\x1b]7;file:///C:/".to_vec();
        long.extend(std::iter::repeat(b'a').take(OSC7_MAX_BYTES + 10));
        long.push(0x07);
        assert_eq!(parser.feed(&long, None), None);
        // The oversized sequence must not poison the next one.
        assert_eq!(parser.feed(b"\x1b]7;file:///C:/ok\x07", None).as_deref(), Some(r"C:\ok"));
        // An interrupted sequence restarts cleanly.
        assert_eq!(parser.feed(b"\x1b]7;file:///C:/lost\x1bX\x1b]7;file:///C:/found\x07", None).as_deref(), Some(r"C:\found"));
    }
    #[test]
    fn osc7_percent_decoding_is_lenient_about_literal_percent() {
        assert_eq!(parse_osc7_payload(b"7;file:///C:/100%/x", None).as_deref(), Some(r"C:\100%\x"));
        assert_eq!(parse_osc7_payload(b"7;file:///home/me/a%20b", None).as_deref(), Some("/home/me/a b"));
        assert_eq!(parse_osc7_payload(b"7;file:///C:", None).as_deref(), Some(r"C:\"));
    }
    #[test]
    fn osc7_unc_paths_need_the_launch_unc_server() {
        assert_eq!(parse_osc7_payload(b"7;file://Server/share/dir", Some("server")).as_deref(), Some(r"\\Server\share\dir"));
        assert_eq!(parse_osc7_payload(b"7;file://Other/share/dir", Some("server")).as_deref(), Some("/share/dir"));
        assert_eq!(unc_host_of(Path::new(r"\\fileserver\share\repo")).as_deref(), Some("fileserver"));
        assert_eq!(unc_host_of(Path::new(r"\\?\C:\repo")), None);
        assert_eq!(unc_host_of(Path::new(r"C:\repo")), None);
    }

    fn terminal(id: &str, status: &str, closed: bool) -> TerminalSession {
        TerminalSession {
            id: id.into(), worker_id: "w".into(), name: id.into(), kind: KIND_SHELL.into(),
            cwd: String::new(), current_cwd: None, session_path: String::new(), status: status.into(),
            pid: if status == STATUS_EXITED { None } else { Some(10) }, exit_code: None, last_error: None,
            desired_running: status != STATUS_EXITED, closed,
        }
    }
    fn worker() -> Worker {
        Worker { id: "w".into(), project_id: "p".into(), name: "w".into(), path: String::new(), branch: String::new(), base_ref: String::new(), base_commit: String::new(), session_path: String::new(), status: STATUS_EXITED.into(), pid: None, exit_code: None, last_error: None, desired_running: false }
    }
    #[test]
    fn worker_status_aggregates_open_terminals_by_precedence() {
        let all = [terminal("a", STATUS_IDLE, false), terminal("b", STATUS_RUNNING, false), terminal("c", STATUS_WAITING_FOR_INPUT, false)];
        assert_eq!(aggregate(worker(), &all).status, STATUS_WAITING_FOR_INPUT);
        assert_eq!(aggregate(worker(), &all[..2]).status, STATUS_RUNNING);
        assert_eq!(aggregate(worker(), &all[..1]).status, STATUS_IDLE);
        let stopped = [terminal("a", STATUS_EXITED, false), terminal("b", STATUS_RUNNING, true)];
        let view = aggregate(worker(), &stopped);
        assert_eq!(view.status, STATUS_EXITED);
        assert_eq!(view.pid, None);
        assert!(!view.desired_running, "a closed terminal never keeps the worktree resumable");
        let live = aggregate(worker(), &[terminal("a", STATUS_EXITED, false), terminal("b", STATUS_IDLE, false)]);
        assert_eq!(live.pid, Some(10));
        assert!(live.desired_running);
    }
    #[test]
    fn launch_directory_falls_back_to_the_root_only_when_it_exists() {
        let root = tempfile::tempdir().unwrap();
        let inner = root.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        let root_text = root.path().to_str().unwrap();
        let inner_text = inner.to_str().unwrap();
        assert_eq!(resolve_launch_dir(&[Some(inner_text)], root_text).unwrap(), inner);
        assert_eq!(resolve_launch_dir(&[None, Some(inner_text)], root_text).unwrap(), inner);
        std::fs::remove_dir(&inner).unwrap();
        assert_eq!(resolve_launch_dir(&[Some(inner_text)], root_text).unwrap(), root.path());
        let missing = root.path().join("gone");
        let error = resolve_launch_dir(&[Some(inner_text)], missing.to_str().unwrap()).unwrap_err();
        assert!(error.contains("Worktree is missing"), "{error}");
    }
    #[test]
    fn extra_session_directories_never_share_the_primary_root() {
        let mut value = worker();
        value.session_path = r"C:\repo\.minnow\sessions\w".into();
        let extra = unique_session_path(&value, "t1");
        assert_ne!(extra, value.session_path);
        assert!(!extra.starts_with(&format!("{}{}", value.session_path, std::path::MAIN_SEPARATOR)));
        assert_ne!(extra, unique_session_path(&value, "t2"));
    }
}
