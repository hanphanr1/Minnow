<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { Search } from 'lucide-svelte';
  import type { Worker } from '../types';
  import { findWorkspaceFiles } from '../workspace-api';
  import { errorMessage } from '../api';
  import { baseName, dirName, fileIconFor } from './file-icons';
  import {
    parseQuickOpenTarget,
    prepareQuickOpenFiles,
    rankQuickOpenFiles,
    type QuickOpenFile
  } from './quick-open-search';

  let { worker, onOpen, onClose }: {
    worker: Worker;
    onOpen: (path: string, line?: number) => void;
    onClose: () => void;
  } = $props();

  // A palette belongs to the worktree it was opened for.
  const workerId = untrack(() => worker.id);
  const recentKey = `minnow.quickopen.recent.${workerId}`;

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);
  let query = $state('');
  let files = $state<QuickOpenFile[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let truncated = $state(false);
  let selected = $state(0);
  let recents = $state<string[]>(readRecents());

  function readRecents(): string[] {
    try {
      const value = JSON.parse(localStorage.getItem(recentKey) ?? '[]');
      return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
    } catch {
      return [];
    }
  }

  const target = $derived(parseQuickOpenTarget(query));
  const results = $derived.by(() => {
    if (!target.pathQuery) {
      // Orca lists recent files first when nothing is typed.
      const known = new Set(files.map((file) => file.path));
      const first = recents.filter((path) => known.has(path));
      const rest = files.map((file) => file.path).filter((path) => !first.includes(path));
      return [...first, ...rest].slice(0, 50);
    }
    return rankQuickOpenFiles(target.pathQuery, files).map((result) => result.path);
  });
  const activeIndex = $derived(results.length === 0 ? -1 : Math.min(selected, results.length - 1));

  function choose(path: string): void {
    try {
      localStorage.setItem(recentKey, JSON.stringify([path, ...recents.filter((item) => item !== path)].slice(0, 20)));
    } catch {
      // Recent files are a convenience.
    }
    onOpen(path, target.line);
    onClose();
  }

  function move(delta: number): void {
    if (results.length === 0) return;
    selected = (activeIndex + delta + results.length) % results.length;
    void tick().then(() => listEl?.querySelector<HTMLElement>('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' }));
  }

  function handleKey(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key === 'ArrowDown') { event.preventDefault(); move(1); }
    else if (event.key === 'ArrowUp') { event.preventDefault(); move(-1); }
    else if (event.key === 'Enter') {
      event.preventDefault();
      const path = results[activeIndex];
      if (path) choose(path);
    }
  }

  onMount(() => {
    dialogEl?.showModal();
    void tick().then(() => inputEl?.focus());
    let cancelled = false;
    findWorkspaceFiles(workerId, '', true)
      .then((list) => {
        if (cancelled) return;
        files = prepareQuickOpenFiles(list.paths);
        truncated = list.truncated;
      })
      .catch((error) => {
        if (!cancelled) loadError = errorMessage(error);
      })
      .finally(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
      if (dialogEl?.open) dialogEl.close();
    };
  });
</script>

<!-- Geometry and footer follow Orca's QuickOpen command dialog (MIT). -->
<dialog
  bind:this={dialogEl}
  class="fixed top-[min(10%,4rem)] bottom-auto left-1/2 z-50 m-0 max-h-[min(90vh,calc(100vh-1.5rem))] w-[640px] max-w-[96vw] -translate-x-1/2 overflow-hidden rounded-xl border border-border/70 bg-dialog-surface p-0 text-foreground shadow-[0_26px_84px_rgba(0,0,0,0.45)] backdrop:bg-black/40"
  aria-label="Go to file"
  onclose={onClose}
  onclick={(event) => { if (event.target === dialogEl) onClose(); }}
>
  <div class="flex max-h-[min(90vh,calc(100vh-1.5rem))] min-h-0 flex-col overflow-hidden rounded-md">
    <div class="mx-3 mt-3 flex shrink-0 items-center rounded-lg border border-border/55 bg-muted/28 px-3.5 py-1">
      <Search class="mr-2.5 size-4 shrink-0 text-muted-foreground/60" aria-hidden="true" />
      <input
        bind:this={inputEl}
        bind:value={query}
        oninput={() => { selected = 0; }}
        onkeydown={handleKey}
        class="flex h-9 min-w-0 flex-1 bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground"
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls="quick-open-list"
        aria-label="Go to file"
        placeholder="Go to file..."
        autocomplete="off"
        spellcheck="false"
      />
    </div>
    <div
      bind:this={listEl}
      id="quick-open-list"
      role="listbox"
      aria-label="Files"
      class="min-h-0 max-h-[min(420px,calc(100vh-14rem))] overflow-y-auto p-2 scrollbar-sleek"
    >
      {#if loading}
        <div class="py-6 text-center text-sm text-muted-foreground">Loading files...</div>
      {:else if loadError}
        <div class="whitespace-pre-wrap px-4 py-6 text-center text-sm text-muted-foreground">{loadError}</div>
      {:else if results.length === 0}
        <div class="py-6 text-center text-sm text-muted-foreground">No matching files.</div>
      {:else}
        {#each results as path, index (path)}
          {@const Icon = fileIconFor(path)}
          <div
            role="option"
            aria-selected={index === activeIndex}
            tabindex="-1"
            class={`flex w-full min-w-0 cursor-default items-center gap-2 rounded-md px-3 py-1 text-sm ${index === activeIndex ? 'bg-accent text-accent-foreground' : ''}`}
            title={path}
            onpointermove={() => { selected = index; }}
            onclick={() => choose(path)}
            onkeydown={() => {}}
          >
            <Icon size={14} class="shrink-0 text-muted-foreground" />
            <span class="min-w-0 max-w-full shrink-0 truncate text-foreground">{baseName(path)}</span>
            {#if dirName(path)}<span class="min-w-0 truncate text-muted-foreground">{dirName(path)}</span>{/if}
          </div>
        {/each}
      {/if}
      {#if truncated && !loading && !loadError}
        <div class="px-3 py-2 text-center text-xs text-muted-foreground">More matches may be available. Refine your search to narrow the results.</div>
      {/if}
    </div>
    <div class="flex shrink-0 items-center justify-end border-t border-border/60 px-3.5 py-2.5 text-[11px] text-muted-foreground/82">
      <div class="flex items-center gap-2">
        <span class="rounded-full border border-border/60 bg-muted/35 px-2 py-0.5 text-[10px] font-medium text-foreground/85">Enter</span><span>Open</span>
        <span class="rounded-full border border-border/60 bg-muted/35 px-2 py-0.5 text-[10px] font-medium text-foreground/85">Esc</span><span>Close</span>
        <span class="rounded-full border border-border/60 bg-muted/35 px-2 py-0.5 text-[10px] font-medium text-foreground/85">↑↓</span><span>Move</span>
      </div>
    </div>
    <div aria-live="polite" class="sr-only">{query.trim() ? `${results.length} files found` : ''}</div>
  </div>
</dialog>
