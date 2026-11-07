<script lang="ts">
  // Orca components/settings/quick-commands/QuickCommandsPane.tsx and
  // terminal-quick-commands/TerminalQuickCommandDialog.tsx (MIT), as one inline editor.
  import { ArrowDown, ArrowUp, Pencil, Plus, Trash2, Bot, SquareTerminal } from 'lucide-svelte';
  import type { Project } from '../../types';
  import {
    newQuickCommandId,
    quickCommandStore,
    setQuickCommands,
    type QuickCommand
  } from '../quick-commands.svelte';
  import { matchesSearch } from './search';

  let { projects, query = '' }: { projects: Project[]; query?: string } = $props();

  let draft = $state<QuickCommand | null>(null);
  let draftIsNew = $state(false);
  let error = $state<string | null>(null);
  let scopeFilter = $state<'all' | string>('all');

  const visible = $derived(
    quickCommandStore.list.filter((command) => {
      if (scopeFilter !== 'all') {
        if (scopeFilter === 'global' ? command.scope.type !== 'global' : command.scope.type !== 'repo' || command.scope.repoId !== scopeFilter) return false;
      }
      return !query.trim() || matchesSearch(query, { title: command.label, description: command.command || command.prompt, keywords: [] });
    })
  );

  function scopeLabel(command: QuickCommand): string {
    if (command.scope.type === 'global') return 'All projects';
    const repoId = command.scope.repoId;
    return projects.find((project) => project.id === repoId)?.name ?? 'Removed project';
  }

  function startNew(): void {
    draft = { id: newQuickCommandId(), label: '', scope: { type: 'global' }, action: 'terminal-command', command: '', appendEnter: true, prompt: '' };
    draftIsNew = true;
    error = null;
  }

  function startEdit(command: QuickCommand): void {
    draft = { ...command, scope: { ...command.scope } };
    draftIsNew = false;
    error = null;
  }

  function save(): void {
    if (!draft) return;
    const label = draft.label.trim();
    if (!label) { error = 'Give the command a label.'; return; }
    if (draft.action === 'terminal-command' && !draft.command.trim()) { error = 'Enter the command to run.'; return; }
    if (draft.action === 'agent-prompt' && !draft.prompt.trim()) { error = 'Enter the prompt for the agent.'; return; }
    const value = { ...draft, label };
    const list = quickCommandStore.list;
    setQuickCommands(draftIsNew ? [...list, value] : list.map((item) => (item.id === value.id ? value : item)));
    draft = null;
  }

  function remove(id: string): void {
    setQuickCommands(quickCommandStore.list.filter((item) => item.id !== id));
    if (draft?.id === id) draft = null;
  }

  function move(id: string, delta: -1 | 1): void {
    const list = [...quickCommandStore.list];
    const index = list.findIndex((item) => item.id === id);
    const target = index + delta;
    if (index < 0 || target < 0 || target >= list.length) return;
    [list[index], list[target]] = [list[target], list[index]];
    setQuickCommands(list);
  }
</script>

