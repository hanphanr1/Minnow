# Orca feature inventory and Minnow status

Inventory of every feature area in the pinned Orca source (`_ref/orca/src/renderer/src/components/*` and `components/settings/*`), with Minnow's status. Legend:

- **Done**: implemented natively and exercised on the desktop.
- **Local**: implementable with local resources only; queued.
- **External**: needs a service, account, device or downloaded runtime that Minnow does not have. These are not faked; they stay as labelled unavailable shells until their prerequisite exists.

## Workspaces and terminals

| Orca area | Status |
|---|---|
| Projects, worktree creation, palette, history, pins/groups (`sidebar`, `new-workspace`, `worktree-creation`, `worktree-jump`) | Done |
| Many terminals per worktree, splits, tab groups, drag/drop, pins, colours, reopen (`tab-bar`, `tab-group`, `terminal-pane`) | Done |
| Split cwd inheritance (OSC 7) | Done |
| Terminal quick commands (`terminal-quick-commands`, settings `quick-commands`) | Done (this pass) |
| Floating terminal (`floating-terminal`) | Done (this pass) |
| Workspace cleanup of merged worktrees (`workspace-cleanup`) | Done (this pass) |
| Workspace emoji / board / dashboard (`workspace-emoji`, `dashboard`, `dashboard-popout`) | Local, partly (board/dashboard queued) |
| Floating workspaces (`floating-workspaces`) | Local, queued |
| Sparse checkout, WSL drives/hosts (`sparse`, `wsl-*`) | External (WSL/sparse repos not present) |
| SSH, remote hosts, servers, plain SSH terminal (`ssh-*`, `host-picker`, `plain-ssh-terminal`, `runtime-access`) | External |

## Files, editor and Git

| Orca area | Status |
|---|---|
| Explorer: Names/Contents, Find files, decorations, toggles | Done |
| Explorer file operations, context menu, reveal | Done (duplicate / drag-move / multi-select queued) |
| Quick Open (Ctrl+P) | Done |
| Editor tabs inside groups, dirty guards (`editor`) | Done |
| Content search | Done |
| Source control: status, stage/unstage, commit, history, discard | Done |
| Source control: history pagination, conflict section | Done (this pass) |
| Diff comments (`diff-comments`) | Local, queued |
| Markdown preview tabs | Local, queued |
| GitHub / GitLab items, PR page, checks, hosted review (`github*`, `gitlab*`, `pull-request-page`, `checks-panel`) | External (provider auth) |

## Agents and automation

| Orca area | Status |
|---|---|
| omp agent terminals, status detection, resume | Done |
| Agent session history and search (`settings/session-history`, `agent-session-continuation`) | Done (this pass, local omp session files) |
| Activity / notifications history and preferences (`activity`, `notifications`) | Done (this pass) |
| Usage stats (`stats`) | Done (this pass, local counts only) |
| Automations (`automations`) | Local, queued |
| Ports panel (`ports`) | Done (this pass, local listening ports) |
| Native chat, provider accounts, orchestration crew, managed runtime (`native-chat`, `provider-accounts`, `orchestration-crew`, `managed-runtime`) | External |
| Skills, artifacts, plugins (`skills`, `artifacts`, `plugins`) | External runtimes; local skill listing queued |
| Tasks / integrations (`task-page`, `integrations`, `github-project`) | External |
| Browser pane, mobile emulator, computer use, voice/dictation, mobile, pet (`browser-pane`, `emulator-pane`, `dictation`, `mobile`, `pet`) | External or out of scope (no Electron; device/runtime prerequisites) |

## Settings

| Section | Status |
|---|---|
| Appearance, Terminal, General, Shortcuts (view) | Done |
| Quick Commands, Notifications, Session History, Stats, Git | Done (this pass) |
| Editable shortcuts with conflict detection | Done (this pass) |
| Agents, Accounts, Orchestration, Computer Use, Voice, Integrations, Mobile, Artifacts, Skills, Tasks, Browser, Emulator, Chat, SSH, Servers, Privacy, Plugins | External / labelled unavailable |
