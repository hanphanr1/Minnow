# Terminal cutover: paused design contract

Status: design/partial frontend migration only. Native implementation has not started. The verified runnable checkpoint is `1f4746c`; do not treat this document as proof of implementation. See SESSION_HANDOFF.md for the exact paused file inventory.

# Goal
Port Orca's many-terminals-per-worktree behavior to Minnow. A new terminal or pane split MUST NOT call Git worktree creation. Preserve independent process ownership, 2 MiB output ring per terminal, 64 KiB acknowledgement window, visible-only xterm mounts, and deterministic shutdown/resume. Do not claim actual omp inference/RAM results from smoke fixtures.

# Stable worktree surface
`Worker` remains the Git worktree identity used by sidebar, project selection, file/Git APIs, merge/delete and existing create_workers. Existing worker runtime fields become truthful aggregate observations over its non-closed terminals: waiting-for-input > running > idle > exited; `pid` is a live terminal PID if any; `desired_running` represents any resumable terminal, not a durable second source of truth. `session_path` is the worktree's retained session root, preserving existing OMP continuation files.

# New serialized terminal model
`TerminalSession`:
- `id: string` immutable terminal identity
- `worker_id: string` immutable owning worktree
- `name: string`
- `kind: 'agent' | 'shell'`
- `cwd: string` initial launch directory; `current_cwd: string|null` directory reported by actual live OSC 7 output, reset on process restart
- `session_path: string` independent OMP session location
- `status: WorkerStatus`, `pid: number|null`, `exit_code: number|null`, `last_error: string|null`
- `desired_running: boolean`, `closed: boolean`
`Snapshot` adds `terminals: TerminalSession[]`.
`TerminalChunk` uses `terminal_id` instead of `worker_id`.

# Native commands
Keep worktree commands on workerId; migrate every PTY call to terminalId, no compatibility aliases:
- create_terminal(workerId, sourceTerminalId: string|null, kind: 'agent'|'shell') -> TerminalSession
- restart_terminal(terminalId) -> TerminalSession
- kill_terminal(terminalId) -> TerminalSession: stop but retain visible record/output
- close_terminal(terminalId) -> TerminalSession: stop, mark closed, release closed ring resources; retain durable identity/session history for reopen
- attach_terminal(terminalId, cols, rows, onOutput), detach_terminal(terminalId), resize_terminal(terminalId,cols,rows), write_terminal(terminalId,data), ack_terminal(terminalId,end)
- terminal_cwd(terminalId) -> string: confirmed OSC 7 directory, or empty when unknown. Unknown must not be fabricated as a process lookup result. create_terminal uses source current_cwd, else source launch cwd, else worktree root. If the chosen directory disappeared and the worktree root still exists, source permits fallback to that root; a missing root remains an actual error.
Emit `terminal-updated` records and `terminal-removed` IDs independently of `worker-updated` aggregates. Root subscribes before bootstrap and queues in-flight events just as for workers. A close retains its durable record, so emit `terminal-updated` with `closed=true`, not `terminal-removed`; reserve removal for actual record deletion such as successful worktree deletion.

Reference evidence: `_ref/orca/src/main/providers/process-cwd.ts` has only Linux/macOS/lsof paths; `daemon-live-sleep-cwd.ts` explicitly refuses process lookup on Windows. `components/terminal-pane/parse-osc7.ts` and `resolve-split-cwd.ts` are the actual Windows behavior: confirmed OSC 7, then unconfirmed cached cwd, then workspace root. Do not add guessed Win32 PEB reads or spawn missing lsof. Native streaming OSC 7 parsing should handle BEL/ST framing across chunks, bounded payloads, percent escapes, Windows drive and known launch UNC host paths, and ignore malformed/NUL paths. It must not consume or alter terminal output. Parent will prove a real shell emits OSC 7 before a split; no injected replacement prompt/profile.

# Persistence and compatibility cutover
Add a durable terminal_sessions table with worktree foreign key. Migrate each existing worker once to a primary agent terminal whose id equals its old worker id. Preserve its old session_path, desired_running and last_error; runtime PID/status always resets on load. Additional agent sessions have unique subdirectories, never share --session-dir. Ordinary terminals use a real local shell (Windows PowerShell native executable), no forced user profile/prompt replacement. Initial workspace creation still launches its primary agent. Restart/shutdown/resume must affect the correct independently owned processes. Legacy worker desired/error columns may remain only as migration input, not a simultaneously maintained source of truth; remove obsolete reads/writes after migration. No aliases accepting workerId for PTY calls.

