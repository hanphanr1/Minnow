<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    description,
    alignTop = false,
    className = '',
    children
  }: {
    label: string | Snippet;
    description?: string | Snippet;
    alignTop?: boolean;
    className?: string;
    children?: Snippet;
  } = $props();
</script>

<div
  class={`setting-row flex gap-4 ${description ? 'py-3' : 'py-2'} ${alignTop
    ? 'items-start'
    : 'items-center justify-between'} ${className}`}
>
  <div class={`min-w-0 flex-1 ${description ? 'space-y-1' : 'space-y-0.5'}`}>
    {#if typeof label === 'string'}
      <div class="text-sm leading-none font-medium select-text">{label}</div>
    {:else}
      <div class="text-sm leading-none font-medium select-text">{@render label()}</div>
    {/if}
    {#if typeof description === 'string'}
      <p class="text-xs select-text text-muted-foreground">{description}</p>
    {:else if description}
      <p class="text-xs select-text text-muted-foreground">{@render description()}</p>
    {/if}
  </div>
  <div class="shrink-0">
    {@render children?.()}
  </div>
</div>

<style>
  @container settings-content (max-width: 640px) {
    .setting-row { flex-direction: column; align-items: flex-start; gap: 12px; }
    .setting-row > div { max-width: 100%; }
  }
</style>
