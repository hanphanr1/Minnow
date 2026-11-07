<!-- Ported from Orca ShortcutKeyCombo.tsx (MIT); Windows renders "+" between keys. -->
<script lang="ts">
  let {
    keys,
    capClass = '',
    separatorClass = ''
  }: {
    keys: string[];
    capClass?: string;
    separatorClass?: string;
  } = $props();
</script>

<span class="inline-flex items-center gap-1">
  {#each keys as key, index (key + index)}
    <span
      class={`inline-flex min-w-6 items-center justify-center rounded border border-border/80 bg-secondary/70 px-1.5 py-0.5 text-xs font-medium text-muted-foreground shadow-sm ${capClass}`}
    >{key}</span>
    {#if index < keys.length - 1}
      <span class={separatorClass || 'mx-0.5 text-xs text-muted-foreground'}>+</span>
    {/if}
  {/each}
</span>
