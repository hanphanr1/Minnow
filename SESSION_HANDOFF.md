# Minnow Orca parity continuation

## Status

The published Orca-shaped shell is not full visual/functional parity. The user reviewed it, requested all missing functionality, and requested the existing milestone be pushed. `main` through `0997e90` is on GitHub. The continuation adds real native files/search/editing, project creation/clone and Git operations; open families are explicitly tracked in TODO.md and UI_PARITY.md. Continue from this implementation rather than repeating reference/icon work.

The earlier four-real-omp-worker RAM measurement remains paused by the user's instruction. No memory result or claim below 150 MiB has been published. UI smoke workers run a deterministic native PTY fixture, not real omp inference sessions.

## Repository and milestones

- Workspace: `C:/Users/DELL/Documents/ORCA TAURI`.
- Product/window: Minnow; binary: `minnow`; identifier: `com.minnow.app`.
- Author: `hanphanr1`; remote: `https://github.com/hanphanr1/Minnow.git`; branch: `main`.
- Identity/icon milestone: `13f9b81`. Supplied artwork and generated Tauri assets are preserved.
- Working frontend milestone: `24fcd27`.
- Twelve subsequent screen-evidence commits run from `547713e` through `1037ad1`. UI_PARITY.md indexes each screen and exact commit.
- `0997e90` was pushed successfully to `origin/main`; the branch tracks `https://github.com/hanphanr1/Minnow.git`.
- Separate native handshake correction: `f122314`, with actual ConPTY smoke and regression coverage. New feature commits after the published milestone are not yet claimed pushed.

## Implemented frontend

- Exact source theme tokens, Geist/Nerd fonts, Lucide geometry, 280px/350px reference sidebars, 36px Windows chrome, and 24px status bar.
- Project/workspace sidebar, pins/grouping/collapse, selection/history, real local project addition, composer with counts/Create more, and workspace palette with filtering/recency.
- Tabs, pin/title/color/reorder, nested source-shaped context menus, split panes, split tab groups, resizing, focused-pane expansion, and recent-tab switching.
- Visible-only xterm 6 with compatible fit/search/WebGL addons. Terminal engine, WebGL, Settings and DiffView load on demand.
- Actual Git patch list and inline/side-by-side diff, path copy, safe worker actions, source-shaped settings, and persisted supported appearance/terminal preferences.
- Real lazy Explorer/content search, CodeMirror file editing, SHA-256 revision-checked saves, real Git staged/unstaged/untracked status, stage/unstage, index-only commits and bounded history. Native project folder dialog, local clone and new Git project creation replace the old admission placeholders. See the separate native continuation evidence in UI_PARITY.md; successful synthetic native picker selection was not established.

Main files: `src/App.svelte`, `src/Terminal.svelte`, `src/app.css`, `src/orca-theme.css`, `src/ui/`, `src/fonts/`. Source/license mapping is in UI_PARITY.md and THIRD_PARTY_NOTICES.md.

## Backend boundary

The shell milestone left Rust/SQLite/`src/api.ts` unchanged. The continuation adds `workspace-api.ts`, `workspace-watch.ts`, native `projects.rs`, `workspace.rs`, `workspace_watch.rs`, root-scoped command wrappers and one `Manager::worktree_path` accessor. Existing PTY rings, leases and acknowledgement windows are unchanged.

The earlier ConPTY startup-handshake correction is now committed separately as `f122314`, not silently bundled into UI work. Isolated `MINNOW_DATA_DIR` runs now isolate their actual WebView2 profile through the native builder; Tauri 2.12's configuration conversion omitted `dataDirectory` and otherwise collided with an already-running release.

## Verification and artifacts

- `npm.cmd run check`: zero Svelte/TypeScript errors and warnings.
- `npm.cmd run build` and `npm.cmd run release`: succeeded. The release uses the existing LTO/strip profile.
- `cargo check --locked`: passed.
- Existing release executable: `src-tauri/target/release/minnow.exe`; the user's review instance was left running. The new local-feature continuation was exercised with the debug native executable and isolated data, not misreported as a rebuilt release.
- Native release exercise: restored actual worker processes; new/close/reopen; live terminal output; Search/Source Control/settings lazy routes; nested-menu Escape; no page errors in the final run. Owned Minnow smoke windows closed with exit code 0.
- Desktop interaction proof also covers create/count/Create more/duplicate failure, palette, navigation/history, pin/title/color/reorder, horizontal/vertical splits, group moves/resize, expansion, terminal selection/copy/paste/search/clear, real Git patches, themes/zoom, and native maximize/restore.
- Screenshots: 16 labelled side-by-side comparisons plus original captures in `docs/parity/`. See the comparison index in UI_PARITY.md. Originals are 1440×900.
- Reference source: Orca 1.4.214 at `e507dd7732139e302f6adaa2917d8e0aee5b1362`. Genuine reference captures use installed Orca 1.4.245; captions disclose the difference.
- Reviewed continuation frontend check/build passed with zero errors/warnings; 27 Windows-native tests passed. Cold native proof covers actual create/clone, filesystem watching, file edits/saves, CRLF retention, stale revision refusal, same-file search navigation, native Close/Cancel, delete-shortcut dirty guard, staged commit/history and untracked-file diff. Additional 1440×900 native captures are `docs/parity/minnow-functional-*.png`.

## Lifecycle invariants

- Terminal instances are keyed by immutable worker ID. Ready/disposal callbacks retain that identity even when a worker disappears from the roster.
- Replacing tab arrays during insertion preserves Svelte invalidation after close/reopen. Avoid returning to in-place insertion that leaves an active tab absent from the rendered strip.
- Workspace overlays own focus while open; asynchronous PTY attachment must not focus a terminal through menus, rename fields, or confirmations.
- Shell-handled key events do not enter xterm. UI zoom is not intercepted while typing inside a terminal.
- xterm 6 uses its scrollable element; the compatibility viewport must stay transparent and non-scrolling.

## Runtime hygiene

Initial smoke state remains `.smoke/parity-state`; continuation state is `.smoke/functional-state`, with its own `webview/` and CDP port 9225. `.smoke/repo` is the fixture repository, and `functional-proof` is its continuation worktree. All its workers use `.smoke/pty-fixture.exe`, not real omp inference. New/clone admission created additional repositories only under `.smoke/`.

The normal user Minnow release/window and main Orca profile were not used for native feature tests. The isolated reference profile remains `.smoke/orca-parity-profile`. Close only owned services/windows after their proof; retain supplied artwork and original captures.

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
