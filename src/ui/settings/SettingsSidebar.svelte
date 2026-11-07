<script lang="ts">
  // Orca 1.4.214 components/settings/navigation/SettingsSidebar.tsx (MIT, © 2026 Lovecast Inc.)
  import { onMount } from 'svelte';
  import { ArrowLeft, Circle, Search } from 'lucide-svelte';
  import { sectionById, type NavGroup, type NavSection } from './settings-data';

  const onboarding = sectionById('setup-guide');

  let {
    activeSectionId,
    query,
    groups,
    autoFocus = false,
    onQuery,
    onSelect,
    onBack
  }: {
    activeSectionId: string;
    query: string;
    groups: { group: NavGroup; sections: NavSection[] }[];
    autoFocus?: boolean;
    onQuery: (value: string) => void;
    onSelect: (id: string) => void;
    onBack: () => void;
  } = $props();

  const navItemClass =
    'flex w-full items-center gap-2 rounded-lg px-3 py-1.5 text-left text-[13px] outline-none transition-colors duration-150 focus-visible:ring-[3px] focus-visible:ring-worktree-sidebar-ring/50';
  const navItemActiveClass =
    'bg-worktree-sidebar-accent font-medium text-worktree-sidebar-accent-foreground ring-1 ring-worktree-sidebar-ring/25';
  const navItemIdleClass =
    'text-worktree-sidebar-foreground/60 hover:bg-worktree-sidebar-accent/60 hover:text-worktree-sidebar-foreground';

  let searchInput: HTMLInputElement | undefined = $state();
  let navList: HTMLDivElement | undefined = $state();

  // Reveal the selected row on mount and after every section change, without scrolling the page.
  $effect(() => {
    const id = activeSectionId;
    const list = navList;
    if (!list) return;
    list
      .querySelector<HTMLElement>(`[data-section-id="${id}"]`)
      ?.scrollIntoView({ block: 'nearest' });
  });

  onMount(() => {
    // Source hint: Settings search shows Ctrl + F. Keep the hint honest by handling it here.
    function onKeydown(event: KeyboardEvent): void {
      if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'f') {
        event.preventDefault();
        // Settings owns Ctrl+F while open; keep the app-level terminal search from also firing.
        event.stopPropagation();
        searchInput?.focus();
        searchInput?.select();
      }
    }
    document.addEventListener('keydown', onKeydown, true);
    if (autoFocus) searchInput?.focus();
    return () => document.removeEventListener('keydown', onKeydown, true);
  });
</script>

<aside
  class="flex w-[280px] shrink-0 flex-col border-r border-worktree-sidebar-border bg-worktree-sidebar"
>
  <div class="border-b border-worktree-sidebar-border px-3 py-3">
    <button
      type="button"
      onclick={onBack}
      class="inline-flex h-8 w-full shrink-0 cursor-pointer items-center justify-start gap-2 rounded-md px-3 text-[13px] font-medium whitespace-nowrap text-muted-foreground transition-all outline-none hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:hover:bg-accent/50 [&_svg]:size-4 [&_svg]:shrink-0"
    >
      <ArrowLeft aria-hidden="true" />
      Back to app
    </button>
  </div>

  <div class="border-b border-worktree-sidebar-border px-3 py-3">
    <div class="relative">
      <Search
        class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
        aria-hidden="true"
      />
      <input
        bind:this={searchInput}
        type="text"
        value={query}
        oninput={(event) => onQuery(event.currentTarget.value)}
        placeholder="Search settings"
        aria-label="Search settings"
        class="h-9 w-full min-w-0 appearance-none rounded-md border border-input bg-background/60 py-1 pr-16 pl-9 text-[13px] shadow-xs transition-[color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:bg-input/30"
      />
      {#if query === ''}
        <span class="pointer-events-none absolute top-1/2 right-2 flex -translate-y-1/2 items-center gap-1">
          <span
            class="inline-flex min-w-6 items-center justify-center rounded border border-border/80 bg-secondary/70 px-1.5 py-0.5 text-xs font-medium text-muted-foreground shadow-sm"
          >
            Ctrl
          </span>
          <span class="mx-0.5 text-xs text-muted-foreground">+</span>
          <span
            class="inline-flex min-w-6 items-center justify-center rounded border border-border/80 bg-secondary/70 px-1.5 py-0.5 text-xs font-medium text-muted-foreground shadow-sm"
          >
            F
          </span>
        </span>
      {/if}
    </div>
  </div>

  {#if onboarding}
    <div class="border-b border-worktree-sidebar-border px-3 py-3">
      <button
        type="button"
        aria-current={activeSectionId === onboarding.id ? 'page' : undefined}
        onclick={() => onSelect(onboarding.id)}
        class={`flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[13px] outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-worktree-sidebar-ring/50 ${
          activeSectionId === onboarding.id
            ? 'bg-worktree-sidebar-accent font-medium text-worktree-sidebar-accent-foreground'
            : 'text-worktree-sidebar-foreground/60 hover:bg-worktree-sidebar-foreground/8 hover:text-worktree-sidebar-foreground'
        }`}
      >
        <Circle class="size-4 shrink-0" aria-hidden="true" />
        <span class="min-w-0 flex-1 truncate">{onboarding.title}</span>
      </button>
    </div>
  {/if}

  <div bind:this={navList} class="min-h-0 flex-1 overflow-y-auto scrollbar-sleek px-3 py-4">
    <div class="space-y-5">
      {#if groups.length === 0 && query.trim()}
        <p class="px-3 text-xs text-muted-foreground">No matching sections.</p>
      {/if}

      {#each groups as { group, sections } (group.id)}
        <div class="space-y-2">
          <p
            class="px-3 text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase"
          >
            {group.title}
          </p>
          <div class="space-y-1">
            {#each sections as section (section.id)}
              <button
                type="button"
                aria-current={activeSectionId === section.id ? 'page' : undefined}
                data-current={activeSectionId === section.id ? 'true' : undefined}
                data-section-id={section.id}
                onclick={() => onSelect(section.id)}
                class={`${navItemClass} ${
                  activeSectionId === section.id ? navItemActiveClass : navItemIdleClass
                }`}
              >
                <span class="shrink-0" aria-hidden="true">
                  <section.icon class="size-4" />
                </span>
                <span class="truncate">{section.title}</span>
                {#if section.badge}
                  <span
                    class="ml-auto rounded-full bg-muted px-1.5 py-0.5 text-[9px] font-medium tracking-wider text-muted-foreground uppercase"
                  >
                    {section.badge}
                  </span>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/each}

      {#if !query.trim()}
        <div class="space-y-2">
          <p class="px-3 text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
            Projects
          </p>
          <p class="px-3 text-xs text-muted-foreground">
            Project-scoped settings are not available in this build.
          </p>
        </div>
      {/if}
    </div>
  </div>
</aside>
