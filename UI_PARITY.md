# Orca UI parity

Reference: Orca 1.4.214, commit `e507dd7732139e302f6adaa2917d8e0aee5b1362` in `_ref/orca`. Paths in the Source column are relative to `_ref/orca/src/renderer/src` unless stated otherwise. Checked rows were exercised and visually reviewed in the native Minnow desktop. They do not imply implementation of Orca's excluded services.

## Contract

- Preserve the existing Rust backend, SQLite schema, IPC methods, 2 MiB terminal ring, 64 KiB acknowledgement window, and terminal subscription leases.
- Replace the custom Minnow layout with the Orca shell. Retain the Minnow product name and supplied app icon.
- Copy Orca theme tokens, Geist and Nerd Symbols fonts, and Lucide 0.577.0 icon geometry under their original licenses. See `THIRD_PARTY_NOTICES.md`.
- Browser, mobile/emulator, remote hosts, native chat, external integrations and other excluded heavy features retain their shell entry points with explicit unavailable-state content. They do not start services or imitate successful actions.
- Each backend worker owns one worktree and one PTY. Frontend tab names, pins, ordering, split groups and widths are presentation state. Creating another terminal launches another worker through the existing API; it does not claim to create a second PTY inside the same worktree.
- Source control reads the real unified diff returned by `diff_worker`. It does not invent file contents, stages, commit history or provider data.

Design direction: faithful Orca desktop port. ENERGY 1 / RHYTHM 1 / MOTION 1. Geometry, typography, iconography and state colors follow the reference, rather than a new Minnow design system.

## Main-screen checklist

| Verified | Orca screen/component | Minnow equivalent | Source |
|---|---|---|---|
| [x] | Application layout and Windows titlebar | `src/App.svelte`, `src/ui/WindowControls.svelte` | `App.tsx:89-135`, `app-shell/AppWorkspaceShell.tsx:127-303`, `app-shell/WindowControls.tsx:21-61` |
| [x] | Left sidebar chrome, navigation, workspace list, footer | `src/ui/Sidebar.svelte`, `src/ui/sidebar/` | `components/sidebar/`, `app-shell/AppWorkspaceShell.tsx:167-196` |
| [x] | Workspace selection, hover, active rows and status indicators | `src/ui/sidebar/SidebarWorkerRow.svelte` | `components/sidebar/worktree-list/`, `components/sidebar/workspace-status/` |
| [x] | Landing with no active workspace | `src/ui/Landing.svelte` | `components/Landing.tsx:216-324` |
| [x] | Project addition and workspace creation | `src/App.svelte`, `src/ui/CreateWorkspace.svelte` | `components/sidebar/project-admission/dialog/AddRepoStartSteps.tsx`, `components/NewWorkspaceComposerModal.tsx`, `components/NewWorkspaceComposerCard.tsx`, `components/new-workspace/` |
| [x] | Creation pending and failure states | `src/ui/CreateWorkspace.svelte` | `components/worktree-creation/WorktreeCreationPanel.tsx`, `WorktreeCreationTabStrip.tsx` |
| [x] | Titlebar tabs and new-tab menu | `src/ui/Workspace.svelte`, `WorkspaceTabStrip.svelte` | `components/TerminalTitlebarTabs.tsx:66-137`, `components/tab-bar/tab-bar-surface.tsx:113-288` |
| [x] | Tab active/hover/overflow/close/pin/title/color states | `src/ui/Workspace.svelte`, `WorkspaceTabStrip.svelte`, `workspace-layout.ts` | `components/tab-bar/SortableTab.tsx:222-346`, `SortableTabContextMenu.tsx:145-238`, `use-tab-strip-slot-props.ts:15-19` |
| [x] | Split groups, resize handles and focus | `src/ui/PaneTree.svelte`, `Workspace.svelte`, `workspace-layout.ts` | `components/tab-group/TabGroupSplitLayout.tsx:12-45`, `TabGroupSplitNodeTree.tsx:14-262`, `TabGroupPanel.tsx:257-271` |
| [x] | Terminal panes, pane headers, search and context menu | `src/Terminal.svelte`, `src/ui/PaneTree.svelte` | `assets/terminal.css`, `components/terminal-pane/TerminalPaneHeaderOverlay.tsx`, `TerminalContextMenu.tsx`, `components/TerminalSearch.tsx:150-158` |
| [x] | Recent-tab switcher | `src/ui/Workspace.svelte` | `components/tab-bar/RecentTabSwitcher.tsx:57-176` |
| [x] | Workspace switch/search palette | `src/ui/WorkspacePalette.svelte` | `components/worktree-jump/worktree-jump-palette-surface.tsx`, `components/cmd-j/` |
| [x] | Right activity bar and sidebar | `src/ui/RightSidebar.svelte` | `components/right-sidebar/`, `app-shell/AppWorkspaceShell.tsx:288-303` |
| [x] | Source-control changes and diff view | `src/ui/RightSidebar.svelte`, `DiffView.svelte`, `diff.ts` | `components/right-sidebar/source-control/panel/panel.tsx:6-39`, `panel-content.tsx:7-201`, `components/editor/diff/sections/`, `components/editor/ChangesModeView.tsx:16-110` |
| [x] | Settings layout, navigation and search | `src/ui/Settings.svelte`, `src/ui/settings/` | `components/settings/Settings.tsx`, `components/settings/` |
| [x] | Appearance and terminal preferences | `src/ui/settings/AppearancePane.svelte`, `TerminalPane.svelte`, `src/ui/preferences.ts` | `components/settings/terminal/`, appearance settings sources below |
| [x] | Keyboard shortcut settings | `src/ui/settings/ShortcutsPane.svelte`, `shortcut-catalog.ts` | `components/settings/shortcuts/`, `_ref/orca/src/shared/keybindings/definitions-core-1.ts` through `definitions-core-4.ts` |
| [x] | Confirmation, dropdown, tooltip and toast surfaces | `src/App.svelte`, `Workspace.svelte`, `src/ui/sidebar/` | `components/confirmation-dialog.tsx`, `components/ui/`, `components/NotificationCardStack.tsx` |
| [x] | Excluded feature shell pages | labelled pages in `src/App.svelte`, `src/ui/settings/UnavailablePane.svelte` | `app-shell/AppRootSurfaces.tsx`, `components/browser-pane/`, `components/mobile/`, `components/automations/`, `components/plugins/`, `components/task-page/`, `components/stats/` |

