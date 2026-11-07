<script lang="ts">
  import {
    Check,
    CircleAlert,
    Files,
    GitBranch,
    ListChecks,
    Network,
    PanelRight,
    X
  } from 'lucide-svelte';
  import type { Worker, WorkerStatus } from '../types';
  import { announceWorkspaceChange } from '../workspace-api';
  import ExplorerPane from './ExplorerPane.svelte';
  import SourceControlPane from './SourceControlPane.svelte';
  import PortsPane from './PortsPane.svelte';

  type IconComponent = typeof Files;

  type ActivityItem = {
    id: string;
    title: string;
    icon: IconComponent | null;
    shortcut: string;
  };

  const ACTIVITY: ActivityItem[] = [
    { id: 'explorer', title: 'Explorer', icon: Files, shortcut: 'Ctrl+Shift+E' },
    { id: 'agents', title: 'Agents', icon: null, shortcut: '' },
    { id: 'source-control', title: 'Source Control', icon: GitBranch, shortcut: 'Ctrl+Shift+G' },
    { id: 'checks', title: 'Checks', icon: ListChecks, shortcut: '' },
    { id: 'ports', title: 'Ports', icon: Network, shortcut: '' }
  ];

  const STATUS_DOT_CLASS: Record<WorkerStatus, string> = {
    running: 'bg-emerald-500',
    idle: 'bg-muted-foreground/60',
    'waiting-for-input': 'bg-amber-500',
    exited: 'bg-rose-500'
  };

  let {
    worker,
    workers,
    panel,
    onPanel,
    onRefresh,
    onOpenDiff,
    onOpenFile,
    onOpenEditor,
    onSelect,
    onClose
  }: {
    worker: Worker | null;
    workers: Worker[];
    panel: string;
    onPanel: (panel: string) => void;
    onRefresh: () => void;
    onOpenDiff: (path?: string, staged?: boolean) => void;
    onOpenFile: (path: string, line?: number) => void;
    onOpenEditor?: () => void;
    onSelect: (id: string) => void;
    onClose: () => void;
  } = $props();

  let sidebarNotice = $state<{ id: number; message: string; error: boolean } | null>(null);
  let noticeId = 0;
  let noticeTimer: ReturnType<typeof setTimeout> | null = null;

  const activePanel = $derived(panel === 'search' ? 'explorer' : panel);

  function selectPanel(id: string): void {
    onPanel(id);
  }

  function notifyLocal(message: string, error = false): void {
    const id = ++noticeId;
    sidebarNotice = { id, message, error };
    if (noticeTimer) clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => {
      if (sidebarNotice?.id === id) sidebarNotice = null;
      noticeTimer = null;
    }, 4500);
  }

  function handleWorkspaceChanged(): void {
    const current = worker;
    if (!current) return;
    onRefresh();
    announceWorkspaceChange(current.id);
  }

  $effect(() => {
    return () => {
      if (noticeTimer) clearTimeout(noticeTimer);
    };
  });

  function workerStatusLabel(status: WorkerStatus): string {
    return status === 'waiting-for-input' ? 'waiting for input' : status;
  }
</script>

