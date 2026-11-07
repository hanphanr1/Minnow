<script lang="ts">
  interface Option {
    value: string;
    label: string;
    disabled?: boolean;
  }

  let {
    value,
    options,
    ariaLabel,
    onchange,
    size = 'md',
    equalWidth = false
  }: {
    value: string;
    options: readonly Option[];
    ariaLabel: string;
    onchange: (value: string) => void;
    size?: 'sm' | 'md';
    equalWidth?: boolean;
  } = $props();
</script>

<div
  role="radiogroup"
  aria-label={ariaLabel}
  class={`inline-flex items-center rounded-md border border-border bg-background/50 p-0.5 ${equalWidth
    ? 'w-full'
    : ''}`}
>
  {#each options as option (option.value)}
    <button
      type="button"
      role="radio"
      aria-checked={option.value === value}
      aria-disabled={option.disabled}
      onclick={() => {
        if (!option.disabled) onchange(option.value);
      }}
      class={`rounded-sm text-center outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50 ${equalWidth
        ? 'flex-1'
        : ''} ${size === 'sm' ? 'px-2.5 py-0.5 text-xs' : 'px-3 py-1 text-sm'} ${
        option.value === value
          ? 'bg-accent font-medium text-accent-foreground'
          : option.disabled
            ? 'cursor-not-allowed text-muted-foreground/50'
            : 'cursor-pointer text-muted-foreground hover:text-foreground'
      }`}
    >
      {option.label}
    </button>
  {/each}
</div>
