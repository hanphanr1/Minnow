<script lang="ts">
  // Orca 1.4.214 components/settings/terminal/appearance/TerminalFontSizeSetting.tsx (MIT).
  import { untrack } from 'svelte';
  import { Minus, Plus } from 'lucide-svelte';

  let {
    value,
    min,
    max,
    label,
    suffix = 'px',
    onchange
  }: {
    value: number;
    min: number;
    max: number;
    label: string;
    suffix?: string;
    onchange: (value: number) => void;
  } = $props();

  const stepperButtonClass =
    'inline-flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-background text-foreground shadow-xs transition-all outline-none hover:border-muted-foreground/35 hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 dark:border-input dark:bg-input/30 dark:hover:bg-input/50';

  // The draft deliberately starts from the first value; the effect below keeps it current.
  let draft = $state(untrack(() => String(value)));

  $effect(() => {
    draft = String(value);
  });

  function step(delta: number): void {
    const parsed = Number.parseInt(draft, 10);
    const current = Number.isFinite(parsed) ? parsed : value;
    const next = Math.min(max, Math.max(min, current + delta));
    draft = String(next);
    if (next !== value) onchange(next);
  }

  function commit(): void {
    const parsed = Number.parseInt(draft, 10);
    if (!Number.isFinite(parsed)) {
      draft = String(value);
      return;
    }
    const clamped = Math.min(max, Math.max(min, parsed));
    draft = String(clamped);
    if (clamped !== value) onchange(clamped);
  }
</script>

<div class="flex items-center gap-2">
  <button
    type="button"
    aria-label={`Decrease ${label.toLowerCase()}`}
    onclick={() => step(-1)}
    disabled={value <= min}
    class={stepperButtonClass}
  >
    <Minus class="size-3" aria-hidden="true" />
  </button>
  <input
    type="number"
    {min}
    {max}
    step="1"
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
    class="number-input-clean h-9 w-14 rounded-md border border-input bg-transparent px-3 py-1 text-center text-sm tabular-nums shadow-xs transition-[color,box-shadow] outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:bg-input/30"
  />
  <button
    type="button"
    aria-label={`Increase ${label.toLowerCase()}`}
    onclick={() => step(1)}
    disabled={value >= max}
    class={stepperButtonClass}
  >
    <Plus class="size-3" aria-hidden="true" />
  </button>
  <span class="text-xs text-muted-foreground">{suffix}</span>
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
