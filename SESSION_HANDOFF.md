# Minnow: paused checkpoint and agent handoff

## Status

PAUSED at the user's explicit request. Stop implementation in this session; hand the project to the next agent. The last verified runnable code milestone is `1f4746c60c46cac2fa7cb6c0459d82672c1b7774`, and it was pushed to `origin/main`. The working tree now contains a partial terminal-identity migration that is known incomplete: Workspace.svelte and App.svelte still use the old model, and Rust has not been migrated. Do not call the current working tree buildable or fully Orca-compatible. Preserve the partial work instead of restarting the port.

The earlier four-real-omp-worker RAM measurement remains paused by the user's instruction. No memory result or claim below 150 MiB has been published. UI smoke workers run a deterministic native PTY fixture, not real omp inference sessions.

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
3. `TERMINAL_CUTOVER.md`: durable model/API/migration contract, agreed root interface, Windows cwd evidence and acceptance scenarios.
4. `TODO.md` and `UI_PARITY.md`: full remaining feature coverage, source citations, proof and known visual differences. Do not repeat cloning/icons/initial shell work.

## Paused worktree inventory

Both terminal-cutover workers stopped and yielded. No native files were edited during this slice. No checks/builds/native smoke were run after these partial frontend edits.

| File | Preserved partial state |
|---|---|
| `src/types.ts` | Adds TerminalKind/TerminalSession, Snapshot.terminals and TerminalChunk.terminal_id |
| `src/api.ts` | New terminal create/restart/kill/close/cwd wrappers, terminalId PTY parameters/leases and terminal update/removal subscriptions; worktree wrappers retained |
| `src/ui/workspace-layout.ts` | Rewritten terminal-identity model/helpers; version-3 per-worker persistence and legacy project-layout migration |
| `src/Terminal.svelte` | Mechanically migrated to terminal prop/identity for attach, leases, output and disposal |
| `src/ui/PaneTree.svelte` | Uses terminalById/terminal leaves and `data-pane-terminal` |
| `src/ui/WorkspaceTabStrip.svelte` | Uses terminal lookup/focus fields |
| `src/ui/Workspace.svelte` | NOT migrated; still imports old helpers such as addWorkerTab/tabWorkerIds/reconcileWorkers and old snapshot signatures. This is a known compile blocker, not an exercised new failure |
| `src/App.svelte` | NOT integrated with terminal collections/events or the agreed new Workspace props |
| `src-tauri/src/model.rs`, `storage.rs`, `pty.rs`, `lib.rs` | Untouched verified workerId-native baseline. TerminalSession/schema/commands/events/OSC7 handling do not exist in Rust yet |

The partial layout layer renamed activateWorker/splitWithNewWorker to activateTerminal/splitWithNewTerminal, and workerById/activeWorker/focusedWorkerId/pane workerId to terminal equivalents. Existing Workspace.svelte therefore needs a real migration, not compatibility re-exports. The new load/save signatures are `(projectId, workerId, ...)`. Reconciliation intentionally does not remove a closed=true terminal from an existing pane: the explicit close action must remove it after the native close completes, preserving failures and reopen history.

Exact recovery copy of the six source-file diffs, relative to `1f4746c`:
- File: `.smoke/checkpoints/terminal-cutover-paused.patch` (64,908 bytes).
- SHA-256: `4FE1A6B9A2809F4891B93D9D44E201349FD6F969C8F665296BBA3B1F5AC73EA6`.
- `git apply --reverse --check` succeeded against the preserved working tree; nothing was applied or reverted. Do NOT apply it again to this tree. It is a recovery copy for the verified baseline, not a completed feature or a public release.
- The unrelated `.gitignore` change was excluded from the patch and feature commits. Its observed contents are only `*.log`, exposing `_ref`, `.smoke`, node_modules, dist and Rust targets as untracked. Do not run `git add -A` or silently restore this user-owned change. Stage explicit source/docs paths only.

## Immediate implementation sequence for the next agent

1. Implement the native TerminalSession model/schema migration and independent PTY ownership from TERMINAL_CUTOVER.md, preserving old OMP session paths, desired-running state and Git worktree safety.
2. Migrate Workspace.svelte fully to the new terminal helpers/props. New/split creates a terminal in the selected worktree, not create_workers. Close/reopen must stop/restart only the correct terminal and preserve failed-close state.
3. Integrate App.svelte: subscribe and queue terminal events before bootstrap, maintain terminals independently of workers, filter by owning worker, key workspace state by selected worktree and wire the agreed callbacks. Never put a terminal ID into activeWorkerId.
4. Inspect the mechanically migrated components against real APIs; remove obsolete callsites rather than adding aliases. Verify the v1-to-v3 layout migration against actual saved shape.
5. Only after the cutover is integrated, run check/build/native tests and actual isolated desktop scenarios: three independent PIDs sharing one worktree, no extra Git worktrees from split/new, source OSC7 cwd inheritance, close one pane without stopping others, merge/delete safety across all terminals, reopen identity and cold resume excluding stopped/closed sessions.
6. Then continue the remaining Explorer strip/global filename query and proper file/diff tabs in workspace groups. The proposed find_workspace_files API is a design note only; no command or implementation was added.

Do not turn the preserved partial source into a 'working' commit before that verification. A docs-only pause checkpoint is separate from the last green code milestone.

## Implemented frontend

