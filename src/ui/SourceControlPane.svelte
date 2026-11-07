<script lang="ts">
  import { tick } from 'svelte';
  import {
    Check,
    ChevronDown,
    CircleAlert,
    GitBranch,
    ListFilter,
    Loader2,
    Minus,
    Plus,
    RefreshCw,
    X
  } from 'lucide-svelte';
  import type { Worker } from '../types';
  import {
    commitWorkspace,
    getWorkspaceGitStatus,
    getWorkspaceHistory,
    stageWorkspacePaths,
    unstageWorkspacePaths,
    workspaceChangedEvent,
    type GitCommit,
    type GitFileChange,
    type GitStatus
  } from '../workspace-api';
  import {
    baseName,
    describeGitStatus,
    dirName,
    fileIconFor
  } from './file-icons';
  import { workspaceEventDetail } from './workspace-events';
  import { errorMessage } from '../api';

  type SectionId = 'staged' | 'unstaged' | 'untracked';

  interface StatusSection {
    id: SectionId;
    label: string;
    staged: boolean;
    entries: GitFileChange[];
  }

  let {
    worker,
    onOpenDiff,
    onChanged,
    onNotify
  }: {
    worker: Worker;
    onOpenDiff: (path?: string, staged?: boolean) => void;
    onChanged: () => void;
    onNotify: (message: string, error?: boolean) => void;
  } = $props();

  const workerId = $derived(worker.id);

  let status = $state<GitStatus | null>(null);
  let history = $state<GitCommit[] | null>(null);
  let statusLoading = $state(false);
  let historyLoading = $state(false);
  let statusError = $state<string | null>(null);
  let historyError = $state<string | null>(null);
  let message = $state('');
  let actionBusy = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let actionNotice = $state<string | null>(null);
  let collapsedSections = $state<Set<string>>(new Set());
  let historyCollapsed = $state(false);
  let filterOpen = $state(false);
  let filterQuery = $state('');
  let filterInput = $state<HTMLInputElement | null>(null);

  let generation = 0;
  let statusSeq = 0;
  let historySeq = 0;

  const normalizedFilter = $derived(filterQuery.trim().toLowerCase());
  const branch = $derived(status?.branch || worker.branch);
  const stagedCount = $derived(status?.staged.length ?? 0);
  const totalChanges = $derived(
    status
      ? status.staged.length + status.unstaged.length + status.untracked.length
      : 0
  );
  const actionsLocked = $derived(actionBusy !== null || statusError !== null);

  const sections: StatusSection[] = $derived.by(() => {
    const current = status;
    if (!current) return [];
    const query = normalizedFilter;
    const matches = (entry: GitFileChange): boolean =>
      !query ||
      entry.path.toLowerCase().includes(query) ||
      (entry.old_path?.toLowerCase().includes(query) ?? false);
    return [
      {
        id: 'staged' as const,
        label: 'Staged Changes',
        staged: true,
        entries: current.staged.filter(matches)
      },
      {
        id: 'unstaged' as const,
        label: 'Changes',
        staged: false,
        entries: current.unstaged.filter(matches)
      },
      {
        id: 'untracked' as const,
        label: 'Untracked',
        staged: false,
        entries: current.untracked.filter(matches)
      }
    ].filter((section) => section.entries.length > 0);
  });

  function rawEntries(id: SectionId): GitFileChange[] {
    if (!status) return [];
    if (id === 'staged') return status.staged;
    if (id === 'unstaged') return status.unstaged;
    return status.untracked;
  }

  function toggleSection(id: string): void {
    const next = new Set(collapsedSections);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    collapsedSections = next;
  }

  function openEntry(entry: GitFileChange, staged: boolean): void {
    onOpenDiff(entry.path, staged);
  }

  function actionPaths(entry: GitFileChange, staged = false): string[] {
    return staged && entry.old_path && entry.old_path !== entry.path
      ? [entry.path, entry.old_path]
      : [entry.path];
  }

  function formatCommitDate(timestamp: number): string {
    if (!Number.isFinite(timestamp) || timestamp <= 0) return '';
    const milliseconds = timestamp > 1_000_000_000_000 ? timestamp : timestamp * 1000;
    const date = new Date(milliseconds);
    if (Number.isNaN(date.getTime())) return '';
    return new Intl.DateTimeFormat(undefined, {
      month: 'short',
      day: 'numeric'
    }).format(date);
  }

  async function loadStatus(
    requestedWorkerId: string,
    generationAtRequest: number,
    showSpinner: boolean
  ): Promise<void> {
    const sequence = ++statusSeq;
    if (showSpinner) statusLoading = true;
    try {
      const next = await getWorkspaceGitStatus(requestedWorkerId);
      if (
        sequence !== statusSeq ||
        generationAtRequest !== generation ||
        requestedWorkerId !== workerId
      ) {
        return;
      }
      status = next;
      statusError = null;
    } catch (error) {
      if (
        sequence !== statusSeq ||
        generationAtRequest !== generation ||
        requestedWorkerId !== workerId
      ) {
        return;
      }
      statusError = errorMessage(error);
    } finally {
      if (sequence === statusSeq && generationAtRequest === generation) {
        statusLoading = false;
      }
    }
  }

  async function loadHistory(
    requestedWorkerId: string,
    generationAtRequest: number
  ): Promise<void> {
    const sequence = ++historySeq;
    historyLoading = true;
    try {
      const next = await getWorkspaceHistory(requestedWorkerId, 30);
      if (
        sequence !== historySeq ||
        generationAtRequest !== generation ||
        requestedWorkerId !== workerId
      ) {
        return;
      }
      history = next;
      historyError = null;
    } catch (error) {
      if (sequence !== historySeq || generationAtRequest !== generation) return;
      historyError = errorMessage(error);
    } finally {
      if (sequence === historySeq && generationAtRequest === generation) {
        historyLoading = false;
      }
    }
  }

  function refreshAll(): void {
    void loadStatus(workerId, generation, true);
    void loadHistory(workerId, generation);
  }

  async function performAction(
    key: string,
    successMessage: string,
    failureLabel: string,
    action: () => Promise<void>
  ): Promise<boolean> {
    if (actionBusy || statusError) return false;
    const requestWorkerId = workerId;
    const generationAtRequest = generation;
    actionBusy = key;
    actionError = null;
    actionNotice = null;
    try {
      await action();
      if (generationAtRequest !== generation || requestWorkerId !== workerId) {
        return false;
      }
      actionNotice = successMessage;
      onNotify(successMessage, false);
      onChanged();
      await loadStatus(requestWorkerId, generationAtRequest, false);
      void loadHistory(requestWorkerId, generationAtRequest);
      return true;
    } catch (error) {
      if (generationAtRequest === generation && requestWorkerId === workerId) {
        const detail = `${failureLabel} failed: ${errorMessage(error)}`;
        actionError = detail;
        onNotify(detail, true);
      }
      return false;
    } finally {
      if (generationAtRequest === generation) actionBusy = null;
    }
  }

  function stageEntry(entry: GitFileChange): void {
    void performAction(
      `stage:${entry.path}`,
      `Staged ${baseName(entry.path)}`,
      'Stage',
      () => stageWorkspacePaths(workerId, actionPaths(entry))
    );
  }

  function unstageEntry(entry: GitFileChange): void {
    void performAction(
      `unstage:${entry.path}`,
      `Unstaged ${baseName(entry.path)}`,
      'Unstage',
      () => unstageWorkspacePaths(workerId, actionPaths(entry, true))
    );
  }

  function stageSection(section: StatusSection): void {
    const entries = rawEntries(section.id);
    const paths = entries.flatMap(entry => actionPaths(entry));
    if (!paths.length) return;
    void performAction(
      `stage-all:${section.id}`,
      `Staged ${entries.length} ${entries.length === 1 ? 'file' : 'files'}`,
      'Stage all',
      () => stageWorkspacePaths(workerId, paths)
    );
  }

  function unstageSection(section: StatusSection): void {
    const entries = rawEntries(section.id);
    const paths = entries.flatMap(entry => actionPaths(entry, true));
    if (!paths.length) return;
    void performAction(
      `unstage-all:${section.id}`,
      `Unstaged ${entries.length} ${entries.length === 1 ? 'file' : 'files'}`,
      'Unstage all',
      () => unstageWorkspacePaths(workerId, paths)
    );
  }

  async function commitChanges(): Promise<void> {
    const text = message.trim();
    if (!status || !text || stagedCount === 0 || actionBusy || statusError) return;
    const requestWorkerId = workerId;
    const generationAtRequest = generation;
    actionBusy = 'commit';
    actionError = null;
    actionNotice = null;
    try {
      const commitId = await commitWorkspace(requestWorkerId, text);
      if (generationAtRequest !== generation || requestWorkerId !== workerId) {
        return;
      }
      const shortId = commitId.trim().slice(0, 7);
      const notice = shortId ? `Committed ${shortId}` : 'Commit created';
      actionNotice = notice;
      onNotify(notice, false);
      message = '';
      onChanged();
      await loadStatus(requestWorkerId, generationAtRequest, false);
      void loadHistory(requestWorkerId, generationAtRequest);
    } catch (error) {
      if (generationAtRequest === generation && requestWorkerId === workerId) {
        const detail = `Commit failed: ${errorMessage(error)}`;
        actionError = detail;
        onNotify(detail, true);
      }
    } finally {
      if (generationAtRequest === generation) actionBusy = null;
    }
  }

  async function toggleFilter(): Promise<void> {
    filterOpen = !filterOpen;
    if (!filterOpen) {
      filterQuery = '';
      return;
    }
    await tick();
    filterInput?.focus();
    filterInput?.select();
  }

  function closeFilter(): void {
    filterOpen = false;
    filterQuery = '';
  }

  function handleFilterKey(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key !== 'Escape') return;
    event.preventDefault();
    closeFilter();
  }

  $effect(() => {
    const currentWorkerId = workerId;
    generation += 1;
    const generationAtRequest = generation;
    status = null;
    history = null;
    statusLoading = false;
    historyLoading = false;
    statusError = null;
    historyError = null;
    message = '';
    actionBusy = null;
    actionError = null;
    actionNotice = null;
    collapsedSections = new Set();
    historyCollapsed = false;
    filterOpen = false;
    filterQuery = '';

    const handleWorkspaceChanged = (event: Event): void => {
      const detail = workspaceEventDetail(event);
      if (typeof detail === 'string' && detail !== currentWorkerId) return;
      refreshAll();
    };
    const handleFocus = (): void => {
      refreshAll();
    };

    window.addEventListener(workspaceChangedEvent, handleWorkspaceChanged);
    window.addEventListener('focus', handleFocus);
    void loadStatus(currentWorkerId, generationAtRequest, true);
    void loadHistory(currentWorkerId, generationAtRequest);

    return () => {
      window.removeEventListener(workspaceChangedEvent, handleWorkspaceChanged);
      window.removeEventListener('focus', handleFocus);
      generation += 1;
    };
  });
