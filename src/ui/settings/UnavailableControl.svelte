<script lang="ts">
  // Fixed-control footprint: the source control renders disabled with a visible hint.
  import type { Snippet } from 'svelte';

  let {
    title,
    hint = 'Unavailable',
    children
  }: {
    title: string;
    hint?: string;
    children: Snippet;
  } = $props();
</script>

<div class="flex items-center gap-2" {title}>
  <span class="shrink-0 text-[10px] leading-none text-muted-foreground">{hint}</span>
  <div class="opacity-60">
    {@render children()}
  </div>
</div>
