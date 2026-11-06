# omp-deck milestones

## 1. Reference and build setup
- [ ] Clone Orca into `_ref/orca`; record revision and license.
- [ ] Trace workers/worktrees, PTYs, status, notifications, resume, and layout in REFERENCE.md.
- [ ] Prepare Rust/MSVC, Svelte 5/Vite, and Tauri 2; commit the working baseline.

## 2. Native worker engine
- [ ] SQLite projects, workers, and settings.
- [ ] Git worktree creation, base diff, safe merge, and safe removal.
- [ ] ConPTY per omp worker; bounded 2 MiB output ring; visible-only output subscription.
- [ ] Running/idle/waiting/exited status, native notifications, restart, kill, editor launch.
- [ ] Persist worker session paths and resume after application restart.
- [ ] Exercise native lifecycle and commit.

## 3. Workspace interface
- [ ] Project sidebar, worker creation, grid and tabs.
- [ ] Visible-only xterm instances with WebGL and disposal; keyboard focus and resize.
- [ ] Worker actions, diff viewer, settings, empty/loading/error states.
- [ ] Exercise real desktop surface and commit.

## 4. Release evidence
- [ ] Cargo check, frontend checks/build, and release build with LTO and stripping.
- [ ] End-to-end lifecycle, recovery, and git safety scenarios.
- [ ] Measure the release application process tree with four actual idle omp workers.
- [ ] README: PowerShell build/run steps, measured RAM with accounting scope, known limitations.
- [ ] Final milestone commit.

## Design decisions

ENERGY 1 / RHYTHM 1 / MOTION 1. The terminal is the primary surface; a compact project rail and worker tabs support switching without dashboard filler. Dark neutrals match the requested omp terminal workflow. A restrained amber focus accent identifies the active worker. Segoe UI uses Windows' installed interface font; Cascadia Mono/Consolas preserve terminal alignment without downloaded fonts. Flat panels and tight spacing reserve pixels for terminal output. No animation or illustration is needed for this workload.