</script>

<div class="flex h-full min-h-0 w-full flex-col overflow-hidden">
  <div class="flex h-8 min-h-8 items-center gap-1.5 border-b border-border px-2">
    <GitBranch size={12} class="shrink-0 text-muted-foreground" aria-hidden="true" />
    <span class="min-w-0 flex-1 truncate text-xs font-medium text-foreground" title={branch}>
      {branch}
    </span>
    {#if totalChanges > 0}
      <span class="shrink-0 text-[10px] tabular-nums text-muted-foreground" title={`${totalChanges} changed files`}>
        {totalChanges}
      </span>
    {/if}
    <button
      type="button"
      class={`inline-flex size-7 shrink-0 items-center justify-center rounded-sm transition-colors ${
        filterOpen
          ? 'bg-accent text-foreground'
          : 'text-muted-foreground hover:bg-accent hover:text-foreground'
      }`}
      aria-label="Filter changes"
      aria-pressed={filterOpen}
      title="Filter changes"
      onclick={() => void toggleFilter()}
    >
      <ListFilter size={12} />
    </button>
    <button
      type="button"
      class="inline-flex size-7 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
      aria-label="Refresh changes"
      title="Refresh changes"
      disabled={statusLoading}
      onclick={refreshAll}
    >
      {#if statusLoading}
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
          aria-label="Filter changed files"
          placeholder="Filter files"
        />
        {#if filterQuery}
          <button
            type="button"
            class="inline-flex shrink-0 items-center justify-center rounded-sm p-0.5 text-muted-foreground transition-colors hover:text-foreground"
            onclick={() => (filterQuery = '')}
            aria-label="Clear change filter"
            title="Clear change filter"
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

  <div class="shrink-0 border-b border-border px-3 pb-2 pt-1.5">
    <textarea
      bind:value={message}
      rows="2"
      aria-label="Commit message"
      placeholder="Message"
      class="mt-0.5 min-h-14 w-full resize-none appearance-none rounded-md border border-input bg-background px-2 py-1.5 text-xs text-foreground shadow-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60 dark:bg-input/30"
      disabled={!status || actionBusy !== null}
    ></textarea>
    <button
      type="button"
      class="mt-1 flex h-6 w-full items-center justify-center gap-1 rounded-md bg-primary px-2 text-xs text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
      disabled={!status || statusError !== null || stagedCount === 0 || message.trim().length === 0 || actionBusy !== null}
      title={
        !status
          ? 'Git status is unavailable'
          : statusError !== null
            ? 'Refresh Git status before committing'
            : stagedCount === 0
            ? 'Stage files before committing'
            : message.trim().length === 0
              ? 'Enter a commit message'
              : 'Commit staged changes'
      }
      onclick={() => void commitChanges()}
    >
      {#if actionBusy === 'commit'}
        <Loader2 size={12} class="animate-spin" aria-hidden="true" />
        Committing…
      {:else}
        <Check size={12} aria-hidden="true" />
        Commit
      {/if}
    </button>
    {#if status && stagedCount === 0 && totalChanges > 0}
      <p class="mt-1 text-[10px] text-muted-foreground">Stage changes to commit.</p>
    {:else if status && totalChanges === 0}
      <p class="mt-1 text-[10px] text-muted-foreground">No changes to commit.</p>
    {/if}
    {#if actionError}
      <div class="mt-1.5 rounded-md border border-destructive/30 bg-destructive/5 p-1.5" role="alert">
        <div class="flex items-start gap-1.5 text-[11px] text-destructive">
          <CircleAlert size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
          <p class="min-w-0 flex-1 break-words">{actionError}</p>
          <button
            type="button"
            class="shrink-0 text-muted-foreground hover:text-foreground"
            aria-label="Dismiss error"
            onclick={() => (actionError = null)}
          >
            <X size={12} />
          </button>
        </div>
      </div>
    {:else if actionNotice}
      <div class="mt-1.5 flex items-center gap-1.5 text-[11px] text-muted-foreground" role="status">
        <Check size={12} aria-hidden="true" />
        <span class="min-w-0 flex-1 break-words">{actionNotice}</span>
      </div>
    {/if}
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto scrollbar-sleek">
    {#if !status && !statusError}
      <div class="flex h-24 items-center justify-center gap-2 text-xs text-muted-foreground" role="status">
        <Loader2 size={14} class="animate-spin" aria-hidden="true" />
        Loading changes…
      </div>
    {:else if !status && statusError}
      <div class="m-3 rounded-md border border-destructive/30 bg-destructive/5 p-3" role="alert">
        <div class="flex items-start gap-2 text-xs text-destructive">
          <CircleAlert size={14} class="mt-0.5 shrink-0" aria-hidden="true" />
          <p class="min-w-0 flex-1 break-words">{statusError}</p>
        </div>
        <button
          type="button"
          class="mt-2 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
          onclick={refreshAll}
        >
          Retry
        </button>
      </div>
    {:else}
      {#if statusError}
        <div class="m-2 flex items-start gap-1.5 rounded-md border border-destructive/30 bg-destructive/5 p-2 text-[11px] text-destructive" role="alert">
          <CircleAlert size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
          <p class="min-w-0 flex-1 break-words">{statusError}</p>
          <button
            type="button"
            class="shrink-0 text-muted-foreground hover:text-foreground"
            aria-label="Retry loading changes"
            onclick={refreshAll}
          >
            <RefreshCw size={12} />
          </button>
        </div>
      {/if}

      {#if status && totalChanges === 0}
        <div class="px-4 py-6">
          <div class="text-sm font-medium text-foreground">No changes</div>
          <div class="mt-1 text-xs text-muted-foreground">
            This worktree matches its base commit.
          </div>
        </div>
      {:else if status && sections.length === 0}
        <div class="px-4 py-6">
          <div class="text-sm font-medium text-foreground">No matching files</div>
          <div class="mt-1 text-xs text-muted-foreground">
            No changed file matches “{filterQuery.trim()}”.
          </div>
          <button
            type="button"
            class="mt-3 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
            onclick={closeFilter}
          >
            Clear filter
          </button>
        </div>
      {:else if status}
        {#each sections as section (section.id)}
          <div class="pl-1 pr-3 pt-3 pb-1">
            <div class="group/section flex items-center gap-x-1 rounded-md pr-1 hover:bg-accent hover:text-accent-foreground">
              <button
                type="button"
                class="flex h-auto min-h-6 min-w-0 flex-1 items-center justify-start gap-x-1 py-0.5 text-left text-[11px] font-semibold uppercase tracking-wider text-foreground/70 group-hover/section:text-accent-foreground"
                onclick={() => toggleSection(section.id)}
                aria-expanded={!collapsedSections.has(section.id)}
              >
                <ChevronDown
                  size={14}
                  class={`shrink-0 transition-transform ${
                    collapsedSections.has(section.id) ? '-rotate-90' : ''
                  }`}
                />
                <span class="min-w-0 flex-1">
                  <span class="flex min-w-0 items-center gap-1">
                    <span class="min-w-0 truncate" title={section.label}>{section.label}</span>
                    <span class="shrink-0 text-[11px] font-medium tabular-nums">
                      {section.entries.length}
                    </span>
                  </span>
                </span>
              </button>
              <div class="ml-auto flex shrink-0 items-center justify-end">
                {#if !normalizedFilter}
                  {#if section.staged}
                    <button
                      type="button"
                      class="inline-flex size-6 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-background/70 hover:text-foreground disabled:opacity-50"
                      aria-label={`Unstage all in ${section.label}`}
                      title={`Unstage all in ${section.label}`}
                      disabled={actionsLocked}
                      onclick={() => unstageSection(section)}
                    >
                      {#if actionBusy === `unstage-all:${section.id}`}
                        <Loader2 size={12} class="animate-spin" />
                      {:else}
                        <Minus size={12} />
                      {/if}
                    </button>
                  {:else}
                    <button
                      type="button"
                      class="inline-flex size-6 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-background/70 hover:text-foreground disabled:opacity-50"
                      aria-label={`Stage all in ${section.label}`}
                      title={`Stage all in ${section.label}`}
                      disabled={actionsLocked}
                      onclick={() => stageSection(section)}
                    >
                      {#if actionBusy === `stage-all:${section.id}`}
                        <Loader2 size={12} class="animate-spin" />
                      {:else}
                        <Plus size={12} />
                      {/if}
                    </button>
                  {/if}
                {/if}
              </div>
            </div>
          </div>

          {#if !collapsedSections.has(section.id)}
            {#each section.entries as entry (section.id + ':' + entry.path)}
              {@const badge = describeGitStatus(entry.status)}
              {@const FileIcon = fileIconFor(entry.path)}
              <div
                role="button"
                tabindex="0"
                class="group relative flex cursor-pointer items-center gap-1 py-1 pr-3 transition-colors hover:bg-accent/40 focus-visible:bg-accent/40 focus-visible:outline-none"
                style:padding-left="20px"
                title={`Open diff: ${entry.path}`}
                onclick={(event) => {
                  if (event.detail > 1) return;
                  openEntry(entry, section.staged);
                }}
                onkeydown={(event) => {
                  if (event.target !== event.currentTarget) return;
                  if (event.key !== 'Enter' && event.key !== ' ') return;
                  event.preventDefault();
                  openEntry(entry, section.staged);
                }}
              >
                <FileIcon size={14} class="shrink-0" color={badge.color} />
                <span class="min-w-0 flex-1 text-xs">
                  <span class="block min-w-0 truncate">
                    <span class="text-foreground">{baseName(entry.path)}</span>
                    {#if dirName(entry.path)}
                      <span class="ml-1.5 text-[11px] text-muted-foreground">
                        {dirName(entry.path)}
                      </span>
                    {/if}
                  </span>
                  {#if entry.old_path}
                    <span class="block truncate text-[11px] text-muted-foreground">
                      from {entry.old_path}
                    </span>
                  {/if}
                </span>
                <span
                  class="w-4 shrink-0 text-center text-[10px] font-bold"
                  style:color={badge.color}
                  title={badge.label}
                >
                  {badge.letter}
                </span>
                <div class="absolute bottom-0 right-0 top-0 flex items-center gap-1.5 bg-accent pl-2 pr-3 opacity-0 transition-opacity pointer-events-none group-hover:pointer-events-auto group-hover:opacity-100 focus-within:pointer-events-auto focus-within:opacity-100">
                  {#if section.staged}
                    <button
                      type="button"
                      class="inline-flex size-6 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-background/70 hover:text-foreground disabled:opacity-50"
                      aria-label={`Unstage ${entry.path}`}
                      title={`Unstage ${entry.path}`}
                      disabled={actionsLocked}
                      onclick={(event) => {
                        event.stopPropagation();
                        unstageEntry(entry);
                      }}
                    >
                      {#if actionBusy === `unstage:${entry.path}`}
                        <Loader2 size={12} class="animate-spin" />
                      {:else}
                        <Minus size={12} />
                      {/if}
                    </button>
                  {:else}
                    <button
                      type="button"
                      class="inline-flex size-6 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-background/70 hover:text-foreground disabled:opacity-50"
                      aria-label={`Stage ${entry.path}`}
                      title={`Stage ${entry.path}`}
                      disabled={actionsLocked}
                      onclick={(event) => {
                        event.stopPropagation();
                        stageEntry(entry);
                      }}
                    >
                      {#if actionBusy === `stage:${entry.path}`}
                        <Loader2 size={12} class="animate-spin" />
                      {:else}
                        <Plus size={12} />
                      {/if}
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          {/if}
        {/each}
      {/if}
    {/if}

    <div class="pl-1 pr-3 pt-3 pb-1">
      <div class="group/section flex items-center gap-x-1 rounded-md pr-1 hover:bg-accent hover:text-accent-foreground">
        <button
          type="button"
          class="flex h-auto min-h-6 min-w-0 flex-1 items-center justify-start gap-x-1 py-0.5 text-left text-[11px] font-semibold uppercase tracking-wider text-foreground/70 group-hover/section:text-accent-foreground"
          onclick={() => (historyCollapsed = !historyCollapsed)}
          aria-expanded={!historyCollapsed}
        >
          <ChevronDown
            size={14}
            class={`shrink-0 transition-transform ${historyCollapsed ? '-rotate-90' : ''}`}
          />
          <span class="min-w-0 flex-1">
            <span class="flex min-w-0 items-center gap-1">
              <span class="min-w-0 truncate">History</span>
              <span class="shrink-0 text-[11px] font-medium tabular-nums">
                {history?.length ?? 0}
              </span>
            </span>
          </span>
        </button>
        <div class="ml-auto flex shrink-0 items-center justify-end">
          <button
            type="button"
            class="inline-flex size-6 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-background/70 hover:text-foreground disabled:opacity-50"
            aria-label="Refresh history"
            title="Refresh history"
            disabled={historyLoading}
            onclick={() => void loadHistory(workerId, generation)}
          >
            {#if historyLoading}
              <Loader2 size={12} class="animate-spin" />
            {:else}
              <RefreshCw size={12} />
            {/if}
          </button>
        </div>
      </div>
    </div>

    {#if !historyCollapsed}
      {#if historyLoading && !history}
        <div class="flex h-16 items-center justify-center gap-2 text-xs text-muted-foreground" role="status">
          <Loader2 size={14} class="animate-spin" aria-hidden="true" />
          Loading history…
        </div>
      {:else if historyError && !history}
        <div class="mx-3 mb-3 rounded-md border border-destructive/30 bg-destructive/5 p-2" role="alert">
          <div class="flex items-start gap-1.5 text-[11px] text-destructive">
            <CircleAlert size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
            <p class="min-w-0 flex-1 break-words">{historyError}</p>
          </div>
          <button
            type="button"
            class="mt-2 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
            onclick={() => void loadHistory(workerId, generation)}
          >
            Retry
          </button>
        </div>
      {:else if history && history.length === 0}
        <div class="px-3 pb-4 text-xs text-muted-foreground">No commits yet.</div>
      {:else if history}
        {#if historyError}
          <div class="mx-3 mb-2 flex items-start gap-1.5 rounded-md border border-destructive/30 bg-destructive/5 p-2 text-[11px] text-destructive" role="alert">
            <CircleAlert size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
            <p class="min-w-0 flex-1 break-words">{historyError}</p>
          </div>
        {/if}
        <div class="pb-4">
          {#each history as commit (commit.id)}
            <div class="flex flex-col gap-0.5 px-3 py-1" title={`${commit.id} — ${commit.author}`}>
              <div class="flex min-w-0 items-baseline gap-1.5">
                <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
                  {commit.short_id}
                </span>
                <span class="min-w-0 flex-1 truncate text-xs text-foreground">
                  {commit.subject}
                </span>
              </div>
              <div class="truncate text-[10px] text-muted-foreground">
                {commit.author}
                {#if formatCommitDate(commit.timestamp)}
                  <span aria-hidden="true"> · </span>{formatCommitDate(commit.timestamp)}
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>
