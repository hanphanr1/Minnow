<script lang="ts">
  // Orca settings/session-history (MIT): browse and search real omp transcripts kept in each
  // agent terminal's session directory, then jump back to the terminal that owns them.
  import { onMount } from 'svelte';
  import { ArrowLeft, Loader2, MessageSquare, RefreshCw, Search, SquareTerminal } from 'lucide-svelte';
  import type { Worker } from '../../types';
  import { errorMessage } from '../../api';
  import { listAgentSessions, readAgentSession, searchAgentSessions, type SessionHit, type SessionMessage, type SessionSummary } from '../../sessions-api';

  let { workers, onOpenTerminal }: { workers: Worker[]; onOpenTerminal?: (workerId: string, terminalId: string) => void } = $props();

  let sessions = $state<SessionSummary[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let query = $state('');
  let hits = $state<SessionHit[] | null>(null);
  let searching = $state(false);
  let selected = $state<SessionSummary | null>(null);
  let transcript = $state<SessionMessage[] | null>(null);
  let transcriptError = $state<string | null>(null);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let searchSeq = 0;

  const workerName = (id: string): string => workers.find((worker) => worker.id === id)?.name ?? 'Deleted workspace';
  const hitPaths = $derived(hits ? new Set(hits.map((hit) => hit.path)) : null);
  const shown = $derived(
    sessions.filter((session) => {
      if (!query.trim()) return true;
      const meta = `${session.title} ${session.preview} ${workerName(session.worker_id)}`.toLowerCase();
      return meta.includes(query.trim().toLowerCase()) || (hitPaths?.has(session.path) ?? false);
    })
  );

  async function load(): Promise<void> {
    loading = true;
    error = null;
    try {
      sessions = await listAgentSessions();
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      loading = false;
    }
  }

  function onQuery(): void {
    if (searchTimer) clearTimeout(searchTimer);
    const text = query.trim();
    const mine = ++searchSeq;
    if (!text) { hits = null; searching = false; return; }
    searching = true;
    searchTimer = setTimeout(async () => {
      try {
        const result = await searchAgentSessions(text);
        if (mine === searchSeq) hits = result;
      } catch {
        if (mine === searchSeq) hits = [];
      } finally {
        if (mine === searchSeq) searching = false;
      }
    }, 250);
  }

  async function open(session: SessionSummary): Promise<void> {
    selected = session;
    transcript = null;
    transcriptError = null;
    try {
      transcript = await readAgentSession(session.path);
    } catch (failure) {
      transcriptError = errorMessage(failure);
    }
  }

  function when(value: string): string {
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
  }

  onMount(() => { void load(); });
</script>

{#if selected}
  <div class="space-y-3">
    <div class="flex items-center gap-2">
      <button type="button" class="mn-button" onclick={() => { selected = null; }}><ArrowLeft size={14} /> All sessions</button>
      <span class="flex-1"></span>
      {#if onOpenTerminal && workers.some((worker) => worker.id === selected?.worker_id)}
        <button type="button" class="mn-button primary" onclick={() => { if (selected) onOpenTerminal?.(selected.worker_id, selected.terminal_id); }}><SquareTerminal size={14} /> Open terminal</button>
      {/if}
    </div>
    <div>
      <h3 class="text-base font-semibold">{selected.title}</h3>
      <p class="text-xs text-muted-foreground">{workerName(selected.worker_id)} · {selected.terminal_name} · {when(selected.updated_at)}</p>
    </div>
    {#if transcriptError}<p class="mn-error" role="alert">{transcriptError}</p>
    {:else if !transcript}<div class="flex items-center gap-2 text-sm text-muted-foreground"><Loader2 size={14} class="animate-spin" /> Loading transcript…</div>
    {:else}
      <ol class="space-y-2" aria-label="Transcript">
        {#each transcript as message, index (index)}
          <li class={`rounded-lg border border-border px-3 py-2 ${message.role === 'user' ? 'bg-accent/40' : 'bg-card/30'}`}>
            <div class="mb-1 flex items-center gap-2 text-[11px] text-muted-foreground">
              <span class="font-medium uppercase tracking-wide">{message.role}</span>
              {#if message.model}<span>{message.model}</span>{/if}
              <span class="flex-1"></span><span>{when(message.timestamp)}</span>
            </div>
            <p class="whitespace-pre-wrap break-words text-sm">{message.text}</p>
            {#if message.truncated}<p class="mt-1 text-[11px] text-muted-foreground">Message shortened for display.</p>{/if}
          </li>
        {/each}
      </ol>
    {/if}
  </div>
{:else}
  <div class="space-y-3">
    <div class="flex items-center gap-2">
      <div class="flex h-8 flex-1 items-center gap-2 rounded-md border border-input bg-background px-2">
        <Search size={14} class="text-muted-foreground" />
        <input bind:value={query} oninput={onQuery} class="min-w-0 flex-1 bg-transparent text-sm outline-none" placeholder="Search titles and messages" aria-label="Search sessions" />
        {#if searching}<Loader2 size={12} class="animate-spin text-muted-foreground" />{/if}
      </div>
      <button type="button" class="mn-icon-button" aria-label="Refresh sessions" onclick={() => void load()}><RefreshCw size={14} /></button>
    </div>
    {#if loading}
      <div class="flex items-center gap-2 text-sm text-muted-foreground"><Loader2 size={14} class="animate-spin" /> Reading session files…</div>
    {:else if error}
      <p class="mn-error" role="alert">{error}</p>
    {:else if shown.length === 0}
      <div class="rounded-xl border border-dashed border-border/60 px-6 py-10 text-center text-sm text-muted-foreground">
        {sessions.length === 0 ? 'No omp sessions yet. Agent terminals save their transcripts here.' : 'No sessions match.'}
      </div>
    {:else}
      <ul class="divide-y divide-border overflow-hidden rounded-xl border border-border" aria-label="Sessions">
        {#each shown as session (session.path)}
          {@const hit = hits?.find((item) => item.path === session.path)}
          <li>
            <button type="button" class="flex w-full items-start gap-3 px-3 py-2 text-left hover:bg-accent/50" onclick={() => void open(session)}>
              <MessageSquare size={14} class="mt-0.5 shrink-0 text-muted-foreground" />
              <span class="min-w-0 flex-1">
                <span class="block truncate text-sm font-medium">{session.title}</span>
                <span class="block truncate text-xs text-muted-foreground">{hit ? hit.snippet : session.preview}</span>
                <span class="block truncate text-[11px] text-muted-foreground">{workerName(session.worker_id)} · {session.terminal_name} · {session.messages} messages</span>
              </span>
              <span class="shrink-0 text-xs text-muted-foreground">{when(session.updated_at)}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}