## Shared visual contract

| Element | Orca value/source | Minnow location |
|---|---|---|
| Light/dark tokens | `assets/main.css:189-504` | `src/orca-theme.css` (direct port) |
| Tailwind theme bindings | `assets/main.css:46-181` | `src/orca-theme.css` (direct port) |
| Geist variable font | `assets/main.css:14-20`, `assets/fonts/Geist-Variable.woff2` | `src/fonts/Geist-Variable.woff2` |
| Nerd Symbols | `assets/main.css:22-29` | `src/fonts/SymbolsNerdFontMono-Regular.woff2` |
| Radius scale | base `0.625rem`; sm ×0.6, md ×0.8, lg ×1, xl ×1.4 | `src/orca-theme.css` |
| Windows titlebar | 36 px, 3 controls ×46 px | `app-shell/app-window-chrome.ts:15-17`; frontend chrome |
| Tab strip | 32 px plus 4 px top drag strip in split layout | workspace components |
| Tab width | min 144 px, max 300 px | `components/tab-bar/tab-width-rules.ts:2`; workspace tabs |
| Split-group divider | 6 px hit region, 1 px visible line, ratio clamp 0.15–0.85 | `assets/main.css:1010-1055`; split renderer |
| Terminal pane | 4 px padding, 24 px title overlay, 13 px mono title | `assets/terminal.css:222-283,508-527`; terminal components |
| New-tab menu | 288 px, 11 px radius, 12 px rows | `components/tab-bar/tab-bar-surface.tsx:219-234`; workspace menu |


### Settings categories and excluded routes

Navigation sources: `hooks/useSettingsNavigationMetadata.ts`, `lib/settings-navigation-types.ts`, `components/settings/navigation/settings-navigation-foundations.ts`, `components/settings/page/settings-page-renderer.tsx`. The reference groups categories as AI Capabilities, Set Up, Workflows, Interface, Remote Hosts, Privacy & Security, Advanced, Experimental, and Projects.

