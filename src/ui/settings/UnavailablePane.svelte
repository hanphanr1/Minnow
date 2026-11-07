<script lang="ts">
  // Explicit excluded-feature shell. Navigation mirrors Orca; the backing service is absent.
  import { Info } from 'lucide-svelte';
  import type { NavSection } from './settings-data';
  import SubsectionHeader from './SubsectionHeader.svelte';

  let { section }: { section: NavSection } = $props();
</script>

<div class="space-y-5">
  <div class="flex items-start gap-3 rounded-lg border border-border/60 bg-background/40 px-4 py-3">
    <Info class="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
    <div class="min-w-0 space-y-1">
      <p class="flex flex-wrap items-center gap-2 text-sm font-semibold">
        Not available in this build
        <span
          class="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium tracking-[0.05em] text-muted-foreground uppercase"
        >
          Unavailable
        </span>
      </p>
      <p class="max-w-3xl text-xs leading-5 text-muted-foreground">
        The reference client provides this settings pane. Minnow does not implement the backing
        service or data store, so the page keeps its navigation entry without pretending the
        controls work.
      </p>
    </div>
  </div>

  {#if section.includes?.length}
    <div class="space-y-3 border-t border-border/40 pt-5">
      <SubsectionHeader
        title="Source pane contents"
        description="Listed so the missing capability is explicit."
      />
      <ul class="space-y-2">
        {#each section.includes as item (item)}
          <li class="flex items-start gap-2 text-sm text-muted-foreground">
            <span
              class="mt-1.5 size-1.5 shrink-0 rounded-full bg-muted-foreground/60"
              aria-hidden="true"
            ></span>
            <span class="select-text">{item}</span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>
