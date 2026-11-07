<script lang="ts">
  import { tick } from 'svelte';
  import {
    CaseSensitive,
    ChevronRight,
    CircleAlert,
    Folder,
    FolderOpen,
    ListCollapse,
    ListFilter,
    Loader2,
    RefreshCw,
    Search,
    X
  } from 'lucide-svelte';
  import type { Worker } from '../types';
  import {
    listWorkspaceDirectory,
    searchWorkspace,
    workspaceChangedEvent,
    type DirectoryEntry,
    type DirectoryListing,
    type SearchMatch,
    type SearchResults
  } from '../workspace-api';
  import {
    baseName,
    dirName,
    fileIconFor,
    joinWorkspacePath,
    normalizeWorkspacePath
  } from './file-icons';
  import { workspaceEventDetail } from './workspace-events';
  import { errorMessage } from '../api';

  let {
    worker,
    mode,
    onOpenFile
  }: {
    worker: Worker;
    mode: 'explorer' | 'search';
    onOpenFile: (path: string, line?: number) => void;
  } = $props();

  interface ExplorerRow {
    entry: DirectoryEntry;
    path: string;
    depth: number;
    expanded: boolean;
    loading: boolean;
    error: string | null;
    truncated: boolean;
  }

  interface SearchGroup {
    path: string;
    name: string;
    dir: string;
    matches: SearchMatch[];
  }

  let root = $state<DirectoryListing | null>(null);
  let rootLoading = $state(false);
  let rootError = $state<string | null>(null);
  let dirCache = $state<Record<string, DirectoryEntry[]>>({});
  let dirTruncated = $state<Record<string, boolean>>({});
  let dirErrors = $state<Record<string, string>>({});
  let loadingDirs = $state<Set<string>>(new Set());
  let expanded = $state<Set<string>>(new Set());
  let filterCollapsed = $state<Set<string>>(new Set());
  let filterOpen = $state(false);
  let filterQuery = $state('');
  let filterInput = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);
  let selectedPath = $state<string | null>(null);
  let focusIndex = $state(0);
  let refreshing = $state(false);

  let searchQuery = $state('');
  let committedQuery = $state('');
  let caseSensitive = $state(false);
  let searchResults = $state<SearchResults | null>(null);
  let searchLoading = $state(false);
  let searchError = $state<string | null>(null);
  let searchCollapsed = $state<Set<string>>(new Set());
  let searchHadCommit = $state(false);
  let searchInput = $state<HTMLInputElement | null>(null);

  const workerId = $derived(worker.id);

  let generation = 0;
  let requestSeq = 0;
  let rootRequestId = 0;
  let searchSeq = 0;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let refreshPromise: Promise<void> | null = null;
  const dirRequestIds = new Map<string, number>();

  const normalizedFilter = $derived(filterQuery.trim().toLowerCase());

  const rows: ExplorerRow[] = $derived.by(() => {
    const listing = root;
    if (!listing) return [];
    const query = normalizedFilter;
    const visible: ExplorerRow[] = [];

    const walk = (entries: DirectoryEntry[], depth: number, parentPath: string): void => {
      for (const entry of entries) {
        const path =
          normalizeWorkspacePath(entry.path) ||
          joinWorkspacePath(parentPath, entry.name);
        const directory = entry.is_dir;
        // While filtering, reuse the folders the user already opened or loaded;
        // an untouched folder stays collapsed rather than claiming hidden children.
        const children = Object.hasOwn(dirCache, path) ? dirCache[path] : undefined;
        const loadedOrOpen =
          children !== undefined ||
          expanded.has(path) ||
          loadingDirs.has(path);
        const isOpen =
          directory &&
          (query ? !filterCollapsed.has(path) && loadedOrOpen : expanded.has(path));
        visible.push({
          entry,
          path,
          depth,
          expanded: isOpen,
          loading: loadingDirs.has(path),
          error: Object.hasOwn(dirErrors, path) ? dirErrors[path] : null,
          truncated: Object.hasOwn(dirTruncated, path)
            ? dirTruncated[path] === true
            : false
        });
        if (!isOpen) continue;
        if (children) walk(children, depth + 1, path);
      }
    };

    walk(listing.entries, 0, '');
    if (!query) return visible;

    const keep = new Set<string>();
    for (const row of visible) {
      if (
        row.entry.name.toLowerCase().includes(query) ||
        row.path.toLowerCase().includes(query)
      ) {
        keep.add(row.path);
        let parent = dirName(row.path);
        while (parent) {
          keep.add(parent);
          parent = dirName(parent);
        }
      }
    }
    return visible.filter((row) => keep.has(row.path));
  });

  const activeIndex = $derived(rows.length ? Math.min(focusIndex, rows.length - 1) : 0);

  const searchGroups: SearchGroup[] = $derived.by(() => {
    const groups = new Map<string, SearchMatch[]>();
    for (const match of searchResults?.matches ?? []) {
      const matches = groups.get(match.path);
      if (matches) matches.push(match);
      else groups.set(match.path, [match]);
    }
    return [...groups.entries()].map(([path, matches]) => ({
      path,
      name: baseName(path),
      dir: dirName(path),
      matches
    }));
  });

  const totalMatches = $derived(
    searchGroups.reduce((sum, group) => sum + group.matches.length, 0)
  );

  function sortEntries(entries: DirectoryEntry[]): DirectoryEntry[] {
    return [...entries].sort((left, right) => {
      if (left.is_dir !== right.is_dir) return left.is_dir ? -1 : 1;
      return left.name.localeCompare(right.name, undefined, {
        numeric: true,
        sensitivity: 'base'
      });
    });
  }

  function withoutKey<T>(record: Record<string, T>, key: string): Record<string, T> {
    if (!(key in record)) return record;
    const next = { ...record };
    delete next[key];
    return next;
  }

  function toggleInSet(set: Set<string>, value: string): Set<string> {
    const next = new Set(set);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    return next;
  }

  function resetExplorer(): void {
    root = null;
    rootLoading = false;
    rootError = null;
    dirCache = {};
    dirTruncated = {};
    dirErrors = {};
    loadingDirs = new Set();
    expanded = new Set();
    filterCollapsed = new Set();
    filterOpen = false;
    filterQuery = '';
    selectedPath = null;
    focusIndex = 0;
    refreshing = false;
    refreshPromise = null;
    dirRequestIds.clear();
  }

  function resetSearch(): void {
    if (searchTimer) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }
    searchQuery = '';
    committedQuery = '';
    caseSensitive = false;
    searchResults = null;
    searchLoading = false;
    searchError = null;
    searchCollapsed = new Set();
    searchHadCommit = false;
  }

  async function loadRoot(
    requestedWorkerId: string,
    generationAtRequest: number
  ): Promise<void> {
    const requestId = ++rootRequestId;
    rootLoading = true;
    rootError = null;
    try {
      const listing = await listWorkspaceDirectory(requestedWorkerId, '');
      if (
        generationAtRequest !== generation ||
        requestedWorkerId !== workerId ||
        requestId !== rootRequestId
      ) {
        return;
      }
      root = { entries: sortEntries(listing.entries), truncated: listing.truncated };
    } catch (error) {
      if (
        generationAtRequest !== generation ||
        requestedWorkerId !== workerId ||
        requestId !== rootRequestId
      ) {
        return;
      }
      rootError = errorMessage(error);
    } finally {
      if (generationAtRequest === generation && requestId === rootRequestId) {
        rootLoading = false;
      }
    }
  }

  async function loadDirectory(
    requestedWorkerId: string,
    path: string,
    generationAtRequest: number
  ): Promise<void> {
    const requestId = ++requestSeq;
    dirRequestIds.set(path, requestId);
    loadingDirs = new Set(loadingDirs).add(path);
    dirErrors = withoutKey(dirErrors, path);
    try {
      const listing = await listWorkspaceDirectory(requestedWorkerId, path);
      if (
        generationAtRequest !== generation ||
        requestedWorkerId !== workerId ||
        dirRequestIds.get(path) !== requestId
      ) {
        return;
      }
      dirCache = { ...dirCache, [path]: sortEntries(listing.entries) };
      dirTruncated = { ...dirTruncated, [path]: listing.truncated };
    } catch (error) {
      if (
        generationAtRequest === generation &&
        requestedWorkerId === workerId &&
        dirRequestIds.get(path) === requestId
      ) {
        dirErrors = { ...dirErrors, [path]: errorMessage(error) };
      }
    } finally {
      if (generationAtRequest === generation && dirRequestIds.get(path) === requestId) {
        const next = new Set(loadingDirs);
        next.delete(path);
        loadingDirs = next;
      }
    }
  }

  function refreshTree(manual = false): Promise<void> {
    if (refreshPromise) return refreshPromise;
    const generationAtRequest = generation;
    if (manual) refreshing = true;
    const task = (async () => {
      await loadRoot(workerId, generationAtRequest);
      if (generationAtRequest !== generation) return;
      const directories = [...expanded];
      await Promise.all(
        directories.map((path) =>
          loadDirectory(workerId, path, generationAtRequest)
        )
      );
    })().finally(() => {
      if (generationAtRequest === generation) {
        refreshing = false;
        refreshPromise = null;
      }
    });
    refreshPromise = task;
    return task;
  }

  function ensureDirectory(row: ExplorerRow): void {
    if (!row.entry.is_dir) return;
    if (loadingDirs.has(row.path)) return;
    const cached = Object.hasOwn(dirCache, row.path)
      ? dirCache[row.path]
      : undefined;
    const failed = Object.hasOwn(dirErrors, row.path)
      ? dirErrors[row.path]
      : undefined;
    if (cached !== undefined && failed === undefined) return;
    void loadDirectory(workerId, row.path, generation);
  }

  function toggleDirectory(row: ExplorerRow): void {
    if (!row.entry.is_dir) return;
    if (normalizedFilter) {
      const loadedOrOpen =
        Object.hasOwn(dirCache, row.path) ||
        expanded.has(row.path) ||
        loadingDirs.has(row.path);
      const collapsed = filterCollapsed.has(row.path) || !loadedOrOpen;
      const next = new Set(filterCollapsed);
      if (collapsed) {
        next.delete(row.path);
        filterCollapsed = next;
        ensureDirectory(row);
      } else {
        next.add(row.path);
        filterCollapsed = next;
      }
      return;
    }
    expanded = toggleInSet(expanded, row.path);
    if (expanded.has(row.path)) ensureDirectory(row);
  }

  function activateRow(row: ExplorerRow): void {
    selectedPath = row.path;
    if (row.entry.is_dir) toggleDirectory(row);
    else onOpenFile(row.path);
  }

  function focusRow(index: number): void {
    if (!rows.length) return;
    const clamped = Math.max(0, Math.min(index, rows.length - 1));
    focusIndex = clamped;
    void tick().then(() => {
      const element = listEl?.querySelector<HTMLElement>(
        `[data-explorer-index="${clamped}"]`
      );
      element?.focus();
      element?.scrollIntoView({ block: 'nearest' });
    });
  }

  function parentRowIndex(row: ExplorerRow): number {
    const parent = dirName(row.path);
    if (!parent) return -1;
    for (let index = rows.length - 1; index >= 0; index -= 1) {
      const candidate = rows[index];
      if (candidate.entry.is_dir && candidate.path === parent) return index;
    }
    return -1;
  }

  function handleRowKey(event: KeyboardEvent, index: number): void {
    const row = rows[index];
    if (!row) return;
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        focusRow(index + 1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        focusRow(index - 1);
        break;
      case 'ArrowRight':
        event.preventDefault();
        if (!row.entry.is_dir) break;
        if (!row.expanded) {
          selectedPath = row.path;
          toggleDirectory(row);
        } else if (rows.length > index + 1) {
          focusRow(index + 1);
        }
        break;
      case 'ArrowLeft': {
        event.preventDefault();
        if (row.entry.is_dir && row.expanded) {
          toggleDirectory(row);
          break;
        }
        const parent = parentRowIndex(row);
        if (parent >= 0) focusRow(parent);
        break;
      }
      case 'Home':
        event.preventDefault();
        focusRow(0);
        break;
      case 'End':
        event.preventDefault();
        focusRow(rows.length - 1);
        break;
      case 'Enter':
      case ' ':
        event.preventDefault();
        activateRow(row);
        break;
    }
  }

  async function toggleFilter(): Promise<void> {
    filterOpen = !filterOpen;
    if (!filterOpen) {
      filterQuery = '';
      filterCollapsed = new Set();
      return;
    }
    await tick();
    filterInput?.focus();
    filterInput?.select();
  }

  function closeFilter(): void {
    filterOpen = false;
    filterQuery = '';
    filterCollapsed = new Set();
  }

  function handleFilterKey(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key !== 'Escape') return;
    event.preventDefault();
    closeFilter();
  }

  function collapseAll(): void {
    expanded = new Set();
    filterCollapsed = new Set();
    focusIndex = 0;
  }

  function clearSearch(): void {
    if (searchTimer) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }
    searchSeq += 1;
    searchQuery = '';
    committedQuery = '';
    searchResults = null;
    searchLoading = false;
    searchError = null;
    searchCollapsed = new Set();
    searchHadCommit = false;
    // Keep Match Case as a session preference; clearing a query should not reset it.
  }

  function queueSearch(event?: Event): void {
    const target = event?.currentTarget;
    if (target instanceof HTMLInputElement) searchQuery = target.value;
    if (searchTimer) clearTimeout(searchTimer);
    const query = searchQuery.trim();
    if (!query) {
      clearSearch();
      return;
    }
    searchTimer = setTimeout(() => {
      searchTimer = null;
      void runSearch(query);
    }, 500);
  }

  function submitSearch(): void {
    if (searchTimer) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }
    const query = searchQuery.trim();
    if (!query) {
      clearSearch();
      return;
    }
    void runSearch(query);
  }

  async function runSearch(rawQuery: string): Promise<void> {
    const query = rawQuery.trim();
    if (!query) return;
    const requestWorkerId = workerId;
    const generationAtRequest = generation;
    const sequence = ++searchSeq;
    committedQuery = query;
    searchHadCommit = true;
    searchError = null;
    searchLoading = true;
    try {
      const results = await searchWorkspace(requestWorkerId, query, caseSensitive);
      if (
        sequence !== searchSeq ||
        generationAtRequest !== generation ||
        requestWorkerId !== workerId
      ) {
        return;
      }
      searchResults = results;
    } catch (error) {
      if (
        sequence !== searchSeq ||
        generationAtRequest !== generation ||
        requestWorkerId !== workerId
      ) {
        return;
      }
      searchResults = null;
      searchError = errorMessage(error);
    } finally {
      if (sequence === searchSeq) searchLoading = false;
    }
  }

  function toggleCaseSensitive(): void {
    caseSensitive = !caseSensitive;
    const query = searchQuery.trim() || committedQuery;
    if (!query) return;
    if (searchTimer) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }
    void runSearch(query);
  }

  function handleSearchKey(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key === 'Enter') {
      event.preventDefault();
      submitSearch();
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      clearSearch();
    }
  }

  function toggleSearchGroup(path: string): void {
    searchCollapsed = toggleInSet(searchCollapsed, path);
  }

  function matchParts(text: string, query: string): { before: string; hit: string; after: string } {
    const source = text.trimStart();
    if (!query) return { before: source, hit: '', after: '' };
    const haystack = caseSensitive ? source : source.toLowerCase();
    const needle = caseSensitive ? query : query.toLowerCase();
    const at = haystack.indexOf(needle);
    if (at < 0) return { before: source, hit: '', after: '' };
    const trimAt = Math.max(0, at - 26);
    const space = trimAt > 0 ? source.indexOf(' ', trimAt) : -1;
    const start = space >= 0 && space < at ? space + 1 : trimAt;
    return {
      before: `${start > 0 ? '…' : ''}${source.slice(start, at)}`,
      hit: source.slice(at, at + needle.length),
      after: source.slice(at + needle.length)
    };
  }

  $effect(() => {
    const currentWorkerId = workerId;
    generation += 1;
    const generationAtRequest = generation;
    if (mode === 'explorer') resetExplorer();
    else resetSearch();

    const handleWorkspaceChanged = (event: Event): void => {
      const detail = workspaceEventDetail(event);
      if (typeof detail === 'string' && detail !== currentWorkerId) return;
      if (mode === 'explorer') {
        void refreshTree(false);
      } else {
        const query = searchQuery.trim() || committedQuery;
        if (query) void runSearch(query);
      }
    };
    const handleFocus = (): void => {
      if (mode === 'explorer') {
        if (root !== null) void refreshTree(false);
      } else {
        const query = searchQuery.trim() || committedQuery;
        if (query) void runSearch(query);
      }
    };

    window.addEventListener(workspaceChangedEvent, handleWorkspaceChanged);
    window.addEventListener('focus', handleFocus);

    if (mode === 'explorer') {
      void loadRoot(currentWorkerId, generationAtRequest);
    } else {
      void tick().then(() => searchInput?.focus());
    }

    return () => {
      window.removeEventListener(workspaceChangedEvent, handleWorkspaceChanged);
      window.removeEventListener('focus', handleFocus);
      generation += 1;
      refreshPromise = null;
      if (searchTimer) {
        clearTimeout(searchTimer);
        searchTimer = null;
      }
      searchSeq += 1;
    };
  });
