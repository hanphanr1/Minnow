<script lang="ts">
  // Orca right-sidebar PortsPanel (MIT), local part only: TCP ports that a process started
  // from this worktree's terminals is listening on, read from the system socket table.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { ExternalLink, Loader2, Network, RefreshCw } from 'lucide-svelte';
  import type { Worker } from '../types';
  import { errorMessage } from '../api';

  interface ListeningPort { port: number; address: string; pid: number; terminal_id: string; terminal_name: string }

  let { worker }: { worker: Worker } = $props();

  let ports = $state<ListeningPort[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let sequence = 0;

  async function refresh(): Promise<void> {
    const mine = ++sequence;
    const workerId = worker.id;
    loading = true;
    try {
      const list = await invoke<ListeningPort[]>('workspace_ports', { workerId });
      if (mine === sequence) { ports = list; error = null; }
    } catch (failure) {
      if (mine === sequence) error = errorMessage(failure);
    } finally {
      if (mine === sequence) loading = false;
    }
  }

  function host(address: string): string {
    return address === '0.0.0.0' || address === '::' || address === '127.0.0.1' ? 'localhost' : address === '::1' ? '[::1]' : address;
  }

  async function open(port: ListeningPort): Promise<void> {
    try {
      await invoke('open_local_url', { url: `http://${host(port.address) === '[::1]' ? '[::1]' : 'localhost'}:${port.port}` });
    } catch (failure) {
      error = errorMessage(failure);
    }
  }

  $effect(() => {
    void worker.id;
    ports = [];
    void refresh();
  });

  onMount(() => {
    // Servers start and stop on their own; a light poll keeps the list truthful while visible.
    const timer = setInterval(() => void refresh(), 5000);
    return () => clearInterval(timer);
  });
</script>

<div class="flex min-h-0 flex-1 flex-col">
  <div class="flex h-8 min-h-8 items-center gap-2 border-b border-border px-2">
    <span class="flex-1 truncate text-xs font-medium">Ports</span>
    <button type="button" class="inline-flex size-7 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent hover:text-foreground" aria-label="Refresh ports" title="Refresh ports" onclick={() => void refresh()}>
      {#if loading}<Loader2 size={12} class="animate-spin" />{:else}<RefreshCw size={12} />{/if}
    </button>
  </div>
  {#if error}
    <p class="m-2 text-xs text-destructive" role="alert">{error}</p>
  {/if}
  {#if ports.length === 0}
    <div class="flex flex-1 flex-col items-center justify-center px-4 text-center">
      <Network size={18} class="text-muted-foreground/60" aria-hidden="true" />
      <div class="mt-2 text-sm font-medium">No forwarded ports</div>
      <p class="mt-1 text-xs text-muted-foreground">Servers started from this workspace's terminals appear here while they listen.</p>
    </div>
  {:else}
    <ul class="min-h-0 flex-1 overflow-y-auto py-1" aria-label="Listening ports">
      {#each ports as port (`${port.pid}:${port.port}:${port.address}`)}
        <li class="group flex items-center gap-2 px-3 py-1.5 hover:bg-accent/40">
          <span class="w-14 shrink-0 font-mono text-xs tabular-nums">{port.port}</span>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-xs">{port.terminal_name}</span>
            <span class="block truncate text-[11px] text-muted-foreground">{port.address} · PID {port.pid}</span>
          </span>
          <button type="button" class="inline-flex size-6 items-center justify-center rounded-sm text-muted-foreground hover:bg-background/70 hover:text-foreground" aria-label={`Open port ${port.port} in browser`} title="Open in browser" onclick={() => void open(port)}>
            <ExternalLink size={12} />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
