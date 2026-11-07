<script lang="ts">
  // Orca 1.4.214 components/settings/shortcuts/ShortcutsPane.tsx,
  // ShortcutFilterRail.tsx, ShortcutRowsList.tsx and ShortcutCommandBlock.tsx layout (MIT).
  // Minnow has no keybinding override store, so this port is an honest read-only inventory.
  import { Info, Search, X } from 'lucide-svelte';
  import {
    shortcutDefinitions,
    shortcutFilterLabels,
    type ShortcutDefinition,
    type ShortcutFilter
  } from './shortcut-catalog';
  import { matchesSearch, normalizeQuery } from './search';
  import SubsectionHeader from './SubsectionHeader.svelte';

  let { query = '' }: { query?: string } = $props();

  let localQuery = $state('');
  let filter = $state<ShortcutFilter>('all');

  let searching = $derived(normalizeQuery(query).length > 0);
  let paneLabelMatch = $derived(
    matchesSearch(query, {
      title: 'Shortcuts',
      description: 'Keyboard shortcuts for common actions.',
      keywords: ['keyboard', 'shortcuts', 'keybindings', 'keys', 'bindings']
    })
  );
  let globalFilterActive = $derived(searching && !paneLabelMatch);

  let baseRows = $derived.by(() => {
    const globalRows = globalFilterActive
      ? shortcutDefinitions.filter((definition) =>
          matchesSearch(query, {
            title: definition.title,
            description: `${definition.group} shortcut`,
            keywords: [
              ...definition.keywords,
              definition.group,
              ...definition.keys.flat()
            ]
          })
        )
      : shortcutDefinitions;
    const normalized = normalizeQuery(localQuery);
    if (!normalized) return globalRows;
    return globalRows.filter((definition) => {
      const searchable = [
        definition.title,
        definition.id,
        definition.group,
        ...definition.keywords,
        ...definition.keys.map((keys) => keys.join('+'))
      ];
      return searchable.some((value) => value.toLowerCase().includes(normalized));
    });
  });

  let counts = $derived({
    all: baseRows.length,
    modified: 0,
    unassigned: baseRows.filter((definition) => definition.keys.length === 0).length,
    conflicts: 0
  });

  let visibleRows = $derived.by(() => {
    switch (filter) {
      case 'all':
        return baseRows;
      case 'modified':
        return [];
      case 'unassigned':
        return baseRows.filter((definition) => definition.keys.length === 0);
      case 'conflicts':
        return [];
      default:
        return baseRows;
    }
  });

  let groups = $derived.by(() => {
    const grouped = new Map<string, ShortcutDefinition[]>();
    for (const definition of visibleRows) {
      grouped.set(definition.group, [...(grouped.get(definition.group) ?? []), definition]);
    }
    return Array.from(grouped.entries()).map(([title, rows]) => ({ title, rows }));
  });

  const filters = Object.keys(shortcutFilterLabels) as ShortcutFilter[];

  function filterCount(id: ShortcutFilter): number {
    return counts[id];
  }

  function setLocalQuery(value: string): void {
    localQuery = value;
  }
</script>

