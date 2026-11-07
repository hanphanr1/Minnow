<script lang="ts">
  // Orca settings/stats (MIT): usage computed only from local omp transcripts and live state.
  import { onMount } from 'svelte';
  import { Loader2 } from 'lucide-svelte';
  import type { TerminalSession, Worker } from '../../types';
  import { errorMessage } from '../../api';
  import { listAgentSessions, type SessionSummary } from '../../sessions-api';

  let { workers, terminals }: { workers: Worker[]; terminals: TerminalSession[] } = $props();

  let sessions = $state<SessionSummary[] | null>(null);
  let error = $state<string | null>(null);

  const totals = $derived.by(() => {
    const list = sessions ?? [];
    const sum = { sessions: list.length, messages: 0, prompts: 0, input: 0, output: 0, cacheRead: 0, cost: 0 };
    for (const session of list) {
      sum.messages += session.messages;
      sum.prompts += session.user_messages;
      sum.input += session.usage.input;
      sum.output += session.usage.output;
      sum.cacheRead += session.usage.cache_read;
      sum.cost += session.usage.cost;
    }
    return sum;
  });

  const byModel = $derived.by(() => {
    const map = new Map<string, number>();
    for (const session of sessions ?? []) for (const model of session.models) map.set(model, (map.get(model) ?? 0) + 1);
    return [...map.entries()].sort((left, right) => right[1] - left[1]);
  });

  const byWorkspace = $derived.by(() => {
    const map = new Map<string, { sessions: number; tokens: number; cost: number }>();
    for (const session of sessions ?? []) {
      const entry = map.get(session.worker_id) ?? { sessions: 0, tokens: 0, cost: 0 };
      entry.sessions += 1;
      entry.tokens += session.usage.input + session.usage.output;
      entry.cost += session.usage.cost;
      map.set(session.worker_id, entry);
    }
    return [...map.entries()].sort((left, right) => right[1].tokens - left[1].tokens);
  });

  const liveTerminals = $derived(terminals.filter((terminal) => !terminal.closed && terminal.pid !== null).length);
  const format = (value: number): string => value.toLocaleString();
  const name = (id: string): string => workers.find((worker) => worker.id === id)?.name ?? 'Deleted workspace';

  onMount(async () => {
    try {
      sessions = await listAgentSessions();
    } catch (failure) {
      error = errorMessage(failure);
    }
  });
</script>

{#if error}
  <p class="mn-error" role="alert">{error}</p>
{:else if !sessions}
  <div class="flex items-center gap-2 text-sm text-muted-foreground"><Loader2 size={14} class="animate-spin" /> Reading session files…</div>
{:else}
  <div class="space-y-6">
    <dl class="grid grid-cols-2 gap-3 sm:grid-cols-4">
      {#each [['Workspaces', format(workers.length)], ['Running terminals', format(liveTerminals)], ['Agent sessions', format(totals.sessions)], ['Prompts sent', format(totals.prompts)], ['Input tokens', format(totals.input)], ['Output tokens', format(totals.output)], ['Cache reads', format(totals.cacheRead)], ['Reported cost', `$${totals.cost.toFixed(2)}`]] as [label, value] (label)}
        <div class="rounded-xl border border-border bg-card/30 px-3 py-2">
          <dt class="text-[11px] text-muted-foreground">{label}</dt>
          <dd class="text-lg font-semibold tabular-nums">{value}</dd>
        </div>
      {/each}
    </dl>
    <p class="text-xs text-muted-foreground">Counted from the omp transcripts of Minnow's agent terminals. Cost is what omp recorded; it may be zero for providers that do not report it.</p>
    {#if byModel.length}
      <section class="space-y-2">
        <h3 class="text-sm font-medium">Sessions by model</h3>
        <ul class="divide-y divide-border rounded-xl border border-border">
          {#each byModel as [model, count] (model)}
            <li class="flex items-center justify-between px-3 py-1.5 text-sm"><span class="truncate font-mono text-xs">{model}</span><span class="tabular-nums">{count}</span></li>
          {/each}
        </ul>
      </section>
    {/if}
    {#if byWorkspace.length}
      <section class="space-y-2">
        <h3 class="text-sm font-medium">Usage by workspace</h3>
        <ul class="divide-y divide-border rounded-xl border border-border">
          {#each byWorkspace as [workerId, entry] (workerId)}
            <li class="flex items-center gap-3 px-3 py-1.5 text-sm"><span class="min-w-0 flex-1 truncate">{name(workerId)}</span><span class="tabular-nums text-muted-foreground">{entry.sessions} sessions</span><span class="w-28 text-right tabular-nums">{format(entry.tokens)} tok</span><span class="w-16 text-right tabular-nums">${entry.cost.toFixed(2)}</span></li>
          {/each}
        </ul>
      </section>
    {/if}
  </div>
{/if}