<div class="space-y-4">
  <div class="flex items-center gap-2">
    <label class="text-xs text-muted-foreground" for="qc-scope-filter">Show</label>
    <select id="qc-scope-filter" bind:value={scopeFilter} class="h-8 rounded-md border border-input bg-background px-2 text-sm">
      <option value="all">All commands</option>
      <option value="global">All projects</option>
      {#each projects as project (project.id)}<option value={project.id}>{project.name}</option>{/each}
    </select>
    <span class="flex-1"></span>
    <button type="button" class="mn-button primary" onclick={startNew}><Plus size={14} /> Add Command</button>
  </div>

  {#if draft}
    <form
      class="space-y-3 rounded-xl border border-border bg-card/40 p-4"
      aria-label={draftIsNew ? 'New quick command' : 'Edit quick command'}
      onsubmit={(event) => { event.preventDefault(); save(); }}
    >
      <div class="grid grid-cols-2 gap-1 rounded-md bg-muted p-0.5" role="group" aria-label="Command action">
        <button type="button" aria-pressed={draft.action === 'terminal-command'} class={`h-7 rounded-md text-xs ${draft.action === 'terminal-command' ? 'bg-foreground/12 text-foreground dark:bg-accent' : 'text-muted-foreground'}`} onclick={() => { if (draft) draft.action = 'terminal-command'; }}>Terminal command</button>
        <button type="button" aria-pressed={draft.action === 'agent-prompt'} class={`h-7 rounded-md text-xs ${draft.action === 'agent-prompt' ? 'bg-foreground/12 text-foreground dark:bg-accent' : 'text-muted-foreground'}`} onclick={() => { if (draft) draft.action = 'agent-prompt'; }}>Agent prompt</button>
      </div>
      <label class="block space-y-1">
        <span class="text-xs font-medium">Label</span>
        <input bind:value={draft.label} class="h-8 w-full rounded-md border border-input bg-background px-2 text-sm" placeholder="Run tests" aria-label="Label" />
      </label>
      {#if draft.action === 'terminal-command'}
        <label class="block space-y-1">
          <span class="text-xs font-medium">Command</span>
          <textarea bind:value={draft.command} rows="3" spellcheck="false" class="w-full rounded-md border border-input bg-background px-2 py-1 font-mono text-xs" placeholder="npm test" aria-label="Command"></textarea>
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={draft.appendEnter} aria-label="Press Enter after typing" />
          Press Enter after typing
        </label>
      {:else}
        <label class="block space-y-1">
          <span class="text-xs font-medium">Prompt</span>
          <textarea bind:value={draft.prompt} rows="4" class="w-full rounded-md border border-input bg-background px-2 py-1 text-sm" placeholder="Review the open changes and summarize risks." aria-label="Prompt"></textarea>
        </label>
        <p class="text-xs text-muted-foreground">Starts a new omp agent terminal in the selected workspace and sends this prompt.</p>
      {/if}
      <label class="block space-y-1">
        <span class="text-xs font-medium">Available in</span>
        <select
          class="h-8 w-full rounded-md border border-input bg-background px-2 text-sm"
          aria-label="Scope"
          value={draft.scope.type === 'global' ? 'global' : draft.scope.repoId}
          onchange={(event) => {
            const value = event.currentTarget.value;
            if (draft) draft.scope = value === 'global' ? { type: 'global' } : { type: 'repo', repoId: value };
          }}
        >
          <option value="global">All projects</option>
          {#each projects as project (project.id)}<option value={project.id}>{project.name}</option>{/each}
        </select>
      </label>
      {#if error}<p class="mn-error" role="alert">{error}</p>{/if}
      <div class="flex justify-end gap-2">
        <button type="button" class="mn-button" onclick={() => { draft = null; }}>Cancel</button>
        <button type="submit" class="mn-button primary">Save</button>
      </div>
    </form>
  {/if}

  {#if visible.length === 0}
    <div class="rounded-xl border border-dashed border-border/60 px-6 py-10 text-center text-sm text-muted-foreground">
      {quickCommandStore.list.length === 0 ? 'No quick commands yet. Add one to run it from the Command menu in any workspace.' : 'No commands match.'}
    </div>
  {:else}
    <ul class="divide-y divide-border overflow-hidden rounded-xl border border-border" aria-label="Quick commands">
      {#each visible as command (command.id)}
        <li class="flex items-center gap-3 px-3 py-2">
          {#if command.action === 'agent-prompt'}<Bot size={14} class="shrink-0 text-muted-foreground" />{:else}<SquareTerminal size={14} class="shrink-0 text-muted-foreground" />{/if}
          <div class="min-w-0 flex-1">
            <div class="truncate text-sm font-medium">{command.label}</div>
            <div class="truncate font-mono text-[11px] text-muted-foreground">{command.action === 'agent-prompt' ? command.prompt : command.command}</div>
          </div>
          <span class="shrink-0 rounded-full border border-border px-2 py-0.5 text-[10px] text-muted-foreground">{scopeLabel(command)}</span>
          <button type="button" class="mn-icon-button" aria-label={`Move ${command.label} up`} onclick={() => move(command.id, -1)}><ArrowUp size={13} /></button>
          <button type="button" class="mn-icon-button" aria-label={`Move ${command.label} down`} onclick={() => move(command.id, 1)}><ArrowDown size={13} /></button>
          <button type="button" class="mn-icon-button" aria-label={`Edit ${command.label}`} onclick={() => startEdit(command)}><Pencil size={13} /></button>
          <button type="button" class="mn-icon-button" aria-label={`Delete ${command.label}`} onclick={() => remove(command.id)}><Trash2 size={13} /></button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
