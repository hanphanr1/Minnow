<script lang="ts">
  // Orca 1.4.214 components/settings/general/GeneralPane.tsx and
  // GeneralWorkspaceSettingsSection.tsx layout (MIT), applied to Minnow's OMP settings.
  import { untrack } from 'svelte';
  import { Check, Minus, Plus } from 'lucide-svelte';
  import type { Settings } from '../../types';
  import { matchesSearch, normalizeQuery } from './search';
  import SettingRow from './SettingRow.svelte';
  import SubsectionHeader from './SubsectionHeader.svelte';

  let {
    settings,
    onSave,
    query = ''
  }: {
    settings: Settings;
    onSave: (settings: Settings) => Promise<void>;
    query?: string;
  } = $props();

  // Initial drafts are intentionally snapshots; new settings objects re-sync in the effect below.
  let draftPath = $state(untrack(() => settings.omp_path));
  let draftCount = $state(untrack(() => String(settings.default_worker_count)));
  let previousSettings = untrack(() => settings);
  let saving = $state(false);
  let saved = $state(false);
  let error = $state<string | null>(null);

  let searching = $derived(normalizeQuery(query).length > 0);
  let labelForce = $derived(
    !searching ||
      matchesSearch(query, {
        title: 'General',
        description: 'Workspace defaults, app setup, and maintenance.',
        keywords: ['general', 'omp', 'path', 'worker', 'backend', 'settings']
      })
  );

  let executableVisible = $derived(
    labelForce ||
      rowMatches('OMP Executable', 'Command name or full path used to start workers.', [
        'omp',
        'executable',
        'path',
        'binary',
        'command',
        'backend'
      ])
  );
  let countVisible = $derived(
    labelForce ||
      rowMatches('Default Worker Count', 'How many workers new workspace creation starts with.', [
        'default',
        'worker',
        'count',
        'workspace',
        'create'
      ])
  );
  let policiesVisible = $derived(
    labelForce ||
      rowMatches('Fixed Policies', 'Backend-enforced limits that cannot be changed from Settings.', [
        'policy',
        'safety',
        'limit',
        'fixed',
        'backend'
      ])
  );
  let anythingVisible = $derived(executableVisible || countVisible || policiesVisible);

  let parsedCount = $derived(Number.parseInt(draftCount, 10));
  let dirty = $derived(
    draftPath.trim() !== settings.omp_path ||
      draftCount.trim() !== String(settings.default_worker_count)
  );

  $effect(() => {
    if (settings !== previousSettings) {
      previousSettings = settings;
      draftPath = settings.omp_path;
      draftCount = String(settings.default_worker_count);
    }
  });

  function rowMatches(title: string, description: string, keywords: string[]): boolean {
    return matchesSearch(query, { title, description, keywords });
  }

  function stepCount(delta: number): void {
    const current = Number.isFinite(parsedCount) ? parsedCount : settings.default_worker_count;
    draftCount = String(Math.min(16, Math.max(1, current + delta)));
    saved = false;
  }

  async function save(): Promise<void> {
    if (saving) return;
    const path = draftPath.trim();
    if (!path) {
      error = 'The omp executable cannot be empty.';
      return;
    }
    if (!Number.isInteger(parsedCount) || parsedCount < 1 || parsedCount > 16) {
      error = 'Default worker count must be a whole number from 1 to 16.';
      return;
    }
    saving = true;
    error = null;
    saved = false;
    try {
      await onSave({ omp_path: path, default_worker_count: parsedCount });
      saved = true;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      saving = false;
    }
  }
</script>

