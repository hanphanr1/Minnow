<script lang="ts">
  import { tick } from 'svelte';
  import {
    Check,
    ChevronDown,
    CircleAlert,
    ExternalLink,
    File,
    FileCode2,
    FileImage,
    FileJson,
    FileText,
    Files,
    GitBranch,
    ChevronRight,
    ListChecks,
    Loader2,
    PanelRight,
    RefreshCw,
    Search,
    Plus,
    X
  } from 'lucide-svelte';
  import type { Project, Worker, WorkerStatus } from '../types';
  import {
    diffStatusColor,
    diffStatusLetter,
    parseUnifiedDiff,
    splitDiffPath
  } from './diff';

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
    { id: 'checks', title: 'Checks', icon: ListChecks, shortcut: '' }
  ];

  const STATUS_DOT_CLASS: Record<WorkerStatus, string> = {
    running: 'bg-emerald-500',
    idle: 'bg-muted-foreground/60',
    'waiting-for-input': 'bg-amber-500',
    exited: 'bg-rose-500'
  };

  let {
    project,
    worker,
    workers,
    panel,
    diff,
    loading,
    error,
    onPanel,
    onRefresh,
    onOpenDiff,
    onAction,
    onSelect,
    onClose
  }: {
    project: Project | null;
    worker: Worker | null;
    workers: Worker[];
    panel: string;
    diff: string | null;
    loading: boolean;
    error: string | null;
    onPanel: (panel: string) => void;
    onRefresh: () => void;
    onOpenDiff: () => void;
    onAction: (action: 'merge' | 'delete', worker: Worker) => void;
    onSelect: (id: string) => void;
    onFeature: (name: string) => void;
    onClose: () => void;
  } = $props();

  let filterOpen = $state(false);
  let filterQuery = $state('');
  let filterInput = $state<HTMLInputElement | null>(null);
  let changesCollapsed = $state(false);

  const activePanel = $derived(panel === 'search' ? 'explorer' : panel);
  const parsed = $derived(parseUnifiedDiff(diff ?? ''));
  const normalizedFilter = $derived(filterQuery.trim().toLowerCase());
  const filteredFiles = $derived(
    normalizedFilter
      ? parsed.files.filter(
          (file) =>
            file.path.toLowerCase().includes(normalizedFilter) ||
            (file.oldPath?.toLowerCase().includes(normalizedFilter) ?? false)
        )
      : parsed.files
  );


  function selectPanel(id: string): void {
    if (id !== 'source-control') filterOpen = false;
    onPanel(id);
  }

  async function openFilter(): Promise<void> {
    filterOpen = true;
    await tick();
    filterInput?.focus();
    filterInput?.select();
  }

  function closeFilter(clear: boolean): void {
    if (clear) filterQuery = '';
    filterOpen = false;
  }

  function handleFilterKey(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    closeFilter(false);
  }

  function iconFor(path: string): IconComponent {
    const name = path.toLowerCase();
    if (/\.(png|jpe?g|gif|webp|ico|bmp|avif|svg)$/.test(name)) return FileImage;
    if (/\.(json|jsonc|json5)$/.test(name)) return FileJson;
    if (
      /\.(ts|tsx|js|jsx|mjs|cjs|rs|css|scss|html|svelte|vue|py|go|java|kt|c|h|cpp|hpp|cs|rb|php|sh|ps1|toml|ya?ml)$/.test(
        name
      )
    ) {
      return FileCode2;
    }
    if (/\.(md|mdx|txt|rst)$/.test(name)) return FileText;
    return File;
  }

  function workerStatusLabel(status: WorkerStatus): string {
    return status === 'waiting-for-input' ? 'waiting for input' : status;
  }
</script>