{#snippet keyCombo(keys: string[])}
  <span class="inline-flex items-center gap-1">
    {#each keys as key, index (index)}
      <span
        class="inline-flex min-w-6 items-center justify-center rounded border border-border/80 bg-secondary/70 px-1.5 py-0.5 text-xs font-medium text-muted-foreground shadow-sm"
      >
        {key}
      </span>
      {#if index < keys.length - 1}
        <span class="mx-0.5 text-xs text-muted-foreground">+</span>
      {/if}
    {/each}
  </span>
{/snippet}

<div class="flex h-full min-h-0 flex-col gap-6 overflow-hidden">
  <div class="flex items-start gap-3 rounded-lg border border-border/60 bg-muted/30 px-4 py-3">
    <Info class="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
    <div class="min-w-0 space-y-0.5">
      <p class="text-sm font-semibold">Shortcut editing is unavailable</p>
      <p class="text-xs text-muted-foreground">
        This is the Windows keybinding inventory from the reference client. Minnow has no
        keybinding override store, so shortcuts cannot be recorded or changed here.
      </p>
    </div>
  </div>

  <section class="flex min-h-0 flex-1 flex-col space-y-3">
    <SubsectionHeader
      title="Keyboard Shortcuts"
      description="Searchable source-matched defaults. Read-only in this build."
    />

    <div
      class="grid min-h-0 flex-1 gap-6 max-xl:grid-rows-[auto_minmax(0,1fr)] xl:grid-cols-[16rem_minmax(0,1fr)]"
    >
        <aside class="flex min-h-0 flex-col gap-5 xl:h-full">
          <div class="shrink-0 space-y-2">
            <div class="flex items-center justify-between gap-3">
              <label for="shortcut-filter-search" class="text-xs font-medium">Find shortcuts</label>
              <span class="text-[11px] text-muted-foreground">
                {visibleRows.length}/{shortcutDefinitions.length}
              </span>
            </div>
            <div class="relative">
              <Search
                class="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <input
                id="shortcut-filter-search"
                type="text"
                value={localQuery}
                oninput={(event) => setLocalQuery(event.currentTarget.value)}
                placeholder="Search command or keys"
                class="h-8 w-full min-w-0 appearance-none rounded-md border border-input bg-transparent py-1 pr-8 pl-8 text-sm shadow-xs transition-[color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:bg-input/30"
              />
              {#if localQuery}
                <button
                  type="button"
                  aria-label="Clear shortcut search"
                  onclick={() => setLocalQuery('')}
                  class="absolute top-1/2 right-1 inline-flex size-6 -translate-y-1/2 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
                >
                  <X class="size-3" aria-hidden="true" />
                </button>
              {/if}
            </div>
          </div>

          <nav aria-label="Shortcut status filters" class="shrink-0 space-y-2">
            <p class="text-[11px] font-semibold tracking-[0.05em] text-muted-foreground uppercase">
              Status
            </p>
            <div class="grid gap-1">
              {#each filters as id (id)}
                <button
                  type="button"
                  aria-pressed={filter === id}
                  onclick={() => {
                    filter = id;
                  }}
                  class={`flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left text-xs outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50 ${
                    filter === id
                      ? 'bg-accent font-medium text-accent-foreground'
                      : 'text-muted-foreground hover:bg-accent/60 hover:text-foreground'
                  }`}
                >
                  <span class="truncate">{shortcutFilterLabels[id]}</span>
                  <span class="text-[11px] tabular-nums opacity-80">{filterCount(id)}</span>
                </button>
              {/each}
            </div>
          </nav>
        </aside>

        <div
          class="min-h-0 min-w-0 flex-1 overflow-x-hidden overflow-y-auto pr-1 scrollbar-sleek"
        >
          {#if groups.length === 0}
            <div
              class="rounded-md border border-dashed border-border/70 px-4 py-8 text-center text-sm text-muted-foreground"
            >
              No shortcuts match those filters.
            </div>
          {:else}
          <div class="flex flex-col gap-8">
            {#each groups as group (group.title)}
              <div class="space-y-3">
                <h3 class="border-b border-border/50 pb-2 text-sm font-medium text-muted-foreground">
                  {group.title}
                </h3>
                <div class="flex flex-col gap-3">
                  {#each group.rows as definition (definition.id)}
                    <div
                      class="flex min-h-9 items-center gap-3 rounded-md px-2 py-1 transition-colors hover:bg-accent/40"
                    >
                      <div class="flex min-w-0 flex-1 items-center gap-2">
                        <span class="truncate text-sm text-foreground">{definition.title}</span>
                      </div>
                      <div class="flex shrink-0 flex-col items-end gap-1">
                        {#if definition.keys.length === 0}
                          <span class="px-1 text-xs text-muted-foreground">Unassigned</span>
                        {:else}
                          {#each definition.keys as keys, index (index)}
                            {@render keyCombo(keys)}
                          {/each}
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
          {/if}
        </div>
      </div>
  </section>
</div>
