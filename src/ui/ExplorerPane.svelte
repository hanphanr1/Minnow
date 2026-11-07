<script lang="ts">
  import { tick, untrack } from 'svelte';
  import {
    CaseSensitive,
    Check,
    ChevronRight,
    CircleAlert,
    CircleSlash,
    Ellipsis,
    ExternalLink,
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
    findWorkspaceFiles,
    getWorkspaceGitIgnored,
    getWorkspaceGitStatus,
    listWorkspaceDirectory,
    searchWorkspace,
    workspaceChangedEvent,
    type DirectoryEntry,
    type DirectoryListing,
    type GitStatus,
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
    view,
    onView,
    onOpenFile,
    onOpenEditor
  }: {
    worker: Worker;
    /** Orca's Explorer search mode: filter by Names or search file Contents. */
    view: 'files' | 'search';
    onView: (view: 'files' | 'search') => void;
    onOpenFile: (path: string, line?: number) => void;
    onOpenEditor?: () => void;
  } = $props();

  type GitName = 'modified' | 'added' | 'deleted' | 'renamed' | 'untracked' | 'copied';
  // Same vocabulary, labels and precedence as Orca's git-status-labels / git-path-decoration.
  const STATUS_BY_CODE: Record<string, GitName> = {
    M: 'modified', T: 'modified', U: 'modified', A: 'added', D: 'deleted',
    R: 'renamed', C: 'copied', '?': 'untracked'
  };
  const STATUS_LABEL: Record<GitName, string> = {
    modified: 'M', added: 'A', deleted: 'D', renamed: 'R', untracked: 'U', copied: 'C'
  };
  const STATUS_PRIORITY: Record<GitName, number> = {
    deleted: 5, modified: 4, added: 3, untracked: 3, renamed: 2, copied: 1
  };
  function dominant(left: GitName | undefined, right: GitName): GitName {
    return left === undefined || STATUS_PRIORITY[right] > STATUS_PRIORITY[left] ? right : left;
  }

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
  let filterQuery = $state('');
  let filterInput = $state<HTMLInputElement | null>(null);
  // Global Find files: every matching workspace file, not just the loaded folders.
  let filterFiles = $state<string[] | null>(null);
  let filterTruncated = $state(false);
  let filterLoading = $state(false);
  let filterError = $state<string | null>(null);
  let gitStatus = $state<GitStatus | null>(null);
  let ignoredSet = $state<Set<string>>(new Set());
  let moreOpen = $state(false);
  let moreButton = $state<HTMLButtonElement | null>(null);
  let moreMenu = $state<HTMLDivElement | null>(null);
  const prefsKey = (id: string): string => `minnow.explorer.view.${id}`;
  let showDotfiles = $state(true);
  let showIgnored = $state(true);
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
  let filterTimer: ReturnType<typeof setTimeout> | null = null;
  let filterSeq = 0;
  let refreshPromise: Promise<void> | null = null;
  const dirRequestIds = new Map<string, number>();
  const ignoredAsked = new Set<string>();

  const filterTokens = $derived(
    filterQuery.length > 2048 ? [] : filterQuery.toLowerCase().split(/\s+/).filter(Boolean)
  );
  const hasNameFilter = $derived(filterTokens.length > 0);
  const filterTooLarge = $derived(filterQuery.length > 2048);

  /** Path -> Git decoration, folders carry the dominant status of what they contain. */
  const statusByPath = $derived.by(() => {
    const map = new Map<string, GitName>();
    const folders = new Map<string, GitName>();
    if (!gitStatus) return map;
    for (const change of [...gitStatus.staged, ...gitStatus.unstaged, ...gitStatus.untracked]) {
      const status = STATUS_BY_CODE[change.status] ?? 'modified';
      const path = normalizeWorkspacePath(change.path);
      map.set(path, dominant(map.get(path), status));
      if (status === 'deleted') continue;
      const segments = path.split('/');
      let folder = '';
      for (const segment of segments.slice(0, -1)) {
        folder = folder ? `${folder}/${segment}` : segment;
        folders.set(folder, dominant(folders.get(folder), status));
      }
    }
    for (const [folder, status] of folders) if (!map.has(folder)) map.set(folder, status);
    return map;
  });

  function isIgnoredPath(path: string): boolean {
    if (ignoredSet.size === 0) return false;
    let candidate = path;
    for (;;) {
      if (ignoredSet.has(candidate)) return true;
      const slash = candidate.lastIndexOf('/');
      if (slash <= 0) return false;
      candidate = candidate.slice(0, slash);
    }
  }

  function hiddenByToggles(path: string): boolean {
    if (!showDotfiles && path.split('/').some((segment) => segment.startsWith('.'))) return true;
    return !showIgnored && isIgnoredPath(path);
  }

  const rows: ExplorerRow[] = $derived.by(() => {
    const visible: ExplorerRow[] = [];
    const statusFor = (path: string): GitName | null => statusByPath.get(path) ?? null;
    void statusFor;

    if (hasNameFilter) {
      // Orca's name-filter projection: a synthetic tree of every matching file with
      // its folders opened; folders the user closed stay closed.
      if (filterFiles === null) return visible;
      interface Node { entry: DirectoryEntry; children: Map<string, Node>; }
      const top = new Map<string, Node>();
      for (const file of filterFiles) {
        if (hiddenByToggles(file)) continue;
        const segments = file.split('/');
        let level = top;
        let current = '';
        segments.forEach((name, index) => {
          current = current ? `${current}/${name}` : name;
          const isDir = index < segments.length - 1;
          let node = level.get(name);
          if (!node) {
            node = { entry: { name, path: current, is_dir: isDir, is_symlink: false, size: 0 }, children: new Map() };
            level.set(name, node);
          } else if (isDir && !node.entry.is_dir) {
            node.entry = { ...node.entry, is_dir: true };
          }
          level = node.children;
        });
      }
      const emit = (nodes: Map<string, Node>, depth: number): void => {
        const sorted = [...nodes.values()].sort((left, right) => {
          if (left.entry.is_dir !== right.entry.is_dir) return left.entry.is_dir ? -1 : 1;
          return left.entry.name.localeCompare(right.entry.name, undefined, { numeric: true, sensitivity: 'base' });
        });
        for (const node of sorted) {
          const open = node.children.size > 0 && !filterCollapsed.has(node.entry.path);
          visible.push({
            entry: node.entry, path: node.entry.path, depth, expanded: open,
            loading: false, error: null, truncated: false
          });
          if (open) emit(node.children, depth + 1);
        }
      };
      emit(top, 0);
      return visible;
    }

    const listing = root;
    if (!listing) return visible;
    const walk = (entries: DirectoryEntry[], depth: number, parentPath: string): void => {
      for (const entry of entries) {
        const path =
          normalizeWorkspacePath(entry.path) ||
          joinWorkspacePath(parentPath, entry.name);
        if (hiddenByToggles(path)) continue;
        const directory = entry.is_dir;
        const children = Object.hasOwn(dirCache, path) ? dirCache[path] : undefined;
        const isOpen = directory && expanded.has(path);
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
    return visible;
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
    filterQuery = '';
    filterFiles = null;
    filterTruncated = false;
    filterLoading = false;
    filterError = null;
    gitStatus = null;
    ignoredSet = new Set();
    ignoredAsked.clear();
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
      void loadIgnored(requestedWorkerId, generationAtRequest, listing.entries.map((entry) => normalizeWorkspacePath(entry.path) || entry.name));
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
      void loadIgnored(requestedWorkerId, generationAtRequest, listing.entries.map((entry) => normalizeWorkspacePath(entry.path) || joinWorkspacePath(path, entry.name)));
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

  async function loadIgnored(
    requestedWorkerId: string,
    generationAtRequest: number,
    paths: string[]
  ): Promise<void> {
    const fresh = paths.filter((path) => path && !ignoredAsked.has(path) && !isIgnoredPath(path));
    if (fresh.length === 0) return;
    for (const path of fresh) ignoredAsked.add(path);
    try {
      for (let index = 0; index < fresh.length; index += 1000) {
        const found = await getWorkspaceGitIgnored(requestedWorkerId, fresh.slice(index, index + 1000));
        if (generationAtRequest !== generation || requestedWorkerId !== workerId) return;
        if (found.length > 0) ignoredSet = new Set([...ignoredSet, ...found]);
      }
    } catch {
      // Without ignore data the tree just shows no ignored decoration.
      for (const path of fresh) ignoredAsked.delete(path);
    }
  }

  async function loadGitStatus(requestedWorkerId: string, generationAtRequest: number): Promise<void> {
    try {
      const status = await getWorkspaceGitStatus(requestedWorkerId);
      if (generationAtRequest !== generation || requestedWorkerId !== workerId) return;
      gitStatus = status;
    } catch {
      if (generationAtRequest === generation && requestedWorkerId === workerId) gitStatus = null;
    }
  }

  function runNameFilter(): void {
    if (filterTimer) {
      clearTimeout(filterTimer);
      filterTimer = null;
    }
    const sequence = ++filterSeq;
    if (!hasNameFilter || filterTooLarge) {
      filterFiles = null; filterLoading = false; filterError = null; filterTruncated = false;
      return;
    }
    filterFiles = null;
    filterLoading = true;
    filterError = null;
    const requestedWorkerId = workerId;
    const generationAtRequest = generation;
    const query = filterQuery;
    filterTimer = setTimeout(async () => {
      filterTimer = null;
      try {
        const result = await findWorkspaceFiles(requestedWorkerId, query, showDotfiles);
        if (sequence !== filterSeq || generationAtRequest !== generation) return;
        filterFiles = result.paths;
        filterTruncated = result.truncated;
        void loadIgnored(requestedWorkerId, generationAtRequest, [
          ...new Set(result.paths.flatMap((path) => {
            const parts = path.split('/');
            return parts.map((_, index) => parts.slice(0, index + 1).join('/'));
          }))
        ].slice(0, 4000));
      } catch (error) {
        if (sequence !== filterSeq || generationAtRequest !== generation) return;
        filterFiles = [];
        filterError = errorMessage(error);
      } finally {
        if (sequence === filterSeq) filterLoading = false;
      }
    }, 120);
  }

  function refreshTree(manual = false): Promise<void> {
    if (refreshPromise) return refreshPromise;
    const generationAtRequest = generation;
    if (manual) refreshing = true;
    const task = (async () => {
      ignoredAsked.clear();
      void loadGitStatus(workerId, generationAtRequest);
      if (hasNameFilter) runNameFilter();
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
    if (hasNameFilter) {
      const next = new Set(filterCollapsed);
      if (row.expanded) next.add(row.path);
      else next.delete(row.path);
      filterCollapsed = next;
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

  function clearFilter(): void {
    filterQuery = '';
    filterCollapsed = new Set();
    runNameFilter();
  }

  function handleFilterKey(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key !== 'Escape' || !filterQuery) return;
    event.preventDefault();
    clearFilter();
  }

  function selectView(next: 'files' | 'search'): void {
    if (next === view) return;
    const focusedInput = document.activeElement instanceof HTMLInputElement
      && document.activeElement.hasAttribute('data-explorer-query-input');
    onView(next);
    if (focusedInput) {
      void tick().then(() => (next === 'files' ? filterInput : searchInput)?.focus({ preventScroll: true }));
    }
  }

  function toggleDotfiles(): void {
    showDotfiles = !showDotfiles;
    persistPrefs();
    if (hasNameFilter) runNameFilter();
  }

  function toggleIgnored(): void {
    showIgnored = !showIgnored;
    persistPrefs();
  }

  function persistPrefs(): void {
    try {
      localStorage.setItem(prefsKey(workerId), JSON.stringify({ showDotfiles, showIgnored }));
    } catch {
      // The toggles still work for this session.
    }
  }

  function loadPrefs(id: string): void {
    showDotfiles = true;
    showIgnored = true;
    try {
      const saved = JSON.parse(localStorage.getItem(prefsKey(id)) ?? 'null');
      if (saved && typeof saved === 'object') {
        if (typeof saved.showDotfiles === 'boolean') showDotfiles = saved.showDotfiles;
        if (typeof saved.showIgnored === 'boolean') showIgnored = saved.showIgnored;
      }
    } catch {
      // Unreadable preferences fall back to Orca's defaults (both shown).
    }
  }

  function collapseAll(): void {
    expanded = new Set();
    filterCollapsed = new Set();
    focusIndex = 0;
  }

  const canCollapseAll = $derived(view === 'files' && !hasNameFilter && expanded.size > 0);

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
    resetExplorer();
    resetSearch();
    loadPrefs(currentWorkerId);

    const handleWorkspaceChanged = (event: Event): void => {
      const detail = workspaceEventDetail(event);
      if (typeof detail === 'string' && detail !== currentWorkerId) return;
      void refreshTree(false);
      const query = searchQuery.trim() || committedQuery;
      if (query) void runSearch(query);
    };
    const handleFocus = (): void => {
      if (root !== null) void refreshTree(false);
      const query = searchQuery.trim() || committedQuery;
      if (query) void runSearch(query);
    };

    window.addEventListener(workspaceChangedEvent, handleWorkspaceChanged);
    window.addEventListener('focus', handleFocus);
    void loadRoot(currentWorkerId, generationAtRequest);
    void loadGitStatus(currentWorkerId, generationAtRequest);

    return () => {
      window.removeEventListener(workspaceChangedEvent, handleWorkspaceChanged);
      window.removeEventListener('focus', handleFocus);
      generation += 1;
      refreshPromise = null;
      if (searchTimer) {
        clearTimeout(searchTimer);
        searchTimer = null;
      }
      if (filterTimer) {
        clearTimeout(filterTimer);
        filterTimer = null;
      }
      filterSeq += 1;
      searchSeq += 1;
    };
  });

  // Typing in Names filters through the native file inventory; dotfile toggles re-run it.
  $effect(() => {
    void filterQuery;
    untrack(runNameFilter);
  });

  // Switching to Contents (for example Ctrl+Shift+F) puts the caret in its field.
  $effect(() => {
    if (view !== 'search') return;
    void tick().then(() => searchInput?.focus());
  });

  $effect(() => {
    if (!moreOpen) return;
    const close = (event: Event): void => {
      const target = event.target;
      if (target instanceof Node && (moreMenu?.contains(target) || moreButton?.contains(target))) return;
      moreOpen = false;
    };
    const key = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      moreOpen = false;
      moreButton?.focus();
    };
    window.addEventListener('pointerdown', close, true);
    window.addEventListener('keydown', key, true);
    return () => {
      window.removeEventListener('pointerdown', close, true);
      window.removeEventListener('keydown', key, true);
    };
  });
</script>

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
        canCollapseAll
          ? 'text-muted-foreground hover:bg-accent hover:text-foreground'
          : 'cursor-not-allowed text-muted-foreground/50'
      }`}
      aria-label="Collapse All"
      aria-disabled={!canCollapseAll}
      title="Collapse All"
      onclick={() => {
        if (canCollapseAll) collapseAll();
      }}
    >
      <ListCollapse size={12} />
    </button>
    <button
      type="button"
      class={`inline-flex size-7 shrink-0 items-center justify-center rounded-sm transition-colors ${
        view === 'files'
          ? 'text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-50'
          : 'cursor-not-allowed text-muted-foreground/50'
      }`}
      aria-label="Refresh Explorer"
      aria-disabled={view !== 'files'}
      title="Refresh Explorer"
      disabled={refreshing || rootLoading}
      onclick={() => {
        if (view === 'files') void refreshTree(true);
      }}
    >
      {#if refreshing || rootLoading}
        <Loader2 size={12} class="animate-spin" />
      {:else}
        <RefreshCw size={12} />
      {/if}
    </button>
    <div class="relative">
      <button
        bind:this={moreButton}
        type="button"
        class="inline-flex size-7 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        aria-label="More Explorer Actions"
        aria-haspopup="menu"
        aria-expanded={moreOpen}
        title="More Explorer Actions"
        onclick={() => {
          moreOpen = !moreOpen;
        }}
      >
        <Ellipsis size={12} />
      </button>
      {#if moreOpen}
        <div
          bind:this={moreMenu}
          role="menu"
          class="absolute right-0 top-full z-[70] mt-1 min-w-[12rem] rounded-[11px] border border-border/80 bg-popover p-1 text-popover-foreground shadow-[0_16px_34px_rgba(0,0,0,0.40)]"
        >
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={showDotfiles}
            class="relative flex w-full items-center rounded-md py-1 pl-8 pr-2.5 text-left text-[13px] leading-4 hover:bg-accent"
            onclick={toggleDotfiles}
          >
            <span class="absolute left-2.5 flex size-3.5 items-center justify-center">
              {#if showDotfiles}<Check size={14} />{/if}
            </span>
            Show Dotfiles
          </button>
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={showIgnored}
            class="relative flex w-full items-center rounded-md py-1 pl-8 pr-2.5 text-left text-[13px] leading-4 hover:bg-accent"
            onclick={toggleIgnored}
          >
            <span class="absolute left-2.5 flex size-3.5 items-center justify-center">
              {#if showIgnored}<Check size={14} />{/if}
            </span>
            Show Git Ignored Files
          </button>
          {#if onOpenEditor}
            <div class="-mx-1 my-1 h-px bg-border"></div>
            <button
              type="button"
              role="menuitem"
              class="flex w-full items-center gap-2 rounded-md px-2.5 py-1 text-left text-[13px] leading-4 hover:bg-accent"
              onclick={() => {
                moreOpen = false;
                onOpenEditor?.();
              }}
            >
              <ExternalLink size={14} />
              Open in VS Code
            </button>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  <div class="border-b border-border px-2 py-2">
    <div class="flex flex-col gap-2">
      <div class="rounded-md bg-muted p-0.5">
        <div role="group" aria-label="Explorer search mode" class="flex h-6 w-full items-center gap-1">
          <button
            type="button"
            aria-pressed={view === 'files'}
            aria-label="Filter files by name"
            class={`h-6 min-w-0 flex-1 rounded-md px-2 text-xs transition-colors ${
              view === 'files'
                ? 'bg-foreground/12 text-foreground dark:bg-accent'
                : 'text-muted-foreground hover:bg-foreground/6 hover:text-foreground dark:hover:bg-accent/50'
            }`}
            onpointerdown={(event) => {
              if (event.button === 0 && document.activeElement instanceof HTMLInputElement) event.preventDefault();
            }}
            onclick={() => selectView('files')}
          >
            Names
          </button>
          <button
            type="button"
            aria-pressed={view === 'search'}
            aria-label="Search file contents"
            class={`h-6 min-w-0 flex-1 rounded-md px-2 text-xs transition-colors ${
              view === 'search'
                ? 'bg-foreground/12 text-foreground dark:bg-accent'
                : 'text-muted-foreground hover:bg-foreground/6 hover:text-foreground dark:hover:bg-accent/50'
            }`}
            onpointerdown={(event) => {
              if (event.button === 0 && document.activeElement instanceof HTMLInputElement) event.preventDefault();
            }}
            onclick={() => selectView('search')}
          >
            Contents
          </button>
        </div>
      </div>
      <div class="relative min-h-8">
        <div hidden={view !== 'files'}>
          <div
            class="flex h-8 items-center gap-1 rounded-md border border-border bg-input/50 px-2 focus-within:border-ring focus-within:ring-1 focus-within:ring-ring"
          >
            <ListFilter size={14} class="shrink-0 text-muted-foreground" aria-hidden="true" />
            <input
              bind:this={filterInput}
              bind:value={filterQuery}
              onkeydown={handleFilterKey}
              data-explorer-query-input="files"
              type="text"
              spellcheck="false"
              class="min-w-0 flex-1 bg-transparent py-1 text-xs text-foreground outline-none placeholder:text-muted-foreground"
              aria-label="Find files"
              placeholder="Find files"
            />
            {#if filterLoading}
              <Loader2 size={12} class="shrink-0 animate-spin text-muted-foreground" aria-hidden="true" />
            {/if}
            {#if filterQuery}
              <button
                type="button"
                class="inline-flex shrink-0 items-center justify-center rounded-sm p-0.5 text-muted-foreground transition-colors hover:text-foreground"
                onclick={clearFilter}
                aria-label="Clear file filter"
                title="Clear file filter"
              >
                <X size={12} />
              </button>
            {/if}
          </div>
        </div>
        <div hidden={view !== 'search'}>
      <div
        class="flex h-8 items-center gap-1 rounded-md border border-border bg-input/50 px-2 focus-within:border-ring focus-within:ring-1 focus-within:ring-ring"
      >
        <Search size={14} class="shrink-0 text-muted-foreground" aria-hidden="true" />
        <input
          bind:this={searchInput}
          data-explorer-query-input="search"
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
      </div>
    </div>
  </div>

  <div class="relative min-h-0 flex-1 overflow-hidden">
    <div class={`absolute inset-0 flex min-h-0 flex-col ${view !== 'files' ? 'pointer-events-none invisible' : ''}`}>
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

        {#if hasNameFilter && filterError}
          <div class="m-2 rounded-md border border-destructive/30 bg-destructive/5 p-2 text-xs text-destructive" role="alert">{filterError}</div>
        {:else if hasNameFilter && filterTooLarge}
          <div class="px-3 py-6 text-xs text-muted-foreground">The file filter is too long.</div>
        {:else if hasNameFilter && filterLoading && filterFiles === null}
          <div class="flex h-32 items-center justify-center gap-2 text-xs text-muted-foreground" role="status">
            <Loader2 size={14} class="animate-spin" aria-hidden="true" />
            Searching files…
          </div>
        {:else if !hasNameFilter && rootLoading && !root}
          <div class="flex h-32 items-center justify-center gap-2 text-xs text-muted-foreground" role="status">
            <Loader2 size={14} class="animate-spin" aria-hidden="true" />
            Loading files…
          </div>
        {:else if !hasNameFilter && rootError && !root}
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
            {#if hasNameFilter}
              No files match.
            {:else}
              No files in this workspace.
            {/if}
          </div>
        {:else}
          <div class="py-1">
            {#each rows as row, index (row.path)}
              {@const FileIcon = fileIconFor(row.path)}
              {@const nodeStatus = statusByPath.get(row.path) ?? null}
              {@const ignored = !nodeStatus && isIgnoredPath(row.path)}
              {@const statusColor = nodeStatus ? `var(--git-decoration-${nodeStatus})` : null}
              <button
                type="button"
                role="treeitem"
                aria-level={row.depth + 1}
                aria-selected={selectedPath === row.path}
                aria-expanded={row.entry.is_dir ? row.expanded : undefined}
                data-explorer-index={index}
                data-git-status={nodeStatus ?? undefined}
                data-git-ignored={ignored ? '' : undefined}
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
                <span
                  class={`min-w-0 flex-1 truncate ${ignored ? 'pr-0.5 italic' : ''}`}
                  style:color={nodeStatus ? statusColor : ignored ? 'var(--git-decoration-ignored)' : undefined}
                >{row.entry.name}</span>
                {#if row.error}
                  <span class="max-w-[55%] shrink-0 truncate text-[10px] text-destructive" title={row.error}>
                    {row.error}
                  </span>
                {:else if row.truncated}
                  <span class="shrink-0 text-[10px] text-amber-600 dark:text-amber-400" title="Listing truncated by the workspace limit">truncated</span>
                {:else if nodeStatus}
                  <span class="mr-2 shrink-0 text-[10px] font-semibold tracking-wide" style:color={statusColor}>{STATUS_LABEL[nodeStatus]}</span>
                {:else if ignored}
                  <CircleSlash
                    size={12}
                    aria-label="Ignored by .gitignore"
                    class="mr-2 shrink-0"
                    style="color: var(--git-decoration-ignored)"
                  />
                {/if}
              </button>
            {/each}
          </div>
          {#if hasNameFilter ? filterTruncated : root?.truncated}
            <div class="border-t border-border px-2 py-1 text-[10px] text-muted-foreground">
              {hasNameFilter ? 'Showing the first matching files; refine the filter to narrow them.' : 'Listing truncated by the workspace limit.'}
            </div>
          {/if}
        {/if}
      </div>
    </div>
    <div class={`absolute inset-0 flex min-h-0 flex-col ${view !== 'search' ? 'pointer-events-none invisible' : ''}`}>
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
  </div>
</div>
