<script lang="ts">
  // Activity feed after Orca components/activity (MIT): real agent attention events,
  // newest first, filterable, each one jumps to its workspace and terminal.
  import { Bell, BellOff, CheckCheck, CircleHelp, CircleStop, CircleCheck, Trash2 , Settings2 } from 'lucide-svelte';
  import { activity, clearActivity, describeActivity, markAllRead, markRead, type ActivityEvent } from './activity.svelte';
  import type { Worker } from '../types';

  let {
    workers,
    onOpen,
    onSettings
  }: {
    workers: Worker[];
    onOpen: (event: ActivityEvent) => void;
    onSettings: () => void;
  } = $props();

  let filter = $state<'all' | 'unread' | 'needs-input'>('all');
  let query = $state('');

  const shown = $derived(
    activity.events.filter((event) => {
      if (filter === 'unread' && event.read) return false;
      if (filter === 'needs-input' && event.kind !== 'needs-input') return false;
      const text = `${event.workerName} ${event.terminalName} ${describeActivity(event.kind)}`.toLowerCase();
      return !query.trim() || text.includes(query.trim().toLowerCase());
    })
  );

  const groups = $derived.by(() => {
    const byDay = new Map<string, ActivityEvent[]>();
    for (const event of shown) {
      const day = new Date(event.at).toLocaleDateString(undefined, { weekday: 'long', month: 'short', day: 'numeric' });
      byDay.set(day, [...(byDay.get(day) ?? []), event]);
    }
    return [...byDay.entries()];
  });

  function time(at: number): string {
    return new Date(at).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  }

  function exists(event: ActivityEvent): boolean {
    return workers.some((worker) => worker.id === event.workerId);
  }
</script>

<div class="flex min-h-0 flex-1 flex-col">
  <header class="flex items-center gap-3 border-b border-border px-6 py-4">
    <Bell size={18} class="text-muted-foreground" />
    <h1 class="text-lg font-semibold">Activity</h1>
    <span class="flex-1"></span>
    <input bind:value={query} class="h-8 w-56 rounded-md border border-input bg-background px-2 text-sm" placeholder="Filter activity" aria-label="Filter activity" />
    <div class="flex rounded-md bg-muted p-0.5 text-xs" role="group" aria-label="Show">
      {#each [['all', 'All'], ['unread', 'Unread'], ['needs-input', 'Needs input']] as [id, label] (id)}
        <button type="button" aria-pressed={filter === id} class={`h-6 rounded-md px-2 ${filter === id ? 'bg-foreground/12 text-foreground dark:bg-accent' : 'text-muted-foreground'}`} onclick={() => { filter = id as typeof filter; }}>{label}</button>
      {/each}
    </div>
    <button type="button" class="mn-button" onclick={markAllRead} disabled={!activity.events.some((event) => !event.read)}><CheckCheck size={14} /> Mark all read</button>
    <button type="button" class="mn-icon-button" aria-label="Clear activity" title="Clear activity" onclick={clearActivity} disabled={activity.events.length === 0}><Trash2 size={14} /></button>
    <button type="button" class="mn-icon-button" aria-label="Notification settings" title="Notification settings" onclick={onSettings}><Settings2 size={14} /></button>
  </header>
  <div class="min-h-0 flex-1 overflow-y-auto px-6 py-4 scrollbar-sleek">
    {#if shown.length === 0}
      <div class="m-auto mt-16 max-w-md space-y-2 text-center">
        <BellOff size={22} class="mx-auto text-muted-foreground/70" />
        <p class="text-sm font-medium">{activity.events.length === 0 ? 'No activity yet' : 'Nothing matches'}</p>
        <p class="text-xs text-muted-foreground">Agents that need input, finish a turn or exit appear here.</p>
      </div>
    {:else}
      {#each groups as [day, events] (day)}
        <h2 class="mb-2 mt-4 text-[11px] font-medium tracking-wider text-muted-foreground/70 uppercase first:mt-0">{day}</h2>
        <ul class="divide-y divide-border overflow-hidden rounded-xl border border-border" aria-label={day}>
          {#each events as event (event.id)}
            <li>
              <button
                type="button"
                class="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-accent/50 disabled:opacity-60"
                disabled={!exists(event)}
                title={exists(event) ? 'Open this terminal' : 'That workspace was deleted'}
                onclick={() => { markRead(event.id); onOpen(event); }}
              >
                {#if event.kind === 'needs-input'}<CircleHelp size={15} class="shrink-0 text-agent-question" />
                {:else if event.kind === 'task-complete'}<CircleCheck size={15} class="shrink-0 text-emerald-500" />
                {:else}<CircleStop size={15} class="shrink-0 text-muted-foreground" />{/if}
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm"><span class="font-medium">{event.terminalName}</span> {describeActivity(event.kind)}{event.detail ? ` (${event.detail})` : ''}</span>
                  <span class="block truncate text-xs text-muted-foreground">{event.workerName}</span>
                </span>
                <span class="shrink-0 text-xs tabular-nums text-muted-foreground">{time(event.at)}</span>
                {#if !event.read}<span class="size-2 shrink-0 rounded-full bg-blue-500" aria-label="Unread"></span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/each}
    {/if}
  </div>
</div>
