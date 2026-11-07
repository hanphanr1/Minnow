<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    description,
    align = 'start',
    className = '',
    children
  }: {
    title: string;
    description?: string;
    align?: 'start' | 'center';
    className?: string;
    children?: Snippet;
  } = $props();
</script>

<div
  class={`flex ${align === 'center' ? 'items-center' : 'items-start'} justify-between gap-3 ${className}`}
>
  <div class="space-y-1">
    <h3 class="text-sm font-semibold">{title}</h3>
    {#if description}
      <p class="text-xs text-muted-foreground">{description}</p>
    {/if}
  </div>
  {#if children}
    <div class="shrink-0">{@render children()}</div>
  {/if}
</div>