<div class="flex h-full min-h-0 w-full flex-col overflow-hidden bg-sidebar">
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
      {#if !project || !worker}
        <div class="flex flex-1 items-center justify-center px-4 text-center text-xs text-muted-foreground">
          Select a workspace to view changes
        </div>
      {:else}
        <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
          <div class="border-b border-border px-3 pt-1.5 pb-1">
            <div
              class="flex min-w-0 items-center gap-1"
              data-filter-expanded={filterOpen ? 'true' : 'false'}
            >
              {#if filterOpen}
                <div class="flex min-w-0 w-full flex-1 items-center gap-1.5">
                  <Search size={14} class="shrink-0 text-muted-foreground" aria-hidden="true" />
                  <input
                    bind:this={filterInput}
                    bind:value={filterQuery}
                    onkeydown={handleFilterKey}
                    type="text"
                    class="min-w-0 w-full flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
                    placeholder="Filter files…"
                    aria-label="Filter files…"
                  />
                </div>
                <button
                  type="button"
                  class="inline-flex size-7 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                  onclick={() => closeFilter(true)}
                  aria-label="Clear and close filter"
                  title="Clear and close filter"
                >
                  <X size={14} />
                </button>
              {:else}
                <button type="button" disabled class="mr-auto inline-flex h-6 items-center gap-1 rounded-md bg-muted px-1.5 text-[11px] text-muted-foreground" title="Pull requests are unavailable in this build"><GitBranch size={12}/>Create PR</button>
                <button
                  type="button"
                  class={`relative inline-flex size-7 shrink-0 items-center justify-center rounded-sm transition-colors hover:bg-accent hover:text-foreground ${
                    normalizedFilter ? 'bg-muted text-foreground' : 'text-muted-foreground'
                  }`}
                  onclick={() => void openFilter()}
                  aria-label={normalizedFilter ? `Filter: ${filterQuery}` : 'Filter files by name'}
                  title={normalizedFilter ? `Filter: ${filterQuery}` : 'Filter files by name'}
                >
                  <Search size={14} />
                  {#if normalizedFilter}
                    <span class="absolute right-1 top-1 size-1.5 rounded-full bg-foreground"></span>
                  {/if}
                </button>
                <button
                  type="button"
                  class="inline-flex size-7 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
                  onclick={onRefresh}
                  disabled={loading}
                  aria-label="Refresh changes"
                  title="Refresh changes"
                >
                  <RefreshCw size={14} class={loading ? 'animate-spin' : ''} />
                </button>
              {/if}
            </div>

            <div class="mt-1 flex min-w-0 items-center gap-2 text-[11px] text-muted-foreground">
              <GitBranch size={12} class="shrink-0" aria-hidden="true" />
              <span class="min-w-0 truncate" title={`${worker.branch} → ${project.base_ref}`}>
                {worker.branch}
                <span aria-hidden="true">→</span>
                {project.base_ref}
              </span>
              {#if diff !== null && !loading && !error}
                <span class="ml-auto flex shrink-0 items-center gap-1 tabular-nums">
                  <span>{parsed.files.length} {parsed.files.length === 1 ? 'file' : 'files'}</span>
                  <span style:color="var(--git-decoration-added)">+{parsed.added}</span>
                  <span style:color="var(--git-decoration-deleted)">-{parsed.removed}</span>
                </span>
              {/if}
            </div>
          </div>
          <div class="space-y-2 px-3 pb-4 pt-1.5">
            <textarea disabled rows="2" aria-label="Commit message (unavailable)" placeholder="Message" title="Committing and staging are unavailable in this build" class="h-14 w-full resize-none rounded-md border border-input bg-input/30 px-2 py-1.5 text-xs text-foreground placeholder:text-muted-foreground"></textarea>
            <button type="button" disabled title="Staging is unavailable in this build" class="flex h-6 w-full items-center justify-center gap-1 rounded-md border border-input text-xs text-muted-foreground"><Plus size={12}/>Stage All</button>
          </div>

          <div class="min-h-0 flex-1 overflow-auto scrollbar-sleek">
            {#if loading}
              <div class="flex h-full items-center justify-center gap-2 px-4 text-center text-xs text-muted-foreground" role="status">
                <Loader2 size={14} class="animate-spin" aria-hidden="true" />
                Loading changes…
              </div>
            {:else if error}
              <div class="m-3 rounded-md border border-destructive/30 bg-destructive/5 p-3" role="alert">
                <div class="flex items-start gap-2 text-xs text-destructive">
                  <CircleAlert size={14} class="mt-0.5 shrink-0" aria-hidden="true" />
                  <p class="min-w-0 flex-1 break-words">{error}</p>
                </div>
                <button
                  type="button"
                  class="mt-2 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
                  onclick={onRefresh}
                >
                  Retry
                </button>
              </div>
            {:else if diff === null}
              <div class="px-4 py-6">
                <div class="text-sm font-medium text-foreground">Changes not loaded</div>
                <div class="mt-1 text-xs text-muted-foreground">
                  Load this worktree's patch against its base commit.
                </div>
                <button
                  type="button"
                  class="mt-3 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
                  onclick={onRefresh}
                >
                  Load changes
                </button>
              </div>
            {:else if parsed.files.length === 0}
              <div class="px-4 py-6">
                <div class="text-sm font-medium text-foreground">No changes</div>
                <div class="mt-1 text-xs text-muted-foreground">
                  This worktree has no changes against {project.base_ref}.
                </div>
              </div>
            {:else if filteredFiles.length === 0}
              <div class="px-4 py-6">
                <div class="text-sm font-medium text-foreground">No matching files</div>
                <div class="mt-1 text-xs text-muted-foreground">
                  No changed file matches “{filterQuery.trim()}”.
                </div>
                <button
                  type="button"
                  class="mt-3 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
                  onclick={() => closeFilter(true)}
                >
                  Clear filter
                </button>
              </div>
            {:else}
              <div class="pl-1 pr-3 pt-3 pb-1">
                <div class="group/section flex items-center gap-x-1 rounded-md pr-1 hover:bg-accent hover:text-accent-foreground">
                  <button
                    type="button"
                    class="flex h-auto min-h-6 min-w-0 flex-1 items-center justify-start gap-x-1 py-0.5 text-left text-[11px] font-semibold uppercase tracking-wider text-foreground/70 group-hover/section:text-accent-foreground"
                    onclick={() => (changesCollapsed = !changesCollapsed)}
                    aria-expanded={!changesCollapsed}
                  >
                    <ChevronDown
                      size={14}
                      class={`shrink-0 transition-transform ${changesCollapsed ? '-rotate-90' : ''}`}
                    />
                    <span class="flex min-w-0 flex-1 items-center gap-1">
                      <span class="min-w-0 truncate">Changes</span>
                      <span class="shrink-0 text-[11px] font-medium tabular-nums">
                        {filteredFiles.length}
                      </span>
                    </span>
                  </button>
                  <div class="ml-auto flex shrink-0 items-center justify-end">
                    <button
                      type="button"
                      class="h-6 px-1.5 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
                      onclick={onOpenDiff}
                      title="Open all changes in the diff view"
                    >
                      View all
                    </button>
                  </div>
                </div>
              </div>

              {#if !changesCollapsed}
                {#each filteredFiles as file, index (file.path + ':' + index)}
                  {@const displayPath = splitDiffPath(file.path)}
                  {@const FileIcon = iconFor(file.path)}
                  <button
                    type="button"
                    class="group relative flex w-full cursor-pointer items-center gap-1 py-1 pr-3 text-left transition-colors hover:bg-accent/40"
                    style:padding-left="20px"
                    onclick={onOpenDiff}
                    title={`Open diff: ${file.path}`}
                  >
                    <FileIcon
                      size={14}
                      class="shrink-0"
                      color={diffStatusColor(file.status)}
                    />
                    <span class="min-w-0 flex-1 text-xs">
                      <span class="block min-w-0 truncate">
                        <span class="text-foreground">{displayPath.fileName}</span>
                        {#if displayPath.prefix}
                          <span class="ml-1.5 text-[11px] text-muted-foreground">
                            {displayPath.prefix.slice(0, -1)}
                          </span>
                        {/if}
                      </span>
                      {#if file.oldPath}
                        <span class="block truncate text-[11px] text-muted-foreground">
                          from {file.oldPath}
                        </span>
                      {/if}
                    </span>
                    <span class="shrink-0 text-[10px] tabular-nums">
                      {#if file.added > 0}
                        <span style:color="var(--git-decoration-added)">+{file.added}</span>
                      {/if}
                      {#if file.added > 0 && file.removed > 0}<span> </span>{/if}
                      {#if file.removed > 0}
                        <span style:color="var(--git-decoration-deleted)">-{file.removed}</span>
                      {/if}
                    </span>
                    <span
                      class="w-4 shrink-0 text-center text-[10px] font-bold"
                      style:color={diffStatusColor(file.status)}
                      title={file.status}
                    >
                      {diffStatusLetter(file.status)}
                    </span>
                    <span class="absolute right-3 top-0 bottom-0 flex items-center bg-accent pl-2 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
                      <ExternalLink size={12} class="text-muted-foreground" aria-hidden="true" />
                    </span>
                  </button>
                {/each}
              {/if}
            {/if}
          </div>

          <details class="shrink-0 border-t border-border text-[11px] text-muted-foreground">
            <summary class="flex h-7 cursor-pointer list-none items-center gap-1 px-2 font-semibold uppercase"><ChevronRight size={12}/>Commits</summary>
            <p class="px-3 pb-3">Commit history is unavailable in this build. Review the actual worktree patch above.</p>
          </details>
        </div>
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
    {:else if panel === 'search'}
      <div class="space-y-3 px-3 py-3 text-xs">
        <h2 class="text-sm font-medium">Search</h2>
        <input disabled aria-label="Search files (unavailable)" placeholder="Search files" class="mn-input"/>
        <p class="text-muted-foreground">File search is unavailable in this build. Use the workspace palette to find projects and workers.</p>
      </div>
    {:else if !worker}
      <div class="flex min-h-0 flex-1 items-center justify-center px-4 text-center text-xs text-muted-foreground">
        Select a workspace to browse files
      </div>
    {:else}
      <div class="min-h-0 flex-1 overflow-auto scrollbar-sleek">
        <div class="px-3 pt-3 pb-1">
          <div class="text-sm font-medium text-foreground">Explorer</div>
          <p class="mt-1 text-xs text-muted-foreground">
            This build does not expose a file tree. Minnow's backend provides terminal sessions and
            workspace diffs only.
          </p>
        </div>
        {#if worker}
          <div class="mx-3 mt-2 space-y-1 rounded-md border border-border p-2 font-mono text-[11px] text-muted-foreground">
            <div class="truncate" title={worker.path}>{worker.path}</div>
            <div class="truncate">{worker.branch}</div>
          </div>
        {/if}
        <button
          type="button"
          class="mx-3 mt-3 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
          onclick={() => selectPanel('source-control')}
        >
          Open Source Control
        </button>
      </div>
    {/if}
  </div>
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
