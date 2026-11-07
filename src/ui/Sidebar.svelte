<!--
  Left sidebar. Ported from Orca 1.4.214 components/sidebar (MIT):
  TitlebarLeftControls, SidebarNav, SidebarHeader, WorktreeList/SectionHeader,
  WorktreeCard and SidebarToolbar. Grouping, collapse and pins are Orca
  presentation state; Minnow persists them locally instead of a backend store.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import {
    ArrowLeft,
    Bell,
    ArrowRight,
    BookOpen,
    CalendarClock,
    ChevronsDownUp,
    ChevronsUpDown,
    CircleDashed,
    CircleQuestionMark,
    Crosshair,
    Ellipsis,
    ExternalLink,
    FileDiff,
    FolderPlus,
    Github,
    GitMerge,
    Kanban,
    Keyboard,
    List,
    MessageSquareText,
    PanelLeft,
    Pin,
    Play,
    Plus,
    RefreshCw,
    RotateCw,
    ScrollText,
    Search,
    Settings,
    SlidersHorizontal,
    Smartphone,
    Square,
    Trash2
  } from 'lucide-svelte';
  import minnowMark from '../../src-tauri/icons/32x32.png';
  import type { Project, Worker } from '../types';
  import type { WorkerAction } from './workspace-layout';
  import SidebarKeyCombo from './sidebar/SidebarKeyCombo.svelte';
  import SidebarMenu from './sidebar/SidebarMenu.svelte';
  import SidebarProject from './sidebar/SidebarProject.svelte';
  import SidebarTip from './sidebar/SidebarTip.svelte';
  import SidebarWorkerRow from './sidebar/SidebarWorkerRow.svelte';
  import {
    loadSidebarPresentation,
    pinnedItemKey,
    projectItemKey,
    saveSidebarPresentation,
    sidebarItemSelector,
    toggleListValue,
    workerItemKey,
    type MenuAnchor,
    type SidebarGroupBy,
    type SidebarMenuItem,
    type SidebarPresentation
  } from './sidebar/presentation';

  let {
    projects,
    workers,
    selectedProjectId,
    activeWorkerId,
    activeView,
    onSelect,
    onCreate,
    onAddProject,
    onSearch,
    onSettings,
    onToggle,
    onBack,
    onForward,
    canBack,
    canForward,
    onFeature,
    onWorkerAction
  }: {
    projects: Project[];
    workers: Worker[];
    selectedProjectId: string | null;
    activeWorkerId: string | null;
    activeView: string;
    onSelect: (projectId: string, workerId?: string) => void;
    onCreate: () => void;
    onAddProject: () => void;
    onSearch: () => void;
    onSettings: () => void;
    onToggle: () => void;
    onBack: () => void;
    onForward: () => void;
    canBack: boolean;
    canForward: boolean;
    onFeature: (name: string) => void;
    onWorkerAction: (action: WorkerAction, worker: Worker) => void;
  } = $props();

  interface OpenMenu {
    anchor: MenuAnchor;
    side: 'bottom' | 'top' | 'right';
    width: number;
    label: string;
    items: SidebarMenuItem[];
    restoreTo: HTMLElement | null;
  }

  let presentation = $state<SidebarPresentation>(loadSidebarPresentation());
  let menu = $state<OpenMenu | null>(null);
  let scrollEl = $state<HTMLDivElement | null>(null);
  let focusedKey = $state<string | null>(null);
  const attentionCount = $derived(workers.filter(worker => worker.status === 'waiting-for-input').length);

  $effect(() => {
    saveSidebarPresentation({
      groupBy: presentation.groupBy,
      collapsedProjectIds: [...presentation.collapsedProjectIds],
      pinnedWorkerIds: [...presentation.pinnedWorkerIds],
      pinnedCollapsed: presentation.pinnedCollapsed
    });
  });

  const projectsById = $derived(new Map(projects.map((project) => [project.id, project])));
  const pinnedSet = $derived(new Set(presentation.pinnedWorkerIds));
  const pinnedWorkers = $derived(workers.filter((worker) => pinnedSet.has(worker.id)));
  const unpinnedWorkers = $derived(workers.filter((worker) => !pinnedSet.has(worker.id)));
  const workersByProject = $derived.by(() => {
    const map = new Map<string, Worker[]>();
    for (const project of projects) map.set(project.id, []);
    for (const worker of unpinnedWorkers) map.get(worker.project_id)?.push(worker);
    return map;
  });
  const allCollapsed = $derived(
    projects.length > 0 &&
      projects.every((project) => presentation.collapsedProjectIds.includes(project.id))
  );
  const groupTitle = $derived(presentation.groupBy === 'project' ? 'Projects' : 'Workspaces');

  const itemKeys = $derived.by(() => {
    const keys: string[] = [];
    if (pinnedWorkers.length > 0) {
      keys.push(pinnedItemKey);
      if (!presentation.pinnedCollapsed) {
        for (const worker of pinnedWorkers) keys.push(workerItemKey(worker.id));
      }
    }
    if (presentation.groupBy === 'project') {
      for (const project of projects) {
        keys.push(projectItemKey(project.id));
        if (presentation.collapsedProjectIds.includes(project.id)) continue;
        for (const worker of workersByProject.get(project.id) ?? []) {
          keys.push(workerItemKey(worker.id));
        }
      }
    } else {
      for (const worker of unpinnedWorkers) keys.push(workerItemKey(worker.id));
    }
    return keys;
  });

  const focusableKey = $derived(
    focusedKey && itemKeys.includes(focusedKey)
      ? focusedKey
      : activeWorkerId && itemKeys.includes(workerItemKey(activeWorkerId))
        ? workerItemKey(activeWorkerId)
        : (itemKeys[0] ?? null)
  );

  function tabIndexFor(key: string): number {
    return key === focusableKey ? 0 : -1;
  }

  function toggleProject(projectId: string): void {
    presentation.collapsedProjectIds = toggleListValue(
      presentation.collapsedProjectIds,
      projectId
    );
  }

  function togglePinnedSection(): void {
    presentation.pinnedCollapsed = !presentation.pinnedCollapsed;
  }

  function togglePin(workerId: string): void {
    presentation.pinnedWorkerIds = toggleListValue(presentation.pinnedWorkerIds, workerId);
  }

  function setGroupBy(groupBy: SidebarGroupBy): void {
    presentation.groupBy = groupBy;
  }

  function activate(worker: Worker): void {
    focusedKey = workerItemKey(worker.id);
    onSelect(worker.project_id, worker.id);
  }

  function selectProject(project: Project): void {
    focusedKey = projectItemKey(project.id);
    onSelect(project.id);
  }

  function createForProject(project: Project): void {
    onSelect(project.id);
    onCreate();
  }

  function toggleCollapsedProjectByKey(key: string): void {
    if (key === pinnedItemKey) togglePinnedSection();
    else if (key.startsWith('project:')) toggleProject(key.slice('project:'.length));
  }

  /** Ctrl+Shift+H: expand everything when every project is already collapsed, otherwise collapse. */
  export function toggleCollapsedProjects(): void {
    if (projects.length === 0) return;
    presentation.collapsedProjectIds = allCollapsed ? [] : projects.map((project) => project.id);
  }

  function ensureActiveVisible(): void {
    if (!activeWorkerId) return;
    const worker = workers.find((entry) => entry.id === activeWorkerId);
    if (!worker) return;
    if (presentation.pinnedWorkerIds.includes(worker.id)) {
      if (presentation.pinnedCollapsed) presentation.pinnedCollapsed = false;
      return;
    }
    if (
      presentation.groupBy === 'project' &&
      presentation.collapsedProjectIds.includes(worker.project_id)
    ) {
      presentation.collapsedProjectIds = presentation.collapsedProjectIds.filter(
        (id) => id !== worker.project_id
      );
    }
  }

  function revealActive(): void {
    ensureActiveVisible();
    void tick().then(() => {
      if (!activeWorkerId || !scrollEl) return;
      scrollEl
        .querySelector<HTMLElement>(sidebarItemSelector(workerItemKey(activeWorkerId)))
        ?.scrollIntoView({ block: 'center' });
    });
  }

  /** Ctrl+Shift+0: focus the workspace list, revealing the active workspace first. */
  export function focusList(): void {
    ensureActiveVisible();
    void tick().then(() => {
      const activeKey =
        activeWorkerId && itemKeys.includes(workerItemKey(activeWorkerId))
          ? workerItemKey(activeWorkerId)
          : null;
      const key = activeKey ?? focusableKey;
      const target = key
        ? scrollEl?.querySelector<HTMLElement>(sidebarItemSelector(key)) ?? null
        : null;
      (target ?? scrollEl)?.focus({ preventScroll: true });
      if (target) {
        focusedKey = target.getAttribute('data-sidebar-item');
        target.scrollIntoView({ block: 'nearest' });
      }
    });
  }

  function focusItem(item?: HTMLElement | null): void {
    if (!item) return;
    focusedKey = item.dataset.sidebarItem ?? focusedKey;
    item.focus({ preventScroll: true });
    item.scrollIntoView({ block: 'nearest' });
  }

  function handleFocusIn(event: FocusEvent): void {
    const item = (event.target as HTMLElement | null)?.closest<HTMLElement>('[data-sidebar-item]');
    if (item?.dataset.sidebarItem) focusedKey = item.dataset.sidebarItem;
  }

  function activateItem(item: HTMLElement): void {
    if (item.dataset.kind === 'header') {
      const projectId = item.dataset.projectId;
      if (item.dataset.pinned === 'true') togglePinnedSection();
      else if (projectId) {
        const project = projectsById.get(projectId);
        if (project) selectProject(project);
      }
      return;
    }
    const worker = workers.find((entry) => entry.id === item.dataset.workerId);
    if (worker) activate(worker);
  }

  function handleTreeKeydown(event: KeyboardEvent): void {
    const target = event.target as HTMLElement | null;
    const item = target?.closest<HTMLElement>('[data-sidebar-item]');
    if (!target || !item || !scrollEl) return;
    // Inner controls are native buttons; activation stays theirs, arrows still rove the tree.
    if (target !== item && target.closest('button') && (event.key === 'Enter' || event.key === ' ')) {
      return;
    }
    const items = Array.from(scrollEl.querySelectorAll<HTMLElement>('[data-sidebar-item]'));
    const index = items.indexOf(item);
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        focusItem(items[Math.min(items.length - 1, index + 1)]);
        return;
      case 'ArrowUp':
        event.preventDefault();
        focusItem(items[Math.max(0, index - 1)]);
        return;
      case 'Home':
        event.preventDefault();
        focusItem(items[0]);
        return;
      case 'End':
        event.preventDefault();
        focusItem(items[items.length - 1]);
        return;
      case 'ArrowRight':
        event.preventDefault();
        if (item.dataset.kind === 'header') {
          if (item.getAttribute('aria-expanded') === 'false') {
            toggleCollapsedProjectByKey(item.dataset.sidebarItem ?? '');
          } else if (index + 1 < items.length) {
            focusItem(items[index + 1]);
          }
        }
        return;
      case 'ArrowLeft': {
        event.preventDefault();
        if (item.dataset.kind === 'header' && item.getAttribute('aria-expanded') === 'true') {
          toggleCollapsedProjectByKey(item.dataset.sidebarItem ?? '');
        } else {
          const previous = items
            .slice(0, index)
            .reverse()
            .find((entry) => entry.dataset.kind === 'header');
          if (previous) focusItem(previous);
        }
        return;
      }
      case 'Enter':
      case ' ':
        event.preventDefault();
        activateItem(item);
        return;
      default:
        return;
    }
  }

  function closeMenu(): void {
    menu = null;
  }

  function openAppMenu(event: MouseEvent): void {
    const element = event.currentTarget as HTMLElement;
    const rect = element.getBoundingClientRect();
    menu = {
      anchor: { left: rect.left, top: rect.top, bottom: rect.bottom },
      side: 'bottom',
      width: 224,
      label: 'Application menu',
      restoreTo: element,
      items: [
        {
          id: 'new-workspace',
          label: 'New workspace',
          icon: Plus,
          shortcut: 'Ctrl+N',
          onSelect: onCreate
        },
        {
          id: 'add-project',
          label: 'Add project',
          icon: FolderPlus,
          onSelect: onAddProject
        },
        {
          id: 'toggle-sidebar',
          label: 'Toggle sidebar',
          icon: PanelLeft,
          shortcut: 'Ctrl+B',
          separatorBefore: true,
          onSelect: onToggle
        },
        { id: 'settings', label: 'Settings', icon: Settings, onSelect: onSettings },
        {
          id: 'shortcuts',
          label: 'Keyboard shortcuts',
          icon: Keyboard,
          onSelect: onSettings
        }
      ]
    };
  }

  function openOptionsMenu(event: MouseEvent): void {
    const element = event.currentTarget as HTMLElement;
    const rect = element.getBoundingClientRect();
    menu = {
      anchor: { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom },
      side: 'right',
      width: 288,
      label: 'Workspace options',
      restoreTo: element,
      items: [
        {
          id: 'group-project',
          label: 'Group by project',
          checked: presentation.groupBy === 'project',
          onSelect: () => setGroupBy('project')
        },
        {
          id: 'group-all',
          label: 'Show all workspaces',
          checked: presentation.groupBy === 'flat',
          onSelect: () => setGroupBy('flat')
        },
        {
          id: 'collapse-projects',
          label: allCollapsed ? 'Expand all projects' : 'Collapse all projects',
          icon: allCollapsed ? ChevronsUpDown : ChevronsDownUp,
          shortcut: 'Ctrl+⇧+H',
          disabled: projects.length === 0,
          separatorBefore: true,
          onSelect: toggleCollapsedProjects
        },
        {
          id: 'focus-list',
          label: 'Focus workspace list',
          icon: List,
          shortcut: 'Ctrl+⇧+0',
          separatorBefore: true,
          onSelect: focusList
        }
      ]
    };
  }

  function openHelpMenu(event: MouseEvent): void {
    const element = event.currentTarget as HTMLElement;
    const rect = element.getBoundingClientRect();
    menu = {
      anchor: { left: rect.left, top: rect.top, bottom: rect.bottom },
      side: 'top',
      width: 208,
      label: 'Help',
      restoreTo: element,
      items: [
        {
          id: 'shortcuts',
          label: 'Keyboard Shortcuts',
          icon: Keyboard,
          onSelect: onSettings
        },
        {
          id: 'feedback',
          label: 'Send Feedback',
          icon: MessageSquareText,
          onSelect: () => onFeature('Send feedback')
        },
        {
          id: 'docs',
          label: 'Docs',
          icon: BookOpen,
          separatorBefore: true,
          onSelect: () => onFeature('Docs')
        },
        {
          id: 'changelog',
          label: 'Changelog',
          icon: ScrollText,
          onSelect: () => onFeature('Changelog')
        },
        {
          id: 'github',
          label: 'GitHub',
          icon: Github,
          separatorBefore: true,
          onSelect: () => onFeature('GitHub')
        },
        {
          id: 'updates',
          label: 'Check for Updates',
          icon: RefreshCw,
          onSelect: () => onFeature('Check for updates')
        },
        {
          id: 'restart',
          label: 'Restart Minnow',
          icon: RotateCw,
          separatorBefore: true,
          onSelect: () => onFeature('Restart')
        }
      ]
    };
  }

  function openWorkerMenu(anchor: MenuAnchor, worker: Worker, restoreTo: HTMLElement | null): void {
    const pinned = presentation.pinnedWorkerIds.includes(worker.id);
    const stopped = worker.status === 'exited' || !worker.desired_running;
    menu = {
      anchor,
      side: 'bottom',
      width: 224,
      label: `Workspace actions for ${worker.name}`,
      restoreTo,
      items: [
        {
          id: 'open',
          label: 'Open workspace',
          icon: Play,
          onSelect: () => activate(worker)
        },
        {
          id: 'restart',
          label: 'Restart session',
          icon: RotateCw,
          separatorBefore: true,
          onSelect: () => onWorkerAction('restart', worker)
        },
        {
          id: 'stop',
          label: 'Stop session',
          icon: Square,
          disabled: stopped,
          onSelect: () => onWorkerAction('stop', worker)
        },
        {
          id: 'editor',
          label: 'Open in editor',
          icon: ExternalLink,
          onSelect: () => onWorkerAction('editor', worker)
        },
        {
          id: 'diff',
          label: 'View changes',
          icon: FileDiff,
          onSelect: () => onWorkerAction('diff', worker)
        },
        {
          id: 'merge',
          label: 'Merge into base',
          icon: GitMerge,
          onSelect: () => onWorkerAction('merge', worker)
        },
        {
          id: 'pin',
          label: pinned ? 'Unpin from sidebar' : 'Pin to sidebar',
          icon: Pin,
          separatorBefore: true,
          onSelect: () => togglePin(worker.id)
        },
        {
          id: 'delete',
          label: 'Delete workspace…',
          icon: Trash2,
          danger: true,
          separatorBefore: true,
          onSelect: () => onWorkerAction('delete', worker)
        }
      ]
    };
  }