| Category/route | Orca source | Minnow mapping |
|---|---|---|
| General | `components/settings/general/GeneralPane.tsx`, `GeneralWorkspaceSettingsSection.tsx` | Settings page; actual OMP defaults and explicit fixed safety policies |
| Appearance | `components/settings/appearance/AppearancePane.tsx`, `AppearanceSection.tsx`, `AppearanceInterfaceSection.tsx`, `AppearanceWindowSidebarSection.tsx` | Interface/Terminal/Window accordion cards and persisted supported preferences |
| Terminal | `components/settings/terminal/TerminalPane.tsx`, `TerminalRenderingSection.tsx`, `TerminalInteractionSection.tsx` | OMP executable, terminal preferences, fixed renderer/buffer policy |
| Shortcuts | `components/settings/shortcuts/ShortcutsPane.tsx`, `ShortcutFilterRail.tsx`, `ShortcutRowsList.tsx`, `ShortcutCommandBlock.tsx` | Searchable source-matched Windows shortcut inventory |
| Agents, accounts, integrations, browser, chat, input, voice, orchestration | `components/settings/`, `settings-navigation-types.ts` | Same category navigation; labelled excluded capabilities |
| Git, tasks, quick commands, notifications, automations, artifacts, session history, share skills | same navigation contract; respective `components/settings/` folders | Supported worker actions remain in workspace; unsupported settings labelled unavailable |
| SSH, servers, mobile, mobile emulator, computer use, developer permissions | `settings-navigation-remote-sections.ts`, respective settings folders | Shell-only; no remote connection, device operation or permission changes |
| Privacy, advanced, developer, experimental, plugins, stats, Orca account, Linear, setup guide, project settings | `settings-navigation-types.ts`, `components/settings/` | Navigation/heading parity with explicit unsupported content |
| Tasks, Activity, Automations, Workspace Space, Skills, Artifacts, Mobile | `components/task-page/`, `activity/`, `automations/`, `workspace-space/`, `skills/`, `artifacts/`, `mobile/` | Main-shell placeholder pages; no fabricated tasks/accounts/metrics |
| Explorer, Search, Vault, Workspaces, PR Checks, Source Control, Checks, Ports, plugin panels | `components/right-sidebar/`, `_ref/orca/src/shared/ui-chrome-types.ts:98-108` | Activity-bar shell; real unified diff where backend supports it; explicit unavailable states otherwise |

Settings layout: 280 px navigation sidebar, source group ordering, row controls, source-width content bands. Appearance uses the original three accordions; Shortcuts uses the original filter rail/list layout. Menu/dialog recipes come from `components/ui/dialog.tsx`, `dropdown-menu.tsx`, `context-menu.tsx`, `command.tsx`.

The executable available for authentic captures is installed Orca **1.4.245**, not the pinned source **1.4.214**. Captures use a new isolated profile under `.smoke/orca-parity-profile`, never a copy of the user's credentials or workspace history. Screenshot captions must retain this version distinction.
## Windows keyboard parity

Source: `_ref/orca/src/shared/keybindings/definitions-core-{1,2,3,4}.ts`. Ctrl is Orca's Windows Mod key. Settings and tab rename are unbound by default on Windows; do not invent Ctrl+, or steal Ctrl+R from the terminal.

- [x] Ctrl+N / Ctrl+Shift+N: create workspace.
- [x] Ctrl+Shift+Up / Down, Ctrl+1…9: select workspace.
- [x] Ctrl+B / Ctrl+L: toggle left/right sidebar.
- [x] Ctrl+Shift+E / F / G: explorer/search/source control; unsupported file search is explicitly labelled.
- [x] Ctrl+Shift+0: focus workspace list; Ctrl+Shift+H: collapse/expand projects.
- [x] Ctrl+Alt+Left / Right: selection history.
- [x] Ctrl+T: new terminal; Ctrl+W: close focused pane (or a single-pane tab); Ctrl+Shift+T: reopen.
- [x] Ctrl+PageUp / Down, Ctrl+Shift+[ / ], Ctrl+Alt+[ / ]: previous/next tab.
- [x] Alt+1…9: tab by index; Ctrl+Tab: recent-tab overlay.
- [x] Ctrl+Shift+D / Alt+Shift+D: split right/down.
- [x] Ctrl+[ / ]: pane focus; Ctrl+Shift+Enter: expand/collapse focused pane.
- [x] Ctrl+F: terminal search; Ctrl+K: clear; Ctrl+Shift+A: select all.
- [x] Ctrl+C / Ctrl+Shift+C: copy selection; Ctrl+V / Ctrl+Shift+V / Shift+Insert: paste.
- [x] Ctrl+Shift+B / M / O: excluded browser/markdown shell entry points.
- [x] Ctrl+Shift+Y: excluded Plugins shell.
- [x] Ctrl+= / - / 0: UI zoom outside terminal panes; terminal input is not intercepted.
- [x] Escape dismisses overlays and restores focus; menu arrow keys and tab navigation work without a mouse.

