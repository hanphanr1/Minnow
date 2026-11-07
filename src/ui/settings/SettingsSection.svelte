<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    id,
    title,
    description,
    badge,
    focused = false,
    className = '',
    bodyClassName = '',
    children
  }: {
    id: string;
    title: string;
    description: string;
    badge?: string;
    focused?: boolean;
    className?: string;
    bodyClassName?: string;
    children: Snippet;
  } = $props();
</script>

<section
  id={id}
  data-settings-section={id}
  class={focused
    ? `flex h-full min-h-0 flex-col gap-6 space-y-0 scroll-mt-8 ${className}`
    : `space-y-6 scroll-mt-8 ${className}`}
>
  <div class="flex items-start justify-between gap-4 border-b border-border/60 pb-5">
    <div class="min-w-0 flex-1 space-y-2">
      <h2 class="flex flex-wrap items-center gap-2 text-2xl leading-tight font-semibold text-foreground">
        {title}
        {#if badge}
          <span
            class="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium tracking-[0.05em] text-muted-foreground uppercase"
          >
            {badge}
          </span>
        {/if}
      </h2>
      <p class="max-w-3xl text-sm leading-6 text-muted-foreground">{description}</p>
    </div>
  </div>
  <div
    class={focused
      ? `min-h-0 flex-1 overflow-hidden rounded-xl border border-border/50 bg-card/50 px-7 py-6 shadow-xs ${bodyClassName}`
      : `rounded-xl border border-border/50 bg-card/50 px-7 py-6 shadow-xs ${bodyClassName}`}
  >
    {@render children()}
  </div>
</section>