Merge/delete worktree must refuse while ANY owned terminal is alive and preserve existing dirty/unmerged safety rules. Stop/restart worktree actions operate on its non-closed sessions; empty-worktree Restart may create a new primary agent intentionally. Close/reopen a tab restarts its same terminal identity/session and never a new worktree. Worker deletion removes terminal records only after native Git safety succeeds and retains session files as before.

# Frontend ownership
Workspace takes one selected `worker: Worker`, its `terminals: TerminalSession[]`, `preferences`, and callbacks. Tabs/groups/panes are scoped to that worker. Pane leaves use terminalId; focus/terminal handles/leases use terminalId. File/Git panels continue using selected Worker, not focused terminal. `onCreate(kind,sourceTerminalId?)` returns a TerminalSession; default new terminal is shell, split copies source kind and live cwd. Explicit New agent terminal remains reachable and uses configured OMP. `onStop/onRestart/onCloseTerminal` use terminal identity; worktree actions receive Worker.

Preserve tab pins/title/color/order, groups, ratios, close/reopen and keyboard/clipboard behavior. Move persistence to a versioned per-worker key; migrate old project-level layouts by keeping only the selected worker's old primary pane/tab and collapsing foreign-worktree splits. Never reinterpret an old split's separate worktrees as shared terminals. No empty success or fake terminal fallback.

## Agreed root component interface (not implemented in Workspace.svelte yet)

```ts
project: Project;
worker: Worker;
terminals: TerminalSession[]; // selected worker only, including closed records
preferences: UiPreferences;
sidebarVisible: boolean;
rightSidebarVisible?: boolean;
onActivateTerminal: (terminalId: string) => void;
onCreateTerminal: (kind: TerminalKind, sourceTerminalId: string | null) => Promise<TerminalSession>;
onRestartTerminal: (terminalId: string) => Promise<TerminalSession>;
onStopTerminal: (terminalId: string) => Promise<TerminalSession>;
onCloseTerminal: (terminalId: string) => Promise<TerminalSession>;
onAction: (action: WorkerAction, worker: Worker) => void;
onStopWorker?: (worker: Worker) => Promise<void>;
// Keep onFeature/onError/onToggleSidebar/onToggleRightSidebar.
// Export createTerminal(kind: TerminalKind = 'shell'), handleShortcut and focusActive.
```

`onActivateTerminal` must never assign a terminal ID to App's activeWorkerId. Keep worktree selection separate. App still needs terminal event subscription/queueing before bootstrap and a terminal collection keyed by terminal ID.

The partial layout code uses `minnow.orca.workspace.v3.<projectId>.<workerId>`, `version: 3`. It migrates legacy `minnow.orca.workspace.v1.<projectId>`. Verify actual older snapshot shape in the saved state before changing migration: do not discard another worktree's durable record or reinterpret it as a terminal.

Native migration should be transactional/idempotent and not recreate a primary on every reopen. Treat a worker with existing terminal records as already migrated; do not invent a new primary merely because a different terminal ID is present.

## Separate Explorer follow-on (planned only)

Source Explorer has a persistent Names/Contents switch plus a global Find files query. Current Minnow only has loaded-tree name filtering and separate content-search mode. No new filename-search implementation was written before the pause. Planned addition: `workspace::FileSearchResults { paths: Vec<String>, truncated: bool }`, `workspace::find_files(root, query)` and `find_workspace_files(workerId, query)` wrapper in lib.rs. Reuse the bounded directory/ignore traversal in workspace_search.rs rather than copying a second walker or returning fake SearchMatch line numbers. Defer this until terminal cutover is coherent if needed. No such command/type exists yet.


# Parent integration
Parent owns App.svelte, documentation, package/Cargo dependencies if requested, smoke scripts and final checks. Native slice owns model/storage/pty/lib plus narrowly necessary native modules/tests. Frontend slice owns types/api/terminal-api (if useful), Terminal.svelte, Workspace.svelte, PaneTree.svelte, workspace-layout.ts and narrowly needed component helpers. Shared structures and command/event names above are the contract; discuss any necessary change before diverging.

# Acceptance
Actual native smoke: migrate existing isolated state; one worktree with at least three independent terminal PIDs; new/split does not add workers/Git worktrees; typed shell cwd change inherited by split when live cwd is available; input/output streams independent; closing one pane stops only that process; stop/merge safety sees all terminals; closed pane reopens its terminal identity; cold app restart relaunches desired sessions but not explicitly closed/stopped ones. Check SQLite migration and existing lease/ring regression behavior. Tests alone do not establish native behavior.