## Screenshot and commit protocol

Store original Orca captures, Minnow captures and labelled side-by-side composites in `docs/parity/`. Record viewport, theme, source version and any content/backend differences. Do not substitute a reconstructed mockup for an Orca screenshot. A screen is checked and committed only after the running Minnow screen is compared with the reference.

Main comparisons cover landing, project addition, workspace creation, palette, terminal tabs, split terminals, split tab groups, source control, dark/light diffs, dark/light appearance settings, terminal settings, shortcuts, and the excluded Tasks/Mobile shell pages.

## Evidence

Sixteen labelled comparisons and their original native captures are saved under `docs/parity/`. Orca captures come from the isolated installed 1.4.245 application; Minnow captures use the Tauri desktop in debug or release mode, not a browser mockup. Terminal fixtures are real Rust-owned ConPTY processes, not real omp inference sessions. Their output is deliberately distinguishable from the reference's PowerShell sessions.

Verified interactions include local project addition and inaccessible-path feedback; named/count creation and Create more; duplicate-name rejection; palette search/filter/recency; selection/history; tab pin/name/color/reorder and nested menus; new/close/reopen; horizontal/vertical splits, tab-group moves and resizing; pane focus/expansion; terminal search, selection/copy/paste and clear; actual Git patch rendering, inline/side-by-side toggle, path copy and worktree safety feedback; theme/zoom/settings search/persistence; native maximize/restore/close.

## Deliberate differences

- Branding is Minnow. One backend worker still owns one worktree and one PTY; another terminal/split creates another worker. Orca can have several terminals in the same worktree.
- The composer supports local names/counts and fixed OMP launch. Provider/issue/branch-selection controls are disabled rather than pretending to work.
- Project addition accepts a repository path. Native folder selection, clone/create-project services, and remote hosts are not added.
- Explorer/file search and service-backed categories are explicit unavailable states. Tasks and Mobile comparisons establish the shared frame, not the excluded page content.
- Source Control uses real bounded patches. Staging, commits, PRs and history are labelled unavailable. Diff views render known patch lines only, without importing Monaco or inventing full file contents.
- Shortcut settings are a read-only inventory of 96 defaults in the pinned source. The newer installed reference exposes 139; that version difference is visible in the comparison.
- Narrow settings windows stack navigation above content; the normal reference-size desktop retains the source sidebar/card geometry.

## Native verification

- Frontend checks: zero TypeScript/Svelte errors and warnings. Production builds and native release builds succeeded.
- `cargo check --locked` passed. No Rust or `src/api.ts` changes were made for this UI port. The earlier uncommitted ConPTY handshake fix in `src-tauri/src/pty.rs` is excluded from frontend commits.
- Cold release proof caught and fixed a stale tab-list update: closing/reopening now restores the visible terminal and restarts its actual worker. Pane close stops only the focused worker; the worktree remains.
- Live smoke caught and fixed zero-width flex terminals, premature terminal autofocus during menus, teardown identity errors, xterm 5's disposed-viewport callback, and shell shortcuts leaking into PTY input. Compatible xterm 6/addons replace the old renderer packages.
- Only visible panes mount xterm. Switching hidden tabs disposes their browser-side instances; Rust retains output. The ring/lease/64 KiB acknowledgement API is unchanged.
- At 520/960/1180/1440px, shell center widths were 300/400/550/810px with no horizontal page overflow. Compact sidebars remain toggleable. Settings controls also reflow at 520px.
- The terminal engine, WebGL addon, Settings and DiffView load on demand. A fresh release landing did not fetch those modules. Built entry JS is about 345 kB (about 105 kB gzip); this is bundle size, not RAM.
- Checked text pairings passed normal-text AA: `#A1A1A1/#2A2A2A` 5.56:1; `#ACACAC/#171717` 7.90:1; `#F0F0F0/#0A0A0A` 17.37:1; `#696969/#F5F5F5` 5.04:1. This is not a full accessibility certification.
- Verification gate: source/attribution PASS; real-data and labelled-exclusion checks PASS; desktop geometry/themes/keyboard checks PASS; native runtime/build evidence PASS.

The original four-real-omp-worker RAM measurement remains paused at the user's direction. No RAM target or performance comparison is claimed by these screenshots.
