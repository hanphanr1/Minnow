<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { CircleHelp, FolderTree, GitBranch, ListFilter, Plus, Search } from 'lucide-svelte';
  import type { Project, Worker } from '../types';

  interface Props {
    projects: Project[];
    workers: Worker[];
    onSelect: (projectId: string, workerId?: string) => void;
    onCreate: () => void;
    onClose: () => void;
  }

  interface RecentEntry {
    projectId: string;
    workerId: string | null;
    at: number;
  }

  type Row =
    | { key: string; kind: 'worker'; project: Project; worker: Worker; recentAt?: number }
    | { key: string; kind: 'project'; project: Project; recentAt?: number }
    | { key: string; kind: 'create' };

  interface Section {
    key: string;
    label: string;
    rows: Row[];
  }

  const RECENT_KEY = 'minnow.workspace-palette.recent.v1';
  const MAX_RECENT = 6;

  let { projects, workers, onSelect, onCreate, onClose }: Props = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);
  let query = $state('');
  let recents = $state<RecentEntry[]>([]);
  let activeId = $state('');
  let now = $state(Date.now());
  let category = $state('all');

  let previousFocus: HTMLElement | null = null;
  let unmounted = false;
  let closeNotified = false;
  let previousQuery = '';

  const createRow: Row = { key: 'create', kind: 'create' };

  let projectsById = $derived(
    new Map<string, Project>(
      projects.map((project): [string, Project] => [project.id, project])
    )
  );
  let workersById = $derived(
    new Map<string, Worker>(workers.map((worker): [string, Worker] => [worker.id, worker]))
  );
  let normalizedQuery = $derived(query.trim().toLowerCase());

  function matchesQuery(...parts: Array<string | null | undefined>): boolean {
    const tokens = normalizedQuery.split(/\s+/).filter(Boolean);
    if (tokens.length === 0) return true;
    const haystack = parts.filter((part) => part !== null && part !== undefined).join(' ').toLowerCase();
    return tokens.every((token) => haystack.includes(token));
  }

  let recentRows = $derived.by((): Row[] => {
    if (normalizedQuery.length > 0) return [];
    const rows: Row[] = [];
    const seen = new Set<string>();
    for (const entry of recents) {
      const project = projectsById.get(entry.projectId);
      if (project === undefined) continue;
      if (entry.workerId !== null) {
        const worker = workersById.get(entry.workerId);
        if (worker === undefined || worker.project_id !== project.id) continue;
        const key = `w:${worker.id}`;
        if (seen.has(key)) continue;
        seen.add(key);
        rows.push({ key, kind: 'worker', project, worker, recentAt: entry.at });
        continue;
      }
      const key = `p:${project.id}`;
      if (seen.has(key)) continue;
      seen.add(key);
      rows.push({ key, kind: 'project', project, recentAt: entry.at });
    }
    return rows;
  });

  let projectRows = $derived.by((): Row[] =>
    projects
      .filter((project) => matchesQuery(project.name, project.path, project.base_ref))
      .map((project): Row => ({ key: `p:${project.id}`, kind: 'project', project }))
  );

  let workerRows = $derived.by((): Row[] => {
    const rows: Row[] = [];
    for (const worker of workers) {
      const project = projectsById.get(worker.project_id);
      if (project === undefined) continue;
      if (
        !matchesQuery(
          worker.name,
          worker.branch,
          worker.path,
          worker.status,
          project.name,
          project.path
        )
      ) {
        continue;
      }
      rows.push({ key: `w:${worker.id}`, kind: 'worker', project, worker });
    }
    return rows;
  });

  let sections = $derived.by((): Section[] => {
    const result: Section[] = [];
    const recents = recentRows.filter(row => category === 'all' || row.kind === category);
    const seen = new Set(recents.map(row => row.key));
    const matchingProjects = category === 'worker' || (category === 'all' && !normalizedQuery && workers.length > 0) ? [] : projectRows.filter(row => !seen.has(row.key));
    const matchingWorkers = category === 'project' ? [] : workerRows.filter(row => !seen.has(row.key));
    if (recents.length > 0) result.push({key:'recent',label:'Recent workspaces',rows:recents});
    if (matchingProjects.length > 0) result.push({key:'projects',label:'Projects',rows:matchingProjects});
    if (matchingWorkers.length > 0) result.push({key:'workspaces',label:'Workspaces',rows:matchingWorkers});
    return result;
  });
  const showCreate = $derived(category === 'all' && ((!normalizedQuery && workers.length === 0) || /^(create|new)(\s|$)/.test(normalizedQuery)));

  let flatRows = $derived.by((): Row[] => [
    ...sections.flatMap((section) => section.rows),
    ...(showCreate ? [createRow] : [])
  ]);
  let activeRow = $derived(flatRows.find((row) => row.key === activeId) ?? null);
  let activeDescendant = $derived(activeRow !== null ? rowElementId(activeRow.key) : undefined);
  let resultCount = $derived(sections.reduce((total, section) => total + section.rows.length, 0));

  function rowElementId(key: string): string {
    return `palette-option-${encodeURIComponent(key)}`;
  }

  function isRecent(value: unknown): value is RecentEntry {
    if (typeof value !== 'object' || value === null) return false;
    const record = value as Record<string, unknown>;
    return (
      typeof record.projectId === 'string' &&
      (record.workerId === null ||
        record.workerId === undefined ||
        typeof record.workerId === 'string') &&
      typeof record.at === 'number' &&
      Number.isFinite(record.at)
    );
  }

  function readRecents(): RecentEntry[] {
    try {
      const raw = localStorage.getItem(RECENT_KEY);
      if (raw === null) return [];
      const parsed: unknown = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .filter(isRecent)
        .map((entry): RecentEntry => ({
          projectId: entry.projectId,
          workerId: typeof entry.workerId === 'string' ? entry.workerId : null,
          at: entry.at
        }))
        .slice(0, MAX_RECENT);
    } catch {
      return [];
    }
  }

  function remember(projectId: string, workerId: string | null): void {
    const next = [
      { projectId, workerId, at: Date.now() },
      ...recents.filter(
        (entry) => !(entry.projectId === projectId && entry.workerId === workerId)
      )
    ].slice(0, MAX_RECENT);
    recents = next;
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(next));
    } catch {
      // Recency is optional when storage is unavailable.
    }
  }

  function formatAge(at: number): string {
    const delta = Math.max(0, now - at);
    if (delta < 60_000) return '<1m';
    const minutes = Math.floor(delta / 60_000);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 48) return `${hours}h`;
    return `${Math.floor(hours / 24)}d`;
  }

  function rowAge(row: Row): string | null {
    if (row.kind === 'create' || row.recentAt === undefined) return null;
    return formatAge(row.recentAt);
  }

  function statusLabel(worker: Worker): string {
    if (worker.status === 'running') return 'Running';
    if (worker.status === 'idle') return 'Idle';
    if (worker.status === 'waiting-for-input') return 'Waiting for input';
    return worker.last_error === null ? 'Exited' : `Exited: ${worker.last_error}`;
  }

  function exitedWithError(worker: Worker): boolean {
    return (
      worker.status === 'exited' &&
      (worker.last_error !== null || (worker.exit_code !== null && worker.exit_code !== 0))
    );
  }

  function restoreFocusSoon(): void {
    const target = previousFocus;
    previousFocus = null;
    if (target === null) return;
    requestAnimationFrame(() => {
      if (target.isConnected && document.activeElement === document.body) {
        target.focus({ preventScroll: true });
      }
    });
  }

  function handleNativeClose(): void {
    if (unmounted || closeNotified) return;
    closeNotified = true;
    onClose();
    restoreFocusSoon();
  }

  function requestClose(): void {
    if (dialogEl?.open) dialogEl.close();
    else handleNativeClose();
  }

  function handleBackdrop(event: MouseEvent): void {
    if (dialogEl === null || event.target !== dialogEl) return;
    const rect = dialogEl.getBoundingClientRect();
    const outside =
      event.clientX < rect.left ||
      event.clientX > rect.right ||
      event.clientY < rect.top ||
      event.clientY > rect.bottom;
    if (outside) requestClose();
  }

  function moveActive(delta: number): void {
    if (flatRows.length === 0) return;
    let index = flatRows.findIndex((row) => row.key === activeId);
    if (index < 0) index = delta > 0 ? -1 : 0;
    activeId = flatRows[(index + delta + flatRows.length) % flatRows.length].key;
  }

  function moveToEdge(edge: 'start' | 'end'): void {
    if (flatRows.length === 0) return;
    activeId = flatRows[edge === 'start' ? 0 : flatRows.length - 1].key;
  }

  function activate(row: Row): void {
    if (row.kind === 'create') {
      onClose();
      onCreate();
      return;
    }
    if (row.kind === 'worker') {
      remember(row.project.id, row.worker.id);
      onSelect(row.project.id, row.worker.id);
    } else {
      remember(row.project.id, null);
      onSelect(row.project.id);
    }
    onClose();
  }

  function handleInputKeydown(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      moveActive(1);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      moveActive(-1);
    } else if (event.key === 'Home') {
      event.preventDefault();
      moveToEdge('start');
    } else if (event.key === 'End') {
      event.preventDefault();
      moveToEdge('end');
    } else if (event.key === 'PageDown') {
      event.preventDefault();
      moveActive(5);
    } else if (event.key === 'PageUp') {
      event.preventDefault();
      moveActive(-5);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      if (activeRow !== null) activate(activeRow);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      requestClose();
    }
  }

  $effect(() => {
    const rows = flatRows;
    const q = normalizedQuery;
    if (q !== previousQuery) {
      previousQuery = q;
      activeId = rows[0]?.key ?? '';
      return;
    }
    if (!rows.some((row) => row.key === activeId)) {
      activeId = rows[0]?.key ?? '';
    }
  });

  $effect(() => {
    const row = activeRow;
    if (row === null) return;
    void tick().then(() => {
      document.getElementById(rowElementId(row.key))?.scrollIntoView({ block: 'nearest' });
    });
  });

  onMount(() => {
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    recents = readRecents();
    activeId = '';
    if (dialogEl !== null && !dialogEl.open) dialogEl.showModal();
    void tick().then(() => {
      if (!unmounted) inputEl?.focus();
    });
    const timer = window.setInterval(() => {
      now = Date.now();
    }, 30_000);
    return () => {
      unmounted = true;
      window.clearInterval(timer);
      if (dialogEl?.open) dialogEl.close();
      restoreFocusSoon();
    };
  });
