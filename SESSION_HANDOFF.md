# Minnow: paused checkpoint and agent handoff

## Status

The terminal cutover is complete and integrated: `Worker` is the Git worktree identity and `TerminalSession` is an independently owned PTY inside it. New terminal and split never create a worktree. The verified code milestone is the commit that carries this document (see `git log`); the previous green milestone was `1f4746c`.

Exercised: frontend check/build with zero errors/warnings, 40 native tests, and the native desktop scenarios under "Terminal cutover evidence" below. Not exercised: a rebuilt release executable, real omp inference sessions, the New Agent Terminal menu item, a New Terminal from the file/diff view, flood/ring regressions after the cutover, and non-Windows shells. The four-real-omp-worker RAM measurement remains paused by the user; no memory result or claim below 150 MiB has been published. UI smoke workers run a deterministic native PTY fixture, not omp inference.

## Repository and milestones

- Workspace: `C:/Users/DELL/Documents/ORCA TAURI`.
- Product/window: Minnow; binary: `minnow`; identifier: `com.minnow.app`.
- Author: `hanphanr1`; remote: `https://github.com/hanphanr1/Minnow.git`; branch: `main`.
- Identity/icon milestone: `13f9b81`. Supplied artwork and generated Tauri assets are preserved.
- Working frontend milestone: `24fcd27`.
- Twelve subsequent screen-evidence commits run from `547713e` through `1037ad1`. UI_PARITY.md indexes each screen and exact commit.
- Verified/published native-workspace milestone: `1f4746c` (`feat(workspace): add native files, safe editing and Git workflows`). HEAD and origin/main both resolved to the full hash above before this docs-only handoff checkpoint.
- Separate native handshake correction: `f122314`, included in the successful push through `1f4746c`.

## Read this first on resume

1. `CONTINUE_PROMPT.md`: copy-ready continuation brief.
2. This file: exact verified vs unfinished state and runtime hygiene.
3. `TERMINAL_CUTOVER.md`: durable model/API/migration contract, root interface, Windows cwd evidence and acceptance scenarios (now implemented).
4. `TODO.md` and `UI_PARITY.md`: full remaining feature coverage, source citations, proof and known visual differences. Do not repeat cloning/icons/initial shell work.

## Terminal cutover evidence

Native desktop run with an isolated copy of the verified pre-cutover SQLite (`.smoke/terminal-state`, WebView2 profile and CDP port 9226 of its own; the debug executable loads the Vite dev server). Scripts live in untracked `.smoke/terminal_cutover_*.py`; every claim was cross-checked against the bootstrap snapshot, SQLite, `git worktree list` and the process table.

