# Minnow milestones

Current state: the terminal cutover (many independent terminals per worktree) is integrated and was exercised on the native desktop; SESSION_HANDOFF.md states exactly what was and was not proven. Terminal contract: TERMINAL_CUTOVER.md; next-agent prompt: CONTINUE_PROMPT.md. All unchecked parity work below remains open.

## Reference and identity
- [x] Clone Orca into `_ref/orca`; pin the revision and MIT license.
- [x] Trace worktrees, PTYs, status, notifications, resume, and layout in REFERENCE.md.
- [x] Use Minnow product/window name, `minnow` executable, and `com.minnow.app` identifier.
- [x] Preserve the supplied icon, generate Tauri assets, and verify native/executable icons.
- [x] Configure hanphanr1 authorship, the Minnow GitHub remote, and attribution.

## Orca shell milestone (published at `0997e90`)
- [x] Study renderer components, spacing, typography, theme tokens, icons, states, and shortcuts.
- [x] Map screens/components to source paths in UI_PARITY.md.
- [x] Port Geist/Nerd fonts, light/dark tokens, Lucide geometry, and Windows chrome with license notices.
- [x] Replace the custom layout with project/workspace navigation and the Orca-shaped landing shell.
- [x] Implement real workspace creation, palette search/filtering, selection history, and sidebar presentation state.
- [x] Implement tabs, names, colors, pins, reordering, split terminals, and split tab groups.
- [x] Preserve visible-only terminals, native PTY leases, 2 MiB rings, and 64 KiB acknowledgement windows.
- [x] Implement appearance/terminal preferences and searchable settings/shortcut navigation.
- [x] Render actual Git patches and preserve real restart/stop/editor/merge/delete callbacks.
- [x] Label service-backed placeholders honestly in the initial shell milestone.
- [x] Preserve the PTY buffering/acknowledgement contracts in the shell milestone.
- [x] Finish desktop interaction proof and 16 source/Minnow comparisons in `docs/parity/`.
- [x] Commit the verified screen milestones and record their revisions in UI_PARITY.md.

## Full parity continuation

The shell milestone is not complete Orca parity. The requested continuation includes the missing functions below; a screenshot or unavailable page does not complete a feature.

- [x] Push the existing `main` milestone to `https://github.com/hanphanr1/Minnow.git`.
- [ ] Native folder selection acceptance: opening/cancel works; successful selection still needs a manual pass.
- [x] Real local clone and Git project creation through the native app.
- [x] Lazy file explorer, content search and real UTF-8 editing with revision-checked saves.
- [x] Real staged/unstaged/untracked Git status, staging, commits and bounded history.
- [x] Multiple independent terminals in the same worktree, including split cwd inheritance (OSC 7 only; see SESSION_HANDOFF.md for the Windows evidence).
- [x] Native coalesced file-change invalidation for the visible workspace and Git index.
- [ ] Quick open, file operations and remaining explorer interactions.
- [x] Source-matched persistent Explorer Names/Contents strip, global filename filtering (native `find_workspace_files`), Git status/ignored decorations and dotfile/ignored toggles. Still open: file operations (new/rename/delete/drag), quick open and the row context menu.
- [x] File and diff tabs inside workspace groups, with independent dirty-document ownership (per-tab CodeMirror editors kept alive while hidden, close/leave guards, persisted layout). Reopen of a closed document tab is not implemented.
- [ ] Branch selection, nested workspaces, non-Git folder projects and attached worktrees.
- [ ] Source-control conflicts/discard, history pagination, diff comments and split-open behavior.
- [ ] Editable keyboard shortcuts with one runtime binding registry and conflict detection.
- [ ] Complete terminal/appearance/general settings and theme import/export.
- [ ] Notification preferences, real activity history and agent attention routing.
- [ ] Local quick commands, workspace space and floating workspaces.
- [ ] Agent session history/search, local skills and usage inspection.
- [ ] Tasks and integrations, with actual provider authentication and data.
- [ ] Durable automations with real worker dispatch and run history.
- [ ] Native chat/agent adapters and provider account management.
- [ ] On-demand browser pane and its navigation/session settings without Electron.
- [ ] SSH/server connections, ports and remote session lifecycle.
- [ ] Plugins, artifacts and shared skills backed by actual runtimes/services.
- [ ] Mobile/computer-use/voice capabilities with real device/runtime prerequisites.
- [ ] Setup, diagnostics, project-scoped settings and remaining reference settings.
- [ ] Recompare each implemented screen against Orca; record actual remaining mismatches.

Source entry points and dependency boundaries are tracked in UI_PARITY.md. External services, OAuth registrations, device access and downloaded runtimes are prerequisites, not behavior that can be replaced by fabricated local success.

## Existing native engine
SQLite project/session/settings storage, worktrees, ConPTY, bounded output, status detection, notifications, worker actions, and saved-session relaunch remain the existing Rust implementation. UI work does not replace or re-certify that engine. `src-tauri/src/pty.rs` contains an earlier, separate startup-handshake fix.

## Release evidence
- [x] Run frontend type checks and production builds during integration.
- [x] Run `cargo check --locked` after the chrome/config changes.
- [x] Build and exercise the embedded frontend in the native release executable, including actual worker restore and close/reopen.
- [ ] Measure the full release process tree with four actual idle omp workers.
- [ ] Record real RAM numbers and accounting scope in README.md.

The user stopped the earlier backend/RAM verification and made interface parity the priority. The four-worker measurement remains unclaimed; the 150 MiB target is not a result.

## Design decisions

ENERGY 1 / RHYTHM 1 / MOTION 1. Geometry, palette, spacing, fonts, icons, and interaction patterns follow the explicitly requested Orca reference. Minnow retains its supplied identity. Native terminal content and Git patches are real; excluded services are clearly unavailable. Settings and diff views load on demand. Narrow desktop windows reflow without clipping controls; the 1440px reference layout retains 280px/350px sidebars, 36px titlebar, and 24px status bar.
