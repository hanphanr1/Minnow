# Minnow UI parity checkpoint

## Status

The Orca frontend replacement is complete and committed. This is a runnable Tauri/Svelte application, not the intermediate state described by the old pause checkpoint. The final native release was built and exercised. Do not repeat the reference research, icon generation, or frontend replacement.

The earlier four-real-omp-worker RAM measurement remains paused by the user's instruction. No memory result or claim below 150 MiB has been published. UI smoke workers run a deterministic native PTY fixture, not real omp inference sessions.

## Repository and milestones

- Workspace: `C:/Users/DELL/Documents/ORCA TAURI`.
- Product/window: Minnow; binary: `minnow`; identifier: `com.minnow.app`.
- Author: `hanphanr1`; remote: `https://github.com/hanphanr1/Minnow.git`; branch: `main`.
- Identity/icon milestone: `13f9b81`. Supplied artwork and generated Tauri assets are preserved.
- Working frontend milestone: `24fcd27`.
- Twelve subsequent screen-evidence commits run from `547713e` through `1037ad1`. UI_PARITY.md indexes each screen and exact commit.
- Commits are local; this work did not push the branch.

## Implemented frontend

- Exact source theme tokens, Geist/Nerd fonts, Lucide geometry, 280px/350px reference sidebars, 36px Windows chrome, and 24px status bar.
- Project/workspace sidebar, pins/grouping/collapse, selection/history, real local project addition, composer with counts/Create more, and workspace palette with filtering/recency.
- Tabs, pin/title/color/reorder, nested source-shaped context menus, split panes, split tab groups, resizing, focused-pane expansion, and recent-tab switching.
- Visible-only xterm 6 with compatible fit/search/WebGL addons. Terminal engine, WebGL, Settings and DiffView load on demand.
- Actual Git patch list and inline/side-by-side diff, path copy, safe worker actions, source-shaped settings, and persisted supported appearance/terminal preferences.
- Excluded services remain labelled unavailable. Do not add Electron, browser/mobile/remote services, fake commit/staging behavior, or invented data under the parity task.

Main files: `src/App.svelte`, `src/Terminal.svelte`, `src/app.css`, `src/orca-theme.css`, `src/ui/`, `src/fonts/`. Source/license mapping is in UI_PARITY.md and THIRD_PARTY_NOTICES.md.

## Backend boundary

Rust logic, SQLite schema, `src/api.ts`, PTY leases, the 2 MiB ring, and the 64 KiB acknowledgement window were not changed by this UI work. Frontend-drawn window chrome required Tauri config/capability changes only.

`src-tauri/src/pty.rs` has a pre-existing uncommitted ConPTY startup-handshake fix. It was deliberately excluded from every frontend commit. Do not revert it or silently bundle it into UI documentation/screenshot commits.

## Verification and artifacts

- `npm.cmd run check`: zero Svelte/TypeScript errors and warnings.
- `npm.cmd run build` and `npm.cmd run release`: succeeded. The release uses the existing LTO/strip profile.
- `cargo check --locked`: passed.
- Final executable: `src-tauri/target/release/minnow.exe`.
- Native release exercise: restored actual worker processes; new/close/reopen; live terminal output; Search/Source Control/settings lazy routes; nested-menu Escape; no page errors in the final run. Owned Minnow smoke windows closed with exit code 0.
- Desktop interaction proof also covers create/count/Create more/duplicate failure, palette, navigation/history, pin/title/color/reorder, horizontal/vertical splits, group moves/resize, expansion, terminal selection/copy/paste/search/clear, real Git patches, themes/zoom, and native maximize/restore.
- Screenshots: 16 labelled side-by-side comparisons plus original captures in `docs/parity/`. See the comparison index in UI_PARITY.md. Originals are 1440×900.
- Reference source: Orca 1.4.214 at `e507dd7732139e302f6adaa2917d8e0aee5b1362`. Genuine reference captures use installed Orca 1.4.245; captions disclose the difference.

## Lifecycle invariants

- Terminal instances are keyed by immutable worker ID. Ready/disposal callbacks retain that identity even when a worker disappears from the roster.
- Replacing tab arrays during insertion preserves Svelte invalidation after close/reopen. Avoid returning to in-place insertion that leaves an active tab absent from the rendered strip.
- Workspace overlays own focus while open; asynchronous PTY attachment must not focus a terminal through menus, rename fields, or confirmations.
- Shell-handled key events do not enter xterm. UI zoom is not intercepted while typing inside a terminal.
- xterm 6 uses its scrollable element; the compatibility viewport must stay transparent and non-scrolling.

## Runtime hygiene

Smoke state is isolated in `.smoke/parity-state`; the test repository is `.smoke/repo`. Four retained fixture worker records are not production omp sessions. Their memory must not be used for the original RAM acceptance target. The user's normal Minnow data directory and main Orca session were not used as the smoke profile.

The isolated reference profile is `.smoke/orca-parity-profile`. Comparison browser connections were released, the owned reference service was stopped, and the owned Vite service was stopped. Temporary shortcut extraction and obsolete/misnamed captures were removed. Original supplied artwork and binaries were not removed.

For later GUI debugging, cold-reload/restart after Svelte HMR before interpreting stale derived state as a release failure. Python browser automation accepts JavaScript body strings, not an uninvoked arrow-function string. Clipboard helpers can return cached shim data; verify app copy through its actual native clipboard path and visible feedback.

## Remaining work outside this completed UI task

- Resume the four-real-omp-worker memory measurement only when requested; report the full process tree and accounting scope with real numbers.
- Decide separately whether to commit the earlier Rust handshake fix and push local commits to GitHub.
- Full service-backed Orca features, native folder selection, staging/history/full-file editing, and custom shortcut rebinding remain documented exclusions, not claims of implemented behavior.
