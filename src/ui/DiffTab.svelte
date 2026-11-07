<script lang="ts">
  import { onMount } from 'svelte';
  import type { Component } from 'svelte';
  import { diffWorker, errorMessage } from '../api';
  import { getWorkspaceFileDiff, workspaceChangedEvent } from '../workspace-api';
  import { workspaceEventDetail } from './workspace-events';
  import type { Worker } from '../types';

  type DiffViewProps = {
    text: string;
    worker: Worker;
    onClose: () => void;
    onRefresh: () => void;
    loading: boolean;
    error: string | null;
  };

  let { worker, path, staged, onClose }: {
    worker: Worker;
    /** One file's patch, or null for the whole worktree against its base. */
    path: string | null;
    staged: boolean;
    onClose: () => void;
  } = $props();

  let DiffView = $state<Component<DiffViewProps> | null>(null);
  let viewError = $state<string | null>(null);
  let text = $state('');
  let loading = $state(true);
  let error = $state<string | null>(null);
  let sequence = 0;

  async function load(): Promise<void> {
    const mine = ++sequence;
    loading = true;
    error = null;
    try {
      const result = path === null ? await diffWorker(worker.id) : await getWorkspaceFileDiff(worker.id, path, staged);
      if (mine === sequence) text = result;
    } catch (failure) {
      if (mine === sequence) error = errorMessage(failure);
    } finally {
      if (mine === sequence) loading = false;
    }
  }

  async function loadView(): Promise<void> {
    viewError = null;
    try {
      DiffView = (await import('./DiffView.svelte')).default;
    } catch (failure) {
      viewError = errorMessage(failure);
    }
  }

  onMount(() => {
    void loadView();
    void load();
    // A write, stage or commit announces a change; the patch is read again then.
    const changed = (event: Event): void => {
      const detail = workspaceEventDetail(event);
      if (typeof detail === 'string' && detail !== worker.id) return;
      void load();
    };
    window.addEventListener(workspaceChangedEvent, changed);
    return () => {
      sequence += 1;
      window.removeEventListener(workspaceChangedEvent, changed);
    };
  });
</script>

{#if DiffView}
  <DiffView {text} {worker} {onClose} onRefresh={() => { void load(); }} {loading} {error} />
{:else if viewError}
  <div class="m-auto max-w-lg space-y-3 p-6" role="alert">
    <h2 class="text-lg font-semibold">Diff view failed to load</h2>
    <p class="mn-error">{viewError}</p>
    <button type="button" class="mn-button" onclick={() => { void loadView(); }}>Retry</button>
  </div>
{:else}
  <div class="m-auto space-y-2 p-6 text-center" role="status">
    <h2 class="text-lg font-semibold">Loading diff view…</h2>
  </div>
{/if}
