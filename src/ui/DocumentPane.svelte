<script lang="ts">
  import { getContext, onMount } from 'svelte';
  import type { Component } from 'svelte';
  import { errorMessage } from '../api';
  import DiffTab from './DiffTab.svelte';
  import {
    WS_API,
    WS_VIEW,
    type WorkspaceApi,
    type WorkspaceTab,
    type WorkspaceView,
  } from './workspace-layout';

  type FileEditorProps = {
    worker: WorkspaceView['worker'];
    path: string;
    line?: number;
    rightSidebarVisible?: boolean;
    onClose: () => void;
    onDirtyChange?: (dirty: boolean) => void;
  };

  let { tab }: { tab: WorkspaceTab } = $props();

  const view = getContext<WorkspaceView>(WS_VIEW);
  const api = getContext<WorkspaceApi>(WS_API);

  let FileEditor = $state<Component<FileEditorProps, { requestClose(): Promise<boolean> }> | null>(null);
  let loadError = $state<string | null>(null);
  let editor = $state<{ requestClose(): Promise<boolean> }>();

  async function loadEditor(): Promise<void> {
    loadError = null;
    try {
      FileEditor = (await import('./FileEditor.svelte')).default;
    } catch (failure) {
      loadError = errorMessage(failure);
    }
  }

  onMount(() => {
    if (tab.document?.kind === 'file') void loadEditor();
  });

  $effect(() => {
    const id = tab.id;
    const handle = editor ?? null;
    api.registerEditor(id, handle);
    return () => api.registerEditor(id, null);
  });
</script>

{#if tab.document?.kind === 'file'}
  {#if FileEditor}
    <FileEditor
      bind:this={editor}
      worker={view.worker}
      path={tab.document.path}
      line={tab.document.line}
      rightSidebarVisible={true}
      onClose={() => api.closeTab(tab.id)}
      onDirtyChange={(dirty) => api.setTabDirty(tab.id, dirty)}
    />
  {:else if loadError}
    <div class="m-auto max-w-lg space-y-3 p-6" role="alert">
      <h2 class="text-lg font-semibold">Editor failed to load</h2>
      <p class="mn-error">{loadError}</p>
      <button type="button" class="mn-button" onclick={() => { void loadEditor(); }}>Retry</button>
    </div>
  {:else}
    <div class="m-auto p-6 text-sm text-muted-foreground" role="status">Loading file editor…</div>
  {/if}
{:else if tab.document?.kind === 'diff'}
  <DiffTab
    worker={view.worker}
    path={tab.document.path}
    staged={tab.document.staged}
    onClose={() => api.closeTab(tab.id)}
  />
{/if}
