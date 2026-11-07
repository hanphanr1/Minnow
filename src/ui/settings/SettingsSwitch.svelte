<script lang="ts">
  let {
    checked,
    ariaLabel,
    onchange,
    disabled = false
  }: {
    checked: boolean;
    ariaLabel: string;
    onchange: () => void;
    disabled?: boolean;
  } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={ariaLabel}
  {disabled}
  onclick={onchange}
  class={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-transparent p-0.5 transition-colors outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 ${
    checked ? 'bg-foreground' : 'bg-muted-foreground/30'
  } ${disabled ? 'cursor-not-allowed' : 'cursor-pointer'}`}
>
  <span
    class={`pointer-events-none block size-3.5 rounded-full bg-background shadow-sm transition-transform ${
      checked ? 'translate-x-4' : 'translate-x-0'
    }`}
  ></span>
</button>
