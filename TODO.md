# Minnow milestones

## Reference and identity
- [x] Clone Orca into `_ref/orca`; pin the revision and MIT license.
- [x] Trace worktrees, PTYs, status, notifications, resume, and layout in REFERENCE.md.
- [x] Use Minnow product/window name, `minnow` executable, and `com.minnow.app` identifier.
- [x] Preserve the supplied icon, generate Tauri assets, and verify native/executable icons.
- [x] Configure hanphanr1 authorship, the Minnow GitHub remote, and attribution.

## Orca interface replacement (current priority)
- [x] Study renderer components, spacing, typography, theme tokens, icons, states, and shortcuts.
- [x] Map screens/components to source paths in UI_PARITY.md.
- [x] Port Geist/Nerd fonts, light/dark tokens, Lucide geometry, and Windows chrome with license notices.
- [x] Replace the custom layout with project/workspace navigation and the Orca-shaped landing shell.
- [x] Implement real workspace creation, palette search/filtering, selection history, and sidebar presentation state.
- [x] Implement tabs, names, colors, pins, reordering, split terminals, and split tab groups.
- [x] Preserve visible-only terminals, native PTY leases, 2 MiB rings, and 64 KiB acknowledgement windows.
- [x] Implement appearance/terminal preferences and searchable settings/shortcut navigation.
- [x] Render actual Git patches and preserve real restart/stop/editor/merge/delete callbacks.
- [x] Keep excluded service-backed features as labelled shell-only pages.
- [x] Keep Rust and `src/api.ts` contracts unchanged; exclude the pre-existing ConPTY fix from UI commits.
- [ ] Finish desktop interaction proof and source/Minnow screen comparisons in `docs/parity/`.
- [ ] Commit the verified screen milestones and update the handoff with the resulting revisions.

## Existing native engine
SQLite project/session/settings storage, worktrees, ConPTY, bounded output, status detection, notifications, worker actions, and saved-session relaunch remain the existing Rust implementation. UI work does not replace or re-certify that engine. `src-tauri/src/pty.rs` contains an earlier, separate startup-handshake fix.

## Release evidence
- [x] Run frontend type checks and production builds during integration.
- [x] Run `cargo check --locked` after the chrome/config changes.
- [ ] Build and exercise the final packaged frontend in the release executable.
- [ ] Measure the full release process tree with four actual idle omp workers.
- [ ] Record real RAM numbers and accounting scope in README.md.

The user stopped the earlier backend/RAM verification and made interface parity the priority. The four-worker measurement remains unclaimed; the 150 MiB target is not a result.

## Design decisions

ENERGY 1 / RHYTHM 1 / MOTION 1. Geometry, palette, spacing, fonts, icons, and interaction patterns follow the explicitly requested Orca reference. Minnow retains its supplied identity. Native terminal content and Git patches are real; excluded services are clearly unavailable. Settings and diff views load on demand. Narrow desktop windows reflow without clipping controls; the 1440px reference layout retains 280px/350px sidebars, 36px titlebar, and 24px status bar.