</script>

<!-- Palette geometry, rows, and footer ported from Orca worktree-jump palette (MIT). -->
<dialog
  bind:this={dialogEl}
  class="fixed top-[min(10%,4rem)] bottom-auto left-1/2 z-50 m-0 max-h-[min(90vh,calc(100vh-1.5rem))] w-[900px] max-w-[96vw] -translate-x-1/2 overflow-hidden rounded-xl border border-border/70 bg-dialog-surface text-foreground shadow-[0_26px_84px_rgba(0,0,0,0.32)] outline-none backdrop:bg-black/55"
  aria-labelledby="palette-title"
  aria-describedby="palette-description"
  onclick={handleBackdrop}
  onclose={handleNativeClose}
>
  <h2 id="palette-title" class="sr-only">Jump to workspace</h2>
  <p id="palette-description" class="sr-only">
    Search projects and workspaces. Use the arrow keys to move and Enter to open.
  </p>

  <div class="flex max-h-[min(90vh,calc(100vh-1.5rem))] min-h-0 flex-col overflow-hidden rounded-md">
    <div
      class="mx-3 mt-3 flex shrink-0 items-center rounded-lg border border-border/55 bg-muted/28 px-3.5 py-1 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]"
    >
      <Search class="mr-2.5 size-4 shrink-0 text-muted-foreground/60" aria-hidden="true" />
      <input
        bind:this={inputEl}
        class="flex h-12 min-w-0 flex-1 rounded-md bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground"
        type="text"
        bind:value={query}
        placeholder="Search workspaces and projects..."
        aria-label="Search workspaces and projects"
        autocomplete="off"
        spellcheck="false"
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-listbox"
        aria-activedescendant={activeDescendant}
        aria-autocomplete="list"
        aria-haspopup="listbox"
        onkeydown={handleInputKeydown}
      />
      <label class="relative ml-2 inline-flex h-6 shrink-0 items-center gap-1 rounded-md border border-border bg-muted px-1.5 text-xs">
        <ListFilter class="size-3.5" aria-hidden="true"/>
        <span>{category === 'all' ? 'Filter' : category === 'worker' ? 'Workspaces' : 'Projects'}</span>
        <select aria-label="Filter results" bind:value={category} class="absolute inset-0 w-full cursor-pointer opacity-0">
          <option value="all">All results</option><option value="worker">Workspaces</option><option value="project">Projects</option>
        </select>
      </label>
    </div>

    <div
      id="palette-listbox"
      role="listbox"
      aria-label="Workspaces"
      class="min-h-0 max-h-[min(600px,calc(100vh-14rem))] overflow-x-hidden overflow-y-auto px-2.5 pt-2 pb-2.5 scrollbar-sleek"
    >
      {#if sections.length === 0}
        <div class="px-5 py-8 text-center">
          <p class="text-sm font-medium text-foreground">
            {normalizedQuery.length > 0 ? 'No results match your search' : 'No workspaces yet'}
          </p>
          <p class="mt-1 text-xs text-muted-foreground">
            {normalizedQuery.length > 0
              ? 'Try another project or workspace name.'
              : 'Add a project and create your first workspace.'}
          </p>
        </div>
      {/if}

      {#each sections as section, sectionIndex (section.key)}
        <div role="group" aria-labelledby={`palette-section-${sectionIndex}`}>
          <div
            id={`palette-section-${sectionIndex}`}
            class="mx-0.5 mt-3 mb-1 px-3 text-[11px] font-medium tracking-wider text-muted-foreground/70 uppercase"
          >
            {section.label}
          </div>
          {#each section.rows as row (row.key)}
            {@render paletteRow(row)}
          {/each}
        </div>
      {/each}

      {#if showCreate}{@render paletteRow(createRow)}{/if}
    </div>

    <div
      class="flex shrink-0 items-center justify-end border-t border-border/60 px-3.5 py-2.5 text-[11px] text-muted-foreground/82"
    >
      <div class="flex items-center gap-2">
        <span
          class="rounded-full border border-border/60 bg-muted/35 px-2 py-0.5 text-[10px] font-medium text-foreground/85"
        >
          Enter
        </span>
        <span>Open</span>
        <span
          class="rounded-full border border-border/60 bg-muted/35 px-2 py-0.5 text-[10px] font-medium text-foreground/85"
        >
          Esc
        </span>
        <span>Close</span>
        <span
          class="rounded-full border border-border/60 bg-muted/35 px-2 py-0.5 text-[10px] font-medium text-foreground/85"
        >
          ↑↓
        </span>
        <span>Move</span>
        <span class="rounded-full border border-border/60 bg-muted/35 px-2 py-0.5 text-[10px] font-medium text-foreground/85">Tab</span><span>Filter</span>
      </div>
    </div>
  </div>

  <div aria-live="polite" class="sr-only">{resultCount} results</div>
</dialog>

{#snippet paletteRow(row: Row)}
  {@const selected = row.key === activeId}
  {@const age = rowAge(row)}
  <div
    id={rowElementId(row.key)}
    role="option"
    tabindex="-1"
    aria-selected={selected}
    data-selected={selected ? 'true' : undefined}
    class={`group mx-0.5 flex cursor-pointer items-center gap-3 rounded-lg border px-3 py-2.5 text-left outline-none transition-[background-color,border-color,box-shadow] ${
      row.kind === 'create' ? 'mt-1 ' : ''
    }${selected ? 'border-border bg-accent text-foreground' : 'border-transparent'}`}
    onmousedown={(event) => event.preventDefault()}
    onmouseenter={() => {
      activeId = row.key;
    }}
    onclick={() => activate(row)}
    onkeydown={(event) => {
      if (event.key !== 'Enter' && event.key !== ' ') return;
      event.preventDefault();
      activate(row);
    }}
  >
    {#if row.kind === 'create'}
      <div
        class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-dashed border-border/60 bg-muted/25 text-muted-foreground/70"
      >
        <Plus class="size-3.5" aria-hidden="true" />
      </div>
      <div class="min-w-0 flex-1">
        <div class="text-[14px] font-semibold tracking-[-0.01em] text-foreground">
          Create workspace
        </div>
        <div class="mt-0.5 truncate text-[12px] text-muted-foreground">
          Open the workspace composer
        </div>
      </div>
    {:else if row.kind === 'worker'}
      <div class="flex h-5 w-4 shrink-0 items-center justify-center self-start">
        {#if row.worker.status === 'running'}
          <span
            class="block size-3 animate-spin rounded-full border-2 border-agent-working border-t-transparent"
            aria-hidden="true"
          ></span>
        {:else if row.worker.status === 'waiting-for-input'}
          <CircleHelp class="size-3.5 text-agent-question" aria-hidden="true" />
        {:else if row.worker.status === 'idle'}
          <span class="block size-2 rounded-full bg-emerald-500" aria-hidden="true"></span>
        {:else}
          <span
            class={exitedWithError(row.worker)
              ? 'block size-1.5 rounded-full bg-red-500'
              : 'block size-1.5 rounded-full bg-neutral-500/40'}
            aria-hidden="true"
          ></span>
        {/if}
        <span class="sr-only">{statusLabel(row.worker)}</span>
      </div>
      <div class="min-w-0 flex-1">
        <div class="flex items-center justify-between gap-2.5">
          <div class="min-w-0 flex-1">
            <div class="flex min-w-0 items-center gap-2">
              <span class="truncate text-[14px] font-semibold text-foreground">
                {row.worker.name}
              </span>
              {#if age !== null}
                <span
                  aria-label={`Selected ${age} ago`}
                  class="shrink-0 text-[11px] font-medium tabular-nums text-muted-foreground/70"
                >
                  {age}
                </span>
              {/if}
              {#if row.worker.branch.trim().length > 0}
                <span class="shrink-0 text-muted-foreground/45">·</span>
                <span class="truncate text-[12px] font-medium text-muted-foreground/92">
                  {row.worker.branch}
                </span>
              {/if}
            </div>
            {#if row.worker.status === 'exited' && row.worker.last_error !== null}
              <div class="mt-1.5 truncate text-[12px] leading-5 text-red-400/90">
                {row.worker.last_error}
              </div>
            {/if}
          </div>
          <span
            class="inline-flex max-w-[180px] shrink-0 items-center gap-1.5 rounded-md border border-border bg-muted px-2 py-1 text-[11px] leading-none font-semibold text-foreground"
          >
            <FolderTree class="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
            <span class="truncate">{row.project.name}</span>
          </span>
        </div>
      </div>
    {:else}
      <div class="flex h-5 w-4 shrink-0 items-center justify-center self-start text-muted-foreground/85">
        <FolderTree class="size-3.5" aria-hidden="true" />
      </div>
      <div class="min-w-0 flex-1">
        <div class="flex items-center justify-between gap-2.5">
          <div class="flex min-w-0 flex-1 items-center gap-2">
            <span class="truncate text-[14px] font-semibold text-foreground">
              {row.project.name}
            </span>
            <span
              class="shrink-0 rounded-[6px] border border-border/60 bg-background/45 px-1.5 py-px text-[9px] leading-normal font-medium text-muted-foreground/88"
            >
              Project
            </span>
            {#if age !== null}
              <span
                aria-label={`Selected ${age} ago`}
                class="shrink-0 text-[11px] font-medium tabular-nums text-muted-foreground/70"
              >
                {age}
              </span>
            {/if}
          </div>
          <span
            class="inline-flex max-w-[180px] shrink-0 items-center gap-1.5 rounded-md border border-border bg-muted px-2 py-1 font-mono text-[11px] leading-none font-semibold text-foreground"
          >
            <GitBranch class="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
            <span class="truncate">{row.project.base_ref}</span>
          </span>
        </div>
      </div>
    {/if}
  </div>
{/snippet}