</script>

<aside
  class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-worktree-sidebar text-worktree-sidebar-foreground"
  aria-label="Workspace sidebar"
>
  <!-- Orca TitlebarLeftControls: custom Windows chrome, sidebar toggle, history. -->
  <div class="titlebar-left w-full">
    <div class="flex h-full items-center">
      <img src={minnowMark} alt="" aria-hidden="true" class="titlebar-logo" />
      <SidebarTip label="Application menu" side="bottom">
        <button
          type="button"
          class="titlebar-icon-button"
          aria-label="Application menu"
          aria-haspopup="menu"
          aria-expanded={menu?.label === 'Application menu'}
          onclick={openAppMenu}
        >
          <Ellipsis size={14} />
        </button>
      </SidebarTip>
      <SidebarTip label="Toggle sidebar (Ctrl+B)" side="bottom">
        <button type="button" class="sidebar-toggle" aria-label="Toggle sidebar" onclick={onToggle}>
          <PanelLeft size={16} />
        </button>
      </SidebarTip>
    </div>
    <div class="ml-auto mr-3 flex items-center pl-2">
      <SidebarTip label="Go back (Ctrl+Alt+←)" side="bottom">
        <button
          type="button"
          class="sidebar-toggle sidebar-toggle-compact"
          aria-label="Go back"
          disabled={!canBack}
          onclick={onBack}
        >
          <ArrowLeft size={12} />
        </button>
      </SidebarTip>
      <SidebarTip label="Go forward (Ctrl+Alt+→)" side="bottom">
        <button
          type="button"
          class="sidebar-toggle sidebar-toggle-compact"
          aria-label="Go forward"
          disabled={!canForward}
          onclick={onForward}
        >
          <ArrowRight size={12} />
        </button>
      </SidebarTip>
    </div>
  </div>

  <!-- Orca SidebarNav -->
  <nav
    class="flex flex-col gap-0.5 px-2 pt-2 pb-1"
    aria-label="Sidebar navigation"
  >
    <button
      type="button"
      class="group flex w-full items-center gap-2 rounded-md bg-worktree-sidebar-foreground/5 px-2 py-1.5 text-left text-[13px] font-medium tracking-tight text-worktree-sidebar-foreground/60 transition-colors hover:bg-worktree-sidebar-foreground/8"
      aria-label="Search worktrees"
      onclick={onSearch}
    >
      <Search class="size-4 shrink-0 text-worktree-sidebar-foreground/30" strokeWidth={1.75} />
      <span class="flex-1">Search</span>
      <span
        class="pointer-events-none hidden shrink-0 items-center gap-1 group-hover:flex group-focus-within:flex"
      >
        <SidebarKeyCombo
          keys={['Ctrl', '⇧', 'J']}
          capClass="min-w-4 border-worktree-sidebar-border/80 bg-worktree-sidebar-foreground/8 px-1 py-px text-[9px] text-worktree-sidebar-foreground/55 shadow-none"
          separatorClass="text-[9px] text-worktree-sidebar-foreground/45"
        />
      </span>
    </button>

    <button
      type="button"
      class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[13px] font-medium tracking-tight text-worktree-sidebar-foreground/60 transition-colors hover:bg-worktree-sidebar-foreground/8"
      class:sb-nav-active={activeView === 'Onboarding checklist'}
      aria-current={activeView === 'Onboarding checklist' ? 'page' : undefined}
      onclick={() => onFeature('Onboarding checklist')}
    >
      <CircleDashed class="size-4 shrink-0 text-worktree-sidebar-foreground/30" strokeWidth={1.75} />
      <span class="min-w-0 flex-1 truncate">Onboarding checklist</span>
    </button>

    <button
      type="button"
      class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[13px] font-medium tracking-tight text-worktree-sidebar-foreground/60 transition-colors hover:bg-worktree-sidebar-foreground/8"
      class:sb-nav-active={activeView === 'Tasks'}
      aria-current={activeView === 'Tasks' ? 'page' : undefined}
      onclick={() => onFeature('Tasks')}
    >
      <List class="size-4 shrink-0 text-worktree-sidebar-foreground/30" strokeWidth={1.75} />
      <span class="flex-1">Tasks</span>
    </button>

    <button
      type="button"
      class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[13px] font-medium tracking-tight text-worktree-sidebar-foreground/60 transition-colors hover:bg-worktree-sidebar-foreground/8"
      class:sb-nav-active={activeView === 'Automations'}
      aria-current={activeView === 'Automations' ? 'page' : undefined}
      onclick={() => onFeature('Automations')}
    >
      <CalendarClock
        class="size-4 shrink-0 text-worktree-sidebar-foreground/30"
        strokeWidth={1.75}
      />
      <span class="flex-1">Automations</span>
    </button>

    <button
      type="button"
      class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[13px] font-medium tracking-tight text-worktree-sidebar-foreground/60 transition-colors hover:bg-worktree-sidebar-foreground/8"
      class:sb-nav-active={activeView === 'Orca Mobile'}
      aria-current={activeView === 'Orca Mobile' ? 'page' : undefined}
      onclick={() => onFeature('Orca Mobile')}
    >
      <Smartphone class="size-4 shrink-0 text-worktree-sidebar-foreground/30" strokeWidth={1.75} />
      <span class="min-w-0 flex-1 truncate">Orca Mobile</span>
      <span class="rounded-full bg-primary px-1.5 py-px text-[10px] font-semibold text-primary-foreground">New</span>
    </button>
  </nav>

  <!-- Orca SidebarHeader -->
  <div
    class="group/sidebar-header mt-2 flex h-8 min-w-0 shrink-0 items-center justify-between gap-1.5 px-2"
  >
    <div class="flex min-w-0 items-center">
      <span
        class="min-w-0 truncate pr-0.5 pl-2 text-xs font-semibold text-muted-foreground/80 select-none"
        >{groupTitle}</span
      >
      {#if presentation.groupBy === 'project' && projects.length > 0}
        <span
          class="flex shrink-0 overflow-hidden text-muted-foreground can-hover:w-0 can-hover:opacity-0 can-hover:group-hover/sidebar-header:w-6 can-hover:group-hover/sidebar-header:opacity-100 can-hover:focus-within:w-6 can-hover:focus-within:opacity-100"
        >
          <button
            type="button"
            class="sb-icon-btn"
            aria-label={allCollapsed ? 'Expand all projects' : 'Collapse all projects'}
            title={`${allCollapsed ? 'Expand all projects' : 'Collapse all projects'} (Ctrl+⇧+H)`}
            onclick={toggleCollapsedProjects}
          >
            {#if allCollapsed}
              <ChevronsUpDown size={14} />
            {:else}
              <ChevronsDownUp size={14} />
            {/if}
          </button>
        </span>
      {/if}
    </div>
    <div class="flex shrink-0 items-center gap-1">
      <SidebarTip label="View activity" side="bottom">
        <button type="button" class="sb-icon-btn relative" aria-label="View activity" onclick={() => onFeature('Activity')}>
          <Bell size={13} class="text-muted-foreground" />
          <span class="absolute -right-0.5 -bottom-0.5 text-[10px] leading-none tabular-nums text-foreground">{attentionCount > 99 ? '99+' : attentionCount}</span>
        </button>
      </SidebarTip>
      <SidebarTip label="Workspace options" side="bottom">
        <button
          type="button"
          class="sb-icon-btn"
          aria-label="Workspace options"
          aria-haspopup="menu"
          aria-expanded={menu?.label === 'Workspace options'}
          onclick={openOptionsMenu}
        >
          <SlidersHorizontal size={14} strokeWidth={2.25} />
        </button>
      </SidebarTip>
      <SidebarTip label="Add project" side="bottom">
        <button
          type="button"
          class="sb-icon-btn"
          aria-label="Add project"
          onclick={onAddProject}
        >
          <FolderPlus size={14} strokeWidth={2.25} />
        </button>
      </SidebarTip>
      <SidebarTip label="New workspace (Ctrl+N)" side="bottom">
        <button type="button" class="sb-icon-btn" aria-label="New workspace" onclick={onCreate}>
          <Plus size={14} strokeWidth={2.25} />
        </button>
      </SidebarTip>
    </div>
  </div>

  <!-- Orca WorktreeList: one list, project/pinned sections plus workspace rows. -->
  <div
    bind:this={scrollEl}
    class="scrollbar-sleek min-h-0 flex-1 overflow-x-hidden overflow-y-auto pt-px pl-1"
    tabindex="-1"
  >
    <div
      role="tree"
      aria-label={presentation.groupBy === 'project' ? 'Projects and workspaces' : 'Workspaces'}
      class="pb-1"
      tabindex="-1"
      onfocusin={handleFocusIn}
      onkeydown={handleTreeKeydown}
    >
      {#if pinnedWorkers.length > 0}
        <SidebarProject
          kind="pinned"
          label="Pinned"
          collapsed={presentation.pinnedCollapsed}
          workerCount={pinnedWorkers.length}
          itemKey={pinnedItemKey}
          tabIndex={tabIndexFor(pinnedItemKey)}
          onToggle={togglePinnedSection}
        />
        {#if !presentation.pinnedCollapsed}
          <div role="group">
            {#each pinnedWorkers as worker (worker.id)}
              <SidebarWorkerRow
                {worker}
                project={projectsById.get(worker.project_id) ?? null}
                active={worker.id === activeWorkerId}
                pinned={true}
                showProject={true}
                tabIndex={tabIndexFor(workerItemKey(worker.id))}
                onActivate={activate}
                onMenu={openWorkerMenu}
              />
            {/each}
          </div>
        {/if}
      {/if}

      {#if presentation.groupBy === 'project'}
        {#each projects as project (project.id)}
          {@const collapsed = presentation.collapsedProjectIds.includes(project.id)}
          {@const groupWorkers = workersByProject.get(project.id) ?? []}
          <SidebarProject
            kind="project"
            {project}
            label={project.name}
            {collapsed}
            workerCount={groupWorkers.length}
            itemKey={projectItemKey(project.id)}
            tabIndex={tabIndexFor(projectItemKey(project.id))}
            selected={project.id === selectedProjectId}
            onCreate={() => createForProject(project)}
            onSelect={() => selectProject(project)}
            onToggle={() => toggleProject(project.id)}
          />
          {#if !collapsed}
            <div role="group">
              {#each groupWorkers as worker (worker.id)}
                <SidebarWorkerRow
                  {worker}
                  project={project}
                  active={worker.id === activeWorkerId}
                  pinned={false}
                  showProject={false}
                  tabIndex={tabIndexFor(workerItemKey(worker.id))}
                  onActivate={activate}
                  onMenu={openWorkerMenu}
                />
              {/each}
            </div>
          {/if}
        {/each}
      {:else}
        <div role="group">
          {#each unpinnedWorkers as worker (worker.id)}
            <SidebarWorkerRow
              {worker}
              project={projectsById.get(worker.project_id) ?? null}
              active={worker.id === activeWorkerId}
              pinned={false}
              showProject={true}
              tabIndex={tabIndexFor(workerItemKey(worker.id))}
              onActivate={activate}
              onMenu={openWorkerMenu}
            />
          {/each}
        </div>
      {/if}

      {#if workers.length === 0}
        <div
          role="treeitem"
          aria-level={1}
          aria-selected="false"
          aria-disabled="true"
          tabindex="-1"
          class="flex flex-col items-center gap-2 px-4 py-6 text-center text-[11px] text-muted-foreground"
        >
          No workspaces found
        </div>
      {/if}
    </div>
  </div>

  <!-- Orca SidebarToolbar -->
  <div class="mt-auto shrink-0">
    <div
      class="flex items-center justify-between border-t border-worktree-sidebar-border px-2 py-1.5"
    >
      <div class="flex min-w-0 items-center gap-1">
        <SidebarTip label="Settings" side="top">
          <button
            type="button"
            class="sb-icon-btn"
            aria-label="Settings"
            onclick={onSettings}
          >
            <Settings size={14} />
          </button>
        </SidebarTip>
        <SidebarTip label="Help" side="top">
          <button
            type="button"
            class="sb-icon-btn"
            aria-label="Help"
            aria-haspopup="menu"
            aria-expanded={menu?.label === 'Help'}
            onclick={openHelpMenu}
          >
            <CircleQuestionMark size={14} />
          </button>
        </SidebarTip>
      </div>
      <div class="flex items-center gap-1">
        <SidebarTip label="Reveal active workspace" side="top">
          <button
            type="button"
            class="sb-icon-btn"
            aria-label="Reveal active workspace"
            disabled={!activeWorkerId}
            onclick={revealActive}
          >
            <Crosshair size={14} />
          </button>
        </SidebarTip>
        <SidebarTip label="Workspace board" side="top">
          <button
            type="button"
            class="sb-icon-btn"
            aria-label="Workspace board"
            onclick={() => onFeature('Workspace board')}
          >
            <Kanban size={14} />
          </button>
        </SidebarTip>
      </div>
    </div>
  </div>
</aside>

{#if menu}
  <SidebarMenu
    anchor={menu.anchor}
    side={menu.side}
    width={menu.width}
    label={menu.label}
    items={menu.items}
    restoreTo={menu.restoreTo}
    onClose={closeMenu}
  />
{/if}

<style>
  .sb-nav-active {
    background: var(--worktree-sidebar-accent);
    color: var(--worktree-sidebar-foreground);
  }
  .sb-nav-active :global(svg) { color: inherit; }
  /* Orca assets/main.css:1121-1310 titlebar chrome, scoped to this sidebar header. */
  /* Keep the collapsed titlebar controls outside the sidebar's scroll viewport. */
  .titlebar-left {
    overflow: visible;
  }

  .titlebar-logo {
    height: 16px;
    width: 16px;
    flex-shrink: 0;
    margin: 0 4px 0 10px;
    object-fit: contain;
  }

  .titlebar-icon-button {
    display: flex;
    align-items: center;
    justify-content: center;
    margin-right: 8px;
    padding: 4px;
    border: none;
    border-radius: 4px;
    background: none;
    color: var(--muted-foreground);
    cursor: pointer;
    transition: background 150ms;
  }
  .titlebar-icon-button:hover {
    background: var(--accent);
    color: var(--foreground);
  }

  .sidebar-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px 8px;
    border: none;
    border-radius: 4px;
    background: none;
    color: var(--muted-foreground);
    cursor: pointer;
    transition: background 150ms;
  }
  .sidebar-toggle:hover {
    background: var(--accent);
    color: var(--foreground);
  }
  .sidebar-toggle-compact {
    padding: 4px 5px;
    color: var(--foreground);
  }
  .sidebar-toggle-compact:disabled {
    color: var(--muted-foreground);
  }
  .sidebar-toggle:disabled {
    cursor: default;
    opacity: 0.45;
  }
  .sidebar-toggle:disabled:hover {
    background: none;
    color: var(--muted-foreground);
  }

  /* Orca Button variant=ghost size=icon-xs (components/ui/button.tsx). */
  .sb-icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted-foreground);
    transition:
      background-color 150ms,
      color 150ms;
  }
  .sb-icon-btn:hover {
    background: var(--accent);
    color: var(--accent-foreground);
  }
  .sb-icon-btn:disabled {
    pointer-events: none;
    opacity: 0.45;
  }
</style>
