<script lang="ts">
  // Orca 1.4.214 components/settings/appearance/AppearanceAdvancedDisclosure.tsx (MIT).
  import type { Snippet } from 'svelte';
  import { ChevronRight } from 'lucide-svelte';

  let {
    label = 'Advanced',
    forceOpen = false,
    showTopBorder = true,
    className = '',
    contentClassName = '',
    children
  }: {
    label?: string;
    forceOpen?: boolean;
    showTopBorder?: boolean;
    className?: string;
    contentClassName?: string;
    children: Snippet;
  } = $props();

  let open = $state(false);
  let expanded = $derived(open || forceOpen);
</script>

<div class={`mt-3 pt-2 ${showTopBorder ? 'border-t border-border/50' : ''} ${className}`}>
  <button
    type="button"
    aria-expanded={expanded}
    onclick={() => {
      open = !open;
    }}
    disabled={forceOpen}
    class="flex w-full items-center gap-2 py-1 text-sm font-semibold text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none disabled:cursor-default"
  >
    <ChevronRight
      aria-hidden="true"
      class={`size-3.5 text-muted-foreground transition-transform ${
        expanded ? 'rotate-90' : ''
      }`}
    />
    {label}
  </button>
  {#if expanded}
    <div class={`pt-1 ${contentClassName}`}>
      {@render children()}
    </div>
  {/if}
</div>
