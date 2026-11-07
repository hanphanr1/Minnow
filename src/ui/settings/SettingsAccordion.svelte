<script lang="ts">
  // Orca 1.4.214 components/settings/appearance/AppearanceSection.tsx (MIT, © 2026 Lovecast Inc.)
  import type { Snippet } from 'svelte';
  import { ChevronRight } from 'lucide-svelte';

  let {
    id,
    title,
    summary,
    open,
    disabled = false,
    onToggle,
    icon,
    children
  }: {
    id: string;
    title: string;
    summary: string;
    open: boolean;
    disabled?: boolean;
    onToggle: () => void;
    icon: Snippet;
    children: Snippet;
  } = $props();

  const contentId = $derived(`appearance-section-${id}`);
</script>

<div
  class={`overflow-hidden rounded-xl border border-border/50 bg-card transition-colors ${
    open ? 'border-ring/40' : ''
  }`}
>
  <button
    type="button"
    aria-expanded={open}
    aria-controls={contentId}
    onclick={onToggle}
    {disabled}
    class="flex w-full items-center gap-3.5 px-4 py-3.5 text-left transition-colors hover:bg-accent/15 focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none disabled:cursor-default disabled:hover:bg-transparent"
  >
    <span
      class="grid size-8 shrink-0 place-items-center rounded-md bg-secondary text-foreground [&_svg]:size-4"
    >
      {@render icon()}
    </span>
    <span class="min-w-0 flex-1">
      <span class="block text-sm font-semibold">{title}</span>
      {#if !open}
        <span class="block truncate text-xs text-muted-foreground">{summary}</span>
      {/if}
    </span>
    <ChevronRight
      aria-hidden="true"
      class={`size-[18px] shrink-0 text-muted-foreground transition-transform ${
        open ? 'rotate-90 text-foreground' : ''
      }`}
    />
  </button>
  <div
    class={`grid overflow-hidden transition-[grid-template-rows,opacity,border-color] duration-200 ease-out motion-reduce:transition-none ${
      open
        ? 'grid-rows-[1fr] border-t border-border/50 opacity-100'
        : 'grid-rows-[0fr] border-t border-transparent opacity-0'
    }`}
    aria-hidden={!open}
    inert={!open}
  >
    <div class="min-h-0 overflow-hidden">
      <div id={contentId} role="region" class="px-4 pt-1 pb-4">
        {@render children()}
      </div>
    </div>
  </div>
</div>