<div class="relative flex h-full min-h-0 w-full flex-col overflow-hidden bg-sidebar">
  <!-- 36px titlebar row; the 138px window-control overlay stays clear. -->
  <div
    class="flex h-9 min-h-9 items-center border-b border-border"
    style:padding-right="var(--window-controls-width, 0px)"
    data-tauri-drag-region
  >
    <div class="ml-auto flex shrink-0 items-center pr-1">
      <button
        type="button"
        class="sidebar-toggle mr-1"
        onclick={onClose}
        aria-label="Toggle right sidebar"
        title="Toggle right sidebar (Ctrl+L)"
      >
        <PanelRight size={16} />
      </button>
    </div>
  </div>

  <!-- 40px activity strip below the titlebar row, matching Orca's Windows chrome. -->
  <div class="flex h-10 min-h-10 items-center border-b border-border px-2">
    {#each ACTIVITY as item (item.id)}
      {@const active = activePanel === item.id}
      {@const label = item.shortcut ? `${item.title} (${item.shortcut})` : item.title}
      <button
        type="button"
        class={`relative flex h-9 w-9 shrink-0 items-center justify-center transition-colors ${
          active ? 'text-foreground' : 'text-muted-foreground/60 hover:text-muted-foreground'
        }`}
        onclick={() => selectPanel(item.id)}
        aria-label={label}
        aria-pressed={active}
        title={label}
      >
        {#if item.icon}
          {@const Icon = item.icon}
          <Icon size={16} />
        {:else}
          <!-- Orca AgentSessionHistoryIcon, MIT; kept as an SVG so the strip matches the reference. -->
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path stroke="none" d="M0 0h24v24H0z" fill="none" />
            <path d="M14 4h6v6h-6z" />
            <path d="M4 14h6v6h-6z" />
            <path d="M17 17m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0" />
            <path d="M7 7m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0" />
          </svg>
        {/if}
        {#if active}
          <span class="absolute bottom-0 left-1/4 right-1/4 h-0.5 rounded-t bg-foreground"></span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    {#if activePanel === 'source-control'}
      {#if !worker}
        <div class="flex flex-1 items-center justify-center px-4 text-center text-xs text-muted-foreground">
          Select a workspace to view changes
        </div>
      {:else}
        <SourceControlPane
          {worker}
          {onOpenDiff}
          onOpenFile={(path) => onOpenFile(path)}
          onChanged={handleWorkspaceChanged}
          onNotify={notifyLocal}
        />
      {/if}
    {:else if activePanel === 'agents'}
      <div class="min-h-0 flex-1 overflow-auto scrollbar-sleek">
        <div class="px-3 pt-3 pb-1">
          <div class="text-sm font-medium text-foreground">Agent sessions</div>
          <p class="mt-1 text-xs text-muted-foreground">
            Session history is not available in this build. Project worktrees are listed below.
          </p>
        </div>
        {#if workers.length > 0}
          <div class="pl-1 pr-3 pt-2 pb-1">
            <div class="flex min-h-6 items-center gap-1 rounded-md pr-1">
              <span class="flex min-h-6 items-center gap-1 px-2 py-0.5 text-left text-[11px] font-semibold uppercase tracking-wider text-foreground/70">
                Worktrees
                <span class="font-medium tabular-nums">{workers.length}</span>
              </span>
            </div>
          </div>
          {#each workers as item (item.id)}
            <button
              type="button"
              class={`group relative flex w-full cursor-pointer items-center gap-2 py-1.5 pr-3 text-left transition-colors hover:bg-accent/40 ${
                item.id === worker?.id ? 'bg-accent/60' : ''
              }`}
              style:padding-left="20px"
              onclick={() => onSelect(item.id)}
              title={`Select ${item.name}`}
            >
              <span
                class={`size-2 shrink-0 rounded-full ${STATUS_DOT_CLASS[item.status]}`}
                aria-hidden="true"
              ></span>
              <span class="min-w-0 flex-1">
                <span class="block truncate text-xs text-foreground">{item.name}</span>
                <span class="block truncate text-[11px] text-muted-foreground">
                  {item.branch} · {workerStatusLabel(item.status)}
                </span>
              </span>
              {#if item.id === worker?.id}
                <Check size={12} class="shrink-0 text-foreground" aria-hidden="true" />
              {/if}
            </button>
          {/each}
        {:else}
          <div class="px-4 py-6">
            <div class="text-sm font-medium text-foreground">No worktrees</div>
            <div class="mt-1 text-xs text-muted-foreground">
              Create a workspace to start a session.
            </div>
          </div>
        {/if}
      </div>
    {:else if activePanel === 'ports'}
      {#if worker}
        <PortsPane {worker} />
      {:else}
        <div class="flex flex-1 items-center justify-center px-4 text-center text-xs text-muted-foreground">Select a workspace to see its ports</div>
      {/if}
    {:else if activePanel === 'checks'}
      <div class="flex min-h-0 flex-1 flex-col items-center justify-center px-4 text-center">
        <ListChecks size={18} class="text-muted-foreground/60" aria-hidden="true" />
        <div class="mt-2 text-sm font-medium text-foreground">Checks are unavailable in this build</div>
        <p class="mt-1 text-xs text-muted-foreground">
          Minnow does not connect to a CI provider. Workspace diffs stay under Source Control.
        </p>
        <button
          type="button"
          class="mt-3 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
          onclick={() => selectPanel('source-control')}
        >
          Open Source Control
        </button>
      </div>
    {:else if !worker}
      <div class="flex min-h-0 flex-1 items-center justify-center px-4 text-center text-xs text-muted-foreground">
        {panel === 'search' ? 'Select a workspace to search' : 'Select a workspace to browse files'}
      </div>
    {:else}
      <ExplorerPane {worker} view={panel === 'search' ? 'search' : 'files'} onView={(value) => onPanel(value === 'search' ? 'search' : 'explorer')} {onOpenFile} {onOpenEditor} />
    {/if}
  </div>

  {#if sidebarNotice}
    <div class="pointer-events-none absolute bottom-2 left-2 right-2 z-20">
      <div
        class={`pointer-events-auto flex items-start gap-1.5 rounded-md border bg-dialog-surface p-2 text-[11px] shadow-md ${
          sidebarNotice.error
            ? 'border-destructive/40 text-destructive'
            : 'border-border text-foreground'
        }`}
        role={sidebarNotice.error ? 'alert' : 'status'}
      >
        {#if sidebarNotice.error}
          <CircleAlert size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
        {:else}
          <Check size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
        {/if}
        <p class="min-w-0 flex-1 break-words">{sidebarNotice.message}</p>
        <button
          type="button"
          class="shrink-0 text-muted-foreground transition-colors hover:text-foreground"
          aria-label="Dismiss notification"
          onclick={() => (sidebarNotice = null)}
        >
          <X size={12} />
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .sidebar-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 4px;
    padding: 4px 8px;
    background: none;
    color: var(--muted-foreground);
    cursor: pointer;
    transition: background 150ms;
  }

  .sidebar-toggle:hover {
    background: var(--accent);
    color: var(--foreground);
  }
</style>