- Migration: the legacy worker became one primary agent terminal whose id equals the worker id, with its old session path and resume flag; reopening adds nothing.
- Three independent terminal PIDs in one worktree (agent + two PowerShell). Ctrl+T and Ctrl+Shift+D left the Git worktree list and the worker count unchanged (13 / 1).
- Independent I/O: each shell wrote its own `$PID`, equal to the native terminal PID; input to one never reached the other.
- Windows cwd: a real Windows PowerShell emitted OSC 7 when told to (no injected prompt or profile); `terminal_cwd` was empty before that and the confirmed directory afterwards. A split from that pane started in that directory and its shell reported the same `$PWD`. Orca has no native Windows process-cwd lookup, so an unreported cwd falls back to the launch cwd or worktree root.
- Closing one pane stopped only its process; the other three stayed alive; the record stayed in SQLite with closed=1, desired_running=0.
- Closing the agent tab then Ctrl+Shift+T restarted the same terminal id and session path with a new PID, without a new record or worktree.
- Safety: merge and delete were refused while any terminal was alive, including with a single survivor; after killing all terminals delete succeeded, removed its terminal rows and the worktree, and left the main worktree's terminals intact.
- Cold restart (graceful close, relaunch): desired terminals were relaunched with new PIDs; a terminal stopped with kill_terminal and a closed terminal were not.
- Legacy layout: a real saved version-2 workerId layout (shape copied from the verified build's localStorage) plus an old split that mixed in another worktree migrated to version 3 keeping the tab id, title, color and pin, with the foreign worktree dropped rather than turned into a shared terminal.
- Screenshots: `.smoke/tc-02` to `tc-06` (untracked).

Known limits: ConPTY forwarded OSC 7 in this environment, but a shell that never emits it keeps the fallback; the stale `current_cwd` of a stopped terminal is retained until it is restarted; Windows PowerShell (not pwsh) is the shell.

## Implemented frontend

- Exact source theme tokens, Geist/Nerd fonts, Lucide geometry, 280px/350px reference sidebars, 36px Windows chrome, and 24px status bar.
- Project/workspace sidebar, pins/grouping/collapse, selection/history, real local project addition, composer with counts/Create more, and workspace palette with filtering/recency.
- Tabs, pin/title/color/reorder, nested source-shaped context menus, split panes, split tab groups, resizing, focused-pane expansion, and recent-tab switching.
- Visible-only xterm 6 with compatible fit/search/WebGL addons. Terminal engine, WebGL, Settings and DiffView load on demand.
- Actual Git patch list and inline/side-by-side diff, path copy, safe worker actions, source-shaped settings, and persisted supported appearance/terminal preferences.
- Real lazy Explorer/content search, CodeMirror file editing, SHA-256 revision-checked saves, real Git staged/unstaged/untracked status, stage/unstage, index-only commits and bounded history. Native project folder dialog, local clone and new Git project creation replace the old admission placeholders. See the separate native continuation evidence in UI_PARITY.md; successful synthetic native picker selection was not established.

Main files: `src/App.svelte`, `src/Terminal.svelte`, `src/app.css`, `src/orca-theme.css`, `src/ui/`, `src/fonts/`. Source/license mapping is in UI_PARITY.md and THIRD_PARTY_NOTICES.md.

## Backend boundary

Native files/Git/watch APIs and profile isolation date from `1f4746c`. The PTY engine now keys every process, 2 MiB ring, subscriber lease and resume flag by terminal id (`terminal_sessions` table, `terminal-updated` / `terminal-removed` events, `create/restart/kill/close_terminal`, `terminal_cwd`); worktree commands still take a worker id and aggregate their terminals.

The earlier ConPTY startup-handshake correction is now committed separately as `f122314`, not silently bundled into UI work. Isolated `MINNOW_DATA_DIR` runs now isolate their actual WebView2 profile through the native builder; Tauri 2.12's configuration conversion omitted `dataDirectory` and otherwise collided with an already-running release.

## Verification and artifacts

- At `1f4746c`: frontend check had zero errors/warnings; production build and native build succeeded; 27 native tests passed. These results do not cover the current partial terminal migration.
- The original shell release was built and exercised earlier. The newer native files/Git milestone was tested in the debug desktop; no new release was built for it.
- Native checks/builds passed before the pause; the current incomplete Workspace/native contract was not rerun merely to confirm the known mismatch.
- Existing release executable: `src-tauri/target/release/minnow.exe`; the user's review instance was left running. The new local-feature continuation was exercised with the debug native executable and isolated data, not misreported as a rebuilt release.
- Native release exercise: restored actual worker processes; new/close/reopen; live terminal output; Search/Source Control/settings lazy routes; nested-menu Escape; no page errors in the final run. Owned Minnow smoke windows closed with exit code 0.
- Desktop interaction proof also covers create/count/Create more/duplicate failure, palette, navigation/history, pin/title/color/reorder, horizontal/vertical splits, group moves/resize, expansion, terminal selection/copy/paste/search/clear, real Git patches, themes/zoom, and native maximize/restore.
- Screenshots: 16 labelled side-by-side comparisons plus original captures in `docs/parity/`. See the comparison index in UI_PARITY.md. Originals are 1440×900.
- Reference source: Orca 1.4.214 at `e507dd7732139e302f6adaa2917d8e0aee5b1362`. Genuine reference captures use installed Orca 1.4.245; captions disclose the difference.
- Reviewed continuation frontend check/build passed with zero errors/warnings; 27 Windows-native tests passed. Cold native proof covers actual create/clone, filesystem watching, file edits/saves, CRLF retention, stale revision refusal, same-file search navigation, native Close/Cancel, delete-shortcut dirty guard, staged commit/history and untracked-file diff. Additional 1440×900 native captures are `docs/parity/minnow-functional-*.png`.

## Lifecycle invariants

- Terminals are keyed by immutable terminal ID (a primary migrated terminal reuses its worker ID); ready/disposal callbacks preserve that identity through removal and async teardown. Close keeps its durable record (`terminal-updated` with closed=true); `terminal-removed` is only for a deleted worktree.
- Replacing tab arrays during insertion preserves Svelte invalidation after close/reopen. Avoid returning to in-place insertion that leaves an active tab absent from the rendered strip.
- Workspace overlays own focus while open; asynchronous PTY attachment must not focus a terminal through menus, rename fields, or confirmations.
- Shell-handled key events do not enter xterm. UI zoom is not intercepted while typing inside a terminal.
- xterm 6 uses its scrollable element; the compatibility viewport must stay transparent and non-scrolling.

## Runtime hygiene

Initial smoke state remains `.smoke/parity-state`; continuation state is `.smoke/functional-state`, with its own `webview/` and CDP port 9225. `.smoke/repo` is the fixture repository, and `functional-proof` is its continuation worktree. All its workers use `.smoke/pty-fixture.exe`, not real omp inference. New/clone admission created additional repositories only under `.smoke/`.

The normal user Minnow release/window and main Orca profile were not used for native feature tests. The isolated reference profile remains `.smoke/orca-parity-profile`. Close only owned services/windows after their proof; retain supplied artwork and original captures.

Terminal-cutover smoke state is `.smoke/terminal-state` (copy of the pre-cutover `functional-state`) with CDP port 9226 and launcher `.smoke/LaunchTerminalCutover.ps1`. The debug executable loads the Vite dev server (`npm run dev`, port 1420), so start it first and stop only that owned process afterwards. Both were stopped at the end of the run.

At the earlier pause both implementation workers had stopped. The owned Vite service was stopped and owned browser connections released. The final reviewed Minnow smoke desktop had exited with code 0. Leftover isolated Orca reference processes were explicitly terminated by the owned profile's main PID; the user's normal Minnow release/main Orca session were not closed. No runtime is intentionally left running for the next agent.

For later GUI debugging, cold-reload/restart after Svelte HMR before interpreting stale derived state as a release failure. Python browser automation accepts JavaScript body strings, not an uninvoked arrow-function string. Clipboard helpers can return cached shim data; verify app copy through its actual native clipboard path and visible feedback.

## Remaining work and invariants

- Full feature parity is still open; see TODO.md. Next is the Explorer strip/global filename query and file/diff tabs inside workspace groups (design notes in TERMINAL_CUTOVER.md; no `find_workspace_files` exists yet).
- Native picker opening/cancel was exercised, but synthetic path entry did not update its internal selection model. Typed-path admission and actual create/clone were proven; a manual picker-selection pass remains.
- FileEditor initializes by primitive `documentKey` with untracked prop reads. Same-file line updates must never reload/discard edits. CodeMirror serializes through the document's line-separator facet and `sliceDoc()` to retain CRLF. Native Close uses the same async guard as cross-file navigation.
- Git `check-ignore` takes literal path arguments itself and rejects the global `--literal-pathspecs` flag. Keep this distinct from stage/reset/diff pathspec behavior.
- Tailwind scanning is explicitly source-scoped; do not scan `_ref`, build output or fixtures. An unexpected `.gitignore` working-tree change was observed and left untouched, not bundled into feature commits.
- Native watcher notifications must not cause Git status to rewrite the index and wake itself. `--no-optional-locks status` plus bounded coalescing avoids that loop.
- `onCloseRequested` in the Tauri JS API calls `destroy()` after an unprevented handler; the guarded-close path needs `core:window:allow-destroy` as well as allow-close. Verify actual process exit, not just the dialog.
- Resume the four-real-omp-worker memory measurement only when requested; no RAM claim is established by fixture workers.

## Local Git recovery checkpoint

`refs/checkpoints/terminal-cutover-paused` still holds the pre-integration snapshot of the six partial frontend files for recovery only. It is local, unpushed and superseded by the integrated commit; delete it once the cutover is confirmed.