</script>

{#if mode === 'search'}
  <div class="flex h-full min-h-0 w-full flex-col overflow-hidden">
    <div class="border-b border-border px-2 py-1.5">
      <div
        class="flex h-8 items-center gap-1 rounded-md border border-border bg-input/50 px-2 focus-within:border-ring focus-within:ring-1 focus-within:ring-ring"
      >
        <Search size={14} class="shrink-0 text-muted-foreground" aria-hidden="true" />
        <input
          bind:this={searchInput}
          bind:value={searchQuery}
          oninput={queueSearch}
          onkeydown={handleSearchKey}
          type="text"
          spellcheck="false"
          class="min-w-0 flex-1 bg-transparent py-1 text-xs text-foreground outline-none placeholder:text-muted-foreground"
          aria-label="Search files"
          placeholder="Search"
        />
        {#if searchLoading}
          <Loader2 size={12} class="shrink-0 animate-spin text-muted-foreground" aria-hidden="true" />
        {/if}
        {#if searchQuery}
          <button
            type="button"
            class="inline-flex shrink-0 items-center justify-center rounded-sm p-0.5 text-muted-foreground transition-colors hover:text-foreground"
            onclick={clearSearch}
            aria-label="Clear search"
            title="Clear search"
          >
            <X size={12} />
          </button>
        {/if}
        <button
          type="button"
          class={`inline-flex shrink-0 items-center justify-center rounded-sm p-0.5 transition-colors ${
            caseSensitive
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
          onclick={toggleCaseSensitive}
          aria-label="Match Case"
          aria-pressed={caseSensitive}
          title="Match Case"
        >
          <CaseSensitive size={14} />
        </button>
      </div>
    </div>

    {#if searchResults && (totalMatches > 0 || searchResults.truncated)}
      <div
        class="shrink-0 border-b border-border px-2 py-1 text-[10px] text-muted-foreground"
        role="status"
      >
        {totalMatches} {totalMatches === 1 ? 'result' : 'results'} in
        {searchGroups.length} {searchGroups.length === 1 ? 'file' : 'files'}
        {#if searchResults.truncated}
          <span class="text-amber-600 dark:text-amber-400">(results truncated)</span>
        {/if}
      </div>
    {/if}

    <div class="min-h-0 flex-1 overflow-y-auto scrollbar-sleek">
      {#if searchError}
        <div class="m-2 rounded-md border border-destructive/30 bg-destructive/5 p-2" role="alert">
          <div class="flex items-start gap-1.5 text-xs text-destructive">
            <CircleAlert size={14} class="mt-0.5 shrink-0" aria-hidden="true" />
            <p class="min-w-0 flex-1 break-words">{searchError}</p>
          </div>
          <button
            type="button"
            class="mt-2 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
            onclick={() => void runSearch(committedQuery || searchQuery)}
          >
            Retry
          </button>
        </div>
      {:else if searchLoading && !searchResults}
        <div class="flex h-32 items-center justify-center gap-2 text-xs text-muted-foreground" role="status">
          <Loader2 size={14} class="animate-spin" aria-hidden="true" />
          Searching…
        </div>
      {:else if !searchHadCommit && !searchQuery.trim()}
        <div class="flex h-32 items-center justify-center text-xs text-muted-foreground">
          Type to search in files
        </div>
      {:else if !searchHadCommit && !searchLoading}
        <div class="flex h-32 items-center justify-center text-xs text-muted-foreground">
          Press Enter to search
        </div>
      {:else if searchHadCommit && searchGroups.length === 0 && !searchLoading}
        <div class="px-3 py-6 text-xs text-muted-foreground">
          No results. Try another search.
        </div>
      {:else}
        <div class="pb-2">
          {#each searchGroups as group (group.path)}
            {@const FileIcon = fileIconFor(group.path)}
            <div class="pt-1.5">
              <button
                type="button"
                class="group flex w-full items-center gap-1 rounded-none px-2 py-0.5 text-left transition-colors hover:bg-accent"
                aria-expanded={!searchCollapsed.has(group.path)}
                title={group.path}
                onclick={() => toggleSearchGroup(group.path)}
              >
                <ChevronRight
                  size={12}
                  class={`shrink-0 text-muted-foreground transition-transform ${
                    searchCollapsed.has(group.path) ? '' : 'rotate-90'
                  }`}
                  aria-hidden="true"
                />
                <FileIcon size={14} class="shrink-0 text-muted-foreground" />
                <span class="min-w-0 flex-1 text-xs">
                  <span class="block truncate">
                    <span class="text-foreground">{group.name}</span>
                    {#if group.dir}
                      <span class="ml-1.5 text-[11px] text-muted-foreground">{group.dir}</span>
                    {/if}
                  </span>
                </span>
                <span
                  class="shrink-0 rounded-full bg-muted/80 px-1.5 text-[10px] tabular-nums text-muted-foreground"
                >
                  {group.matches.length}
                </span>
              </button>
              {#if !searchCollapsed.has(group.path)}
                {#each group.matches as match (match.path + ':' + match.line + ':' + match.column)}
                  {@const parts = matchParts(match.text, committedQuery)}
                  <button
                    type="button"
                    class="flex min-h-[18px] w-full items-baseline gap-1 rounded-none py-px pl-7 pr-2 text-left transition-colors hover:bg-accent"
                    title={`${match.path}:${match.line}`}
                    onclick={(event) => {
                      if (event.detail > 1) return;
                      onOpenFile(match.path, match.line);
                    }}
                  >
                    <span class="shrink-0 text-[10px] tabular-nums text-muted-foreground">
                      {match.line}
                    </span>
                    <span class="min-w-0 flex-1 truncate whitespace-pre text-[11px]">
                      <span class="text-muted-foreground">{parts.before}</span>
                      {#if parts.hit}
                        <span class="rounded-sm bg-annotation-highlight/30 text-foreground">
                          {parts.hit}
                        </span>
                      {/if}
                      <span class="text-muted-foreground">{parts.after}</span>
                    </span>
                  </button>
                {/each}
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{:else}
  <div class="flex h-full min-h-0 w-full flex-col overflow-hidden">
    <div class="flex h-8 min-h-8 items-center gap-2 border-b border-border px-2">
      <span
        class="min-w-0 flex-1 truncate text-xs font-medium text-foreground"
        title={worker.path}
      >
        {baseName(worker.path) || worker.name}
      </span>
      <button
        type="button"
        class={`inline-flex size-7 shrink-0 items-center justify-center rounded-sm transition-colors ${
          filterOpen || expanded.size === 0
            ? 'cursor-not-allowed text-muted-foreground/50'
            : 'text-muted-foreground hover:bg-accent hover:text-foreground'
        }`}
        aria-label="Collapse All"
        aria-disabled={filterOpen || expanded.size === 0}
        title={filterOpen ? 'Clear filter to collapse all' : 'Collapse All'}
        onclick={() => {
          if (filterOpen || expanded.size === 0) return;
          collapseAll();
        }}
      >
        <ListCollapse size={12} />
      </button>
      <button
        type="button"
        class={`inline-flex size-7 shrink-0 items-center justify-center rounded-sm transition-colors ${
          filterOpen
            ? 'bg-accent text-foreground'
            : 'text-muted-foreground hover:bg-accent hover:text-foreground'
        }`}
        aria-label="Filter files"
        aria-pressed={filterOpen}
        title="Filter files"
        onclick={() => void toggleFilter()}
      >
        <ListFilter size={12} />
      </button>
      <button
        type="button"
        class="inline-flex size-7 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
        aria-label="Refresh Explorer"
        title="Refresh Explorer"
        disabled={refreshing || rootLoading}
        onclick={() => void refreshTree(true)}
      >
        {#if refreshing || rootLoading}
          <Loader2 size={12} class="animate-spin" />
        {:else}
          <RefreshCw size={12} />
        {/if}
      </button>
    </div>

    {#if filterOpen}
      <div class="border-b border-border px-2 py-1.5">
        <div
          class="flex h-8 items-center gap-1 rounded-md border border-border bg-input/50 px-2 focus-within:border-ring focus-within:ring-1 focus-within:ring-ring"
        >
          <ListFilter size={14} class="shrink-0 text-muted-foreground" aria-hidden="true" />
          <input
            bind:this={filterInput}
            bind:value={filterQuery}
            onkeydown={handleFilterKey}
            type="text"
            spellcheck="false"
            class="min-w-0 flex-1 bg-transparent py-1 text-xs text-foreground outline-none placeholder:text-muted-foreground"
            aria-label="Find files"
            placeholder="Find files"
          />
          {#if filterQuery}
            <button
              type="button"
              class="inline-flex shrink-0 items-center justify-center rounded-sm p-0.5 text-muted-foreground transition-colors hover:text-foreground"
              onclick={() => {
                filterQuery = '';
                filterCollapsed = new Set();
              }}
              aria-label="Clear file filter"
              title="Clear file filter"
            >
              <X size={12} />
            </button>
          {/if}
          <button
            type="button"
            class="inline-flex shrink-0 items-center justify-center rounded-sm p-0.5 text-muted-foreground transition-colors hover:text-foreground"
            onclick={closeFilter}
            aria-label="Close filter"
            title="Close filter"
          >
            <X size={12} />
          </button>
        </div>
      </div>
    {/if}

    <div
      bind:this={listEl}
      class="min-h-0 flex-1 overflow-y-auto scrollbar-sleek"
      role="tree"
      aria-label="Workspace files"
    >
      {#if rootError && root}
        <div class="m-2 rounded-md border border-destructive/30 bg-destructive/5 p-2" role="alert">
          <div class="flex items-start gap-1.5 text-xs text-destructive">
            <CircleAlert size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
            <p class="min-w-0 flex-1 break-words">{rootError}</p>
            <button
              type="button"
              class="shrink-0 text-muted-foreground transition-colors hover:text-foreground"
              aria-label="Retry loading files"
              onclick={() => void refreshTree(true)}
            >
              <RefreshCw size={12} />
            </button>
          </div>
        </div>
      {/if}

      {#if rootLoading && !root}
        <div class="flex h-32 items-center justify-center gap-2 text-xs text-muted-foreground" role="status">
          <Loader2 size={14} class="animate-spin" aria-hidden="true" />
          Loading files…
        </div>
      {:else if rootError && !root}
        <div class="m-2 rounded-md border border-destructive/30 bg-destructive/5 p-2" role="alert">
          <div class="flex items-start gap-1.5 text-xs text-destructive">
            <CircleAlert size={14} class="mt-0.5 shrink-0" aria-hidden="true" />
            <p class="min-w-0 flex-1 break-words">{rootError}</p>
          </div>
          <button
            type="button"
            class="mt-2 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
            onclick={() => void refreshTree(true)}
          >
            Retry
          </button>
        </div>
      {:else if rows.length === 0}
        <div class="px-3 py-6 text-xs text-muted-foreground">
          {#if normalizedFilter}
            No matching files in the loaded folders.
          {:else}
            No files in this workspace.
          {/if}
        </div>
      {:else}
        <div class="py-1">
          {#each rows as row, index (row.path)}
            {@const FileIcon = fileIconFor(row.path)}
            <button
              type="button"
              role="treeitem"
              aria-level={row.depth + 1}
              aria-selected={selectedPath === row.path}
              aria-expanded={row.entry.is_dir ? row.expanded : undefined}
              data-explorer-index={index}
              tabindex={index === activeIndex ? 0 : -1}
              class={`group flex w-full items-center gap-1 rounded-sm px-2 py-1 text-left text-xs transition-colors ${
                selectedPath === row.path
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-accent hover:text-foreground'
              }`}
              style:padding-left={row.depth * 16 + 8 + 'px'}
              title={row.path}
              onfocus={() => {
                focusIndex = index;
              }}
              onclick={(event) => {
                if (event.detail > 1) return;
                activateRow(row);
              }}
              onkeydown={(event) => handleRowKey(event, index)}
            >
              {#if row.entry.is_dir}
                <ChevronRight
                  size={12}
                  class={`shrink-0 text-muted-foreground transition-transform ${
                    row.expanded ? 'rotate-90' : ''
                  }`}
                  aria-hidden="true"
                />
                {#if row.loading}
                  <Loader2 size={12} class="shrink-0 animate-spin text-muted-foreground" aria-hidden="true" />
                {:else if row.error}
                  <CircleAlert size={12} class="shrink-0 text-destructive" aria-hidden="true" />
                {:else if row.expanded}
                  <FolderOpen size={12} class="shrink-0 text-muted-foreground" aria-hidden="true" />
                {:else}
                  <Folder size={12} class="shrink-0 text-muted-foreground" aria-hidden="true" />
                {/if}
              {:else}
                <span class="size-3 shrink-0" aria-hidden="true"></span>
                <FileIcon size={12} class="shrink-0 text-muted-foreground" />
              {/if}
              <span class="min-w-0 flex-1 truncate">{row.entry.name}</span>
              {#if row.error}
                <span class="max-w-[55%] shrink-0 truncate text-[10px] text-destructive" title={row.error}>
                  {row.error}
                </span>
              {:else if row.truncated}
                <span class="shrink-0 text-[10px] text-amber-600 dark:text-amber-400" title="Listing truncated by the workspace limit">truncated</span>
              {/if}
            </button>
          {/each}
        </div>
        {#if root?.truncated}
          <div class="border-t border-border px-2 py-1 text-[10px] text-muted-foreground">
            Listing truncated by the workspace limit.
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}