<div class="space-y-5">
  {#if !searching || anythingVisible}
    {#if executableVisible || countVisible}
      <section class="space-y-4">
        <SubsectionHeader
          title="Workspace"
          description="Defaults used when Minnow creates workers."
        />
        <div class="divide-y divide-border/40 border-y border-border/40">
          {#if executableVisible}
            <SettingRow
              label="OMP Executable"
              description="Command name or full path used to start workers. Resolved on the PATH when no path is given."
            >
              <input
                type="text"
                value={draftPath}
                aria-label="OMP Executable"
                spellcheck="false"
                oninput={(event) => {
                  draftPath = event.currentTarget.value;
                  saved = false;
                  error = null;
                }}
                onkeydown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault();
                    void save();
                  }
                }}
                class="h-9 w-80 max-w-full min-w-0 appearance-none rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs transition-[color,box-shadow] outline-none selection:bg-primary selection:text-primary-foreground placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:bg-input/30"
              />
            </SettingRow>
          {/if}

          {#if countVisible}
            <SettingRow
              label="Default Worker Count"
              description="How many workers new workspace creation starts with."
            >
              <div class="flex items-center gap-2">
                <button
                  type="button"
                  aria-label="Decrease default worker count"
                  onclick={() => stepCount(-1)}
                  disabled={Number.isFinite(parsedCount) && parsedCount <= 1}
                  class="inline-flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-background text-foreground shadow-xs transition-all outline-none hover:border-muted-foreground/35 hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 dark:border-input dark:bg-input/30 dark:hover:bg-input/50"
                >
                  <Minus class="size-3" aria-hidden="true" />
                </button>
                <input
                  type="number"
                  min="1"
                  max="16"
                  step="1"
                  value={draftCount}
                  aria-label="Default Worker Count"
                  oninput={(event) => {
                    draftCount = event.currentTarget.value;
                    saved = false;
                    error = null;
                  }}
                  class="number-input-clean h-9 w-16 rounded-md border border-input bg-transparent px-3 py-1 text-center text-sm tabular-nums shadow-xs transition-[color,box-shadow] outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:bg-input/30"
                />
                <button
                  type="button"
                  aria-label="Increase default worker count"
                  onclick={() => stepCount(1)}
                  disabled={Number.isFinite(parsedCount) && parsedCount >= 16}
                  class="inline-flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-background text-foreground shadow-xs transition-all outline-none hover:border-muted-foreground/35 hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 dark:border-input dark:bg-input/30 dark:hover:bg-input/50"
                >
                  <Plus class="size-3" aria-hidden="true" />
                </button>
                <span class="text-xs text-muted-foreground">workers</span>
              </div>
            </SettingRow>
          {/if}
        </div>

        {#if error}
          <p class="text-xs text-destructive" role="alert">{error}</p>
        {/if}

        <div class="flex items-center justify-end gap-3">
          {#if saved}
            <span class="flex items-center gap-1 text-xs text-muted-foreground">
              <Check class="size-3.5" aria-hidden="true" />
              Saved
            </span>
          {/if}
          <button
            type="button"
            onclick={() => void save()}
            disabled={saving || !dirty}
            class="inline-flex h-8 shrink-0 items-center justify-center gap-2 rounded-md bg-primary px-3 text-sm font-medium whitespace-nowrap text-primary-foreground transition-all outline-none hover:bg-primary/90 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50"
          >
            {saving ? 'Saving…' : 'Save changes'}
          </button>
        </div>
      </section>
    {/if}

    {#if policiesVisible}
      <section class="space-y-3">
        <SubsectionHeader
          title="Fixed Policies"
          description="Backend-enforced limits that cannot be changed from Settings."
        />
        <div class="divide-y divide-border/40">
          <SettingRow
            label="Worker creation range"
            description="The backend accepts 1 to 16 workers per creation request and rejects values outside that range."
          >
            <span
              class="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium tracking-[0.05em] text-muted-foreground uppercase"
            >
              Fixed
            </span>
          </SettingRow>
          <SettingRow
            label="One worktree, one terminal"
            description="Each worker owns its own Git worktree and PTY. Minnow never shares a PTY between workers."
          >
            <span
              class="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium tracking-[0.05em] text-muted-foreground uppercase"
            >
              Fixed
            </span>
          </SettingRow>
          <SettingRow
            label="Executable failures stay visible"
            description="If the configured omp command cannot be resolved, the worker reports the error instead of silently starting a shell."
          >
            <span
              class="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium tracking-[0.05em] text-muted-foreground uppercase"
            >
              Fixed
            </span>
          </SettingRow>
        </div>
      </section>
    {/if}
  {:else}
    <div class="rounded-md border border-dashed border-border/70 px-4 py-8 text-center text-sm text-muted-foreground">
      No general settings match those filters.
    </div>
  {/if}
</div>

<style>
  .number-input-clean {
    appearance: textfield;
  }
  .number-input-clean::-webkit-outer-spin-button,
  .number-input-clean::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
</style>
