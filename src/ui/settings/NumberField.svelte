<script lang="ts">
  import { untrack } from 'svelte';

  let {
    label,
    value,
    min,
    max,
    step = 1,
    suffix,
    onchange,
    disabled = false
  }: {
    label: string;
    value: number;
    min: number;
    max: number;
    step?: number;
    suffix?: string;
    onchange: (value: number) => void;
    disabled?: boolean;
  } = $props();

  // The draft deliberately starts from the first value; the effect below keeps it current.
  let draft = $state(untrack(() => String(value)));

  $effect(() => {
    draft = String(value);
  });

  function commit(): void {
    if (disabled) return;
    const trimmed = draft.trim();
    const parsed = Number(trimmed);
    if (trimmed === '' || !Number.isFinite(parsed)) {
      draft = String(value);
      return;
    }
    const clamped = Math.min(max, Math.max(min, parsed));
    draft = String(clamped);
    if (clamped !== value) onchange(clamped);
  }
</script>

<div class="flex items-center gap-2">
  <input
    type="number"
    {min}
    {max}
    {step}
    {disabled}
    aria-label={label}
    value={draft}
    oninput={(event) => {
      draft = event.currentTarget.value;
    }}
    onblur={commit}
    onkeydown={(event) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        commit();
      }
    }}
    class="number-input-clean h-9 w-24 rounded-md border border-input bg-transparent px-3 py-1 text-sm tabular-nums shadow-xs transition-[color,box-shadow] outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 dark:bg-input/30"
  />
  {#if suffix}
    <span class="shrink-0 text-xs text-muted-foreground">{suffix}</span>
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