- Exact source theme tokens, Geist/Nerd fonts, Lucide geometry, 280px/350px reference sidebars, 36px Windows chrome, and 24px status bar.
- Project/workspace sidebar, pins/grouping/collapse, selection/history, real local project addition, composer with counts/Create more, and workspace palette with filtering/recency.
- Tabs, pin/title/color/reorder, nested source-shaped context menus, split panes, split tab groups, resizing, focused-pane expansion, and recent-tab switching.
- Visible-only xterm 6 with compatible fit/search/WebGL addons. Terminal engine, WebGL, Settings and DiffView load on demand.
- Actual Git patch list and inline/side-by-side diff, path copy, safe worker actions, source-shaped settings, and persisted supported appearance/terminal preferences.
- Real lazy Explorer/content search, CodeMirror file editing, SHA-256 revision-checked saves, real Git staged/unstaged/untracked status, stage/unstage, index-only commits and bounded history. Native project folder dialog, local clone and new Git project creation replace the old admission placeholders. See the separate native continuation evidence in UI_PARITY.md; successful synthetic native picker selection was not established.

Main files: `src/App.svelte`, `src/Terminal.svelte`, `src/app.css`, `src/orca-theme.css`, `src/ui/`, `src/fonts/`. Source/license mapping is in UI_PARITY.md and THIRD_PARTY_NOTICES.md.

## Backend boundary

At `1f4746c`, native files/Git/watch APIs and profile isolation are complete as described below; the original PTY engine still uses worker identity. The paused frontend has already moved its PTY API to terminalId, so the two sides are currently incompatible until the explicit cutover is finished.

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

- Verified baseline terminals were keyed by immutable worker ID. The paused cutover moves this to immutable terminal ID; ready/disposal callbacks must preserve that identity through removal and async teardown.
- Replacing tab arrays during insertion preserves Svelte invalidation after close/reopen. Avoid returning to in-place insertion that leaves an active tab absent from the rendered strip.
- Workspace overlays own focus while open; asynchronous PTY attachment must not focus a terminal through menus, rename fields, or confirmations.
- Shell-handled key events do not enter xterm. UI zoom is not intercepted while typing inside a terminal.
- xterm 6 uses its scrollable element; the compatibility viewport must stay transparent and non-scrolling.

## Runtime hygiene

Initial smoke state remains `.smoke/parity-state`; continuation state is `.smoke/functional-state`, with its own `webview/` and CDP port 9225. `.smoke/repo` is the fixture repository, and `functional-proof` is its continuation worktree. All its workers use `.smoke/pty-fixture.exe`, not real omp inference. New/clone admission created additional repositories only under `.smoke/`.

The normal user Minnow release/window and main Orca profile were not used for native feature tests. The isolated reference profile remains `.smoke/orca-parity-profile`. Close only owned services/windows after their proof; retain supplied artwork and original captures.

At this pause both implementation workers have stopped. The owned Vite service was stopped and owned browser connections released. The final reviewed Minnow smoke desktop had exited with code 0. Leftover isolated Orca reference processes were explicitly terminated by the owned profile's main PID; the user's normal Minnow release/main Orca session were not closed. No runtime is intentionally left running for the next agent.

For later GUI debugging, cold-reload/restart after Svelte HMR before interpreting stale derived state as a release failure. Python browser automation accepts JavaScript body strings, not an uninvoked arrow-function string. Clipboard helpers can return cached shim data; verify app copy through its actual native clipboard path and visible feedback.

## Remaining work and invariants

- Full feature parity is still open. The next architectural mismatch is many independent terminals per worktree: current tabs/splits still create a new worker/worktree. Migrate all native/frontend callers together rather than hiding that difference.
- Native picker opening/cancel was exercised, but synthetic path entry did not update its internal selection model. Typed-path admission and actual create/clone were proven; a manual picker-selection pass remains.
- FileEditor initializes by primitive `documentKey` with untracked prop reads. Same-file line updates must never reload/discard edits. CodeMirror serializes through the document's line-separator facet and `sliceDoc()` to retain CRLF. Native Close uses the same async guard as cross-file navigation.
- Git `check-ignore` takes literal path arguments itself and rejects the global `--literal-pathspecs` flag. Keep this distinct from stage/reset/diff pathspec behavior.
- Tailwind scanning is explicitly source-scoped; do not scan `_ref`, build output or fixtures. An unexpected `.gitignore` working-tree change was observed and left untouched, not bundled into feature commits.
- Native watcher notifications must not cause Git status to rewrite the index and wake itself. `--no-optional-locks status` plus bounded coalescing avoids that loop.
- `onCloseRequested` in the Tauri JS API calls `destroy()` after an unprevented handler; the guarded-close path needs `core:window:allow-destroy` as well as allow-close. Verify actual process exit, not just the dialog.
- Resume the four-real-omp-worker memory measurement only when requested; no RAM claim is established by fixture workers.

## Local Git recovery checkpoint

`refs/checkpoints/terminal-cutover-paused` preserves the tracked working-tree state separately from main. It is a local, stash-shaped recovery snapshot, not a green feature commit and not pushed. Creation does not reset, stash away or otherwise replace the working files. It includes the user-owned tracked `.gitignore` diff for recovery only, whereas the source patch intentionally excludes it. Untracked build output/reference/fixture directories are not included. The handoff documents are committed separately with an explicit path whitelist; the six partial source files stay modified in the working tree.

