<script module lang="ts">
  let fontPickerInstance = 0;
</script>

<script lang="ts">
  // Orca 1.4.214 components/settings/appearance/fonts/FontAutocomplete.tsx (MIT, © 2026 Lovecast Inc.)
  // Minnow cannot enumerate installed fonts without a backend command, so this keeps the
  // source combobox shape and offers the reference fallback families plus free-text entry.
  import { onMount, untrack } from 'svelte';
  import { Check, ChevronsUpDown, CircleX } from 'lucide-svelte';

  let {
    value,
    suggestions = [],
    placeholder = 'Cascadia Code',
    ariaLabel,
    defaultValue,
    onchange
  }: {
    value: string;
    suggestions?: readonly string[];
    placeholder?: string;
    ariaLabel?: string;
    defaultValue?: string;
    onchange: (value: string) => void;
  } = $props();

  const FALLBACK_FONTS = [
    'Cascadia Code',
    'Cascadia Mono',
    'Consolas',
    'Lucida Console',
    'Courier New',
    'JetBrains Mono',
    'Fira Code',
    'Source Code Pro',
    'IBM Plex Mono',
    'DejaVu Sans Mono',
    'Liberation Mono',
    'Ubuntu Mono',
    'Noto Sans Mono',
    'SF Mono',
    'Menlo',
    'Monaco'
  ];

  const MENU_MAX_HEIGHT = 256;
  const listId = `font-picker-listbox-${++fontPickerInstance}`;

  // Both start from the first committed value; the effect below syncs later prop changes.
  let query = $state(untrack(() => value));
  let previousValue = untrack(() => value);
  let open = $state(false);
  let highlighted = $state(-1);
  let root: HTMLDivElement | undefined = $state();
  let input: HTMLInputElement | undefined = $state();
  let menu: HTMLDivElement | undefined = $state();
  let position = $state<{
    left: number;
    top: number;
    width: number;
    listMaxHeight: number;
  } | null>(null);

  $effect(() => {
    if (value !== previousValue) {
      previousValue = value;
      query = value;
    }
  });

  $effect(() => {
    if (open) {
      const target = highlighted;
      if (target >= 0) {
        document
          .getElementById(`${listId}-option-${target}`)
          ?.scrollIntoView({ block: 'nearest' });
      }
    }
  });

  let allFonts = $derived(
    Array.from(new Set([...(value ? [value] : []), ...suggestions, ...FALLBACK_FONTS].filter(Boolean)))
  );

  let visibleFonts = $derived.by(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return allFonts.slice(0, 320);
    const starts: string[] = [];
    const includes: string[] = [];
    for (const font of allFonts) {
      const normalizedFont = font.toLowerCase();
      if (normalizedFont.startsWith(normalized)) starts.push(font);
      else if (normalizedFont.includes(normalized)) includes.push(font);
    }
    return [...starts, ...includes].slice(0, 320);
  });

  function measure(): void {
    if (!input) return;
    const rect = input.getBoundingClientRect();
    const width = Math.max(rect.width, 220);
    const below = window.innerHeight - rect.bottom - 12;
    const above = rect.top - 12;
    const placeAbove = below < 160 && above > below;
    const available = Math.max(96, placeAbove ? above : below);
    position = {
      left: Math.max(8, Math.min(rect.left, window.innerWidth - width - 8)),
      top: placeAbove ? Math.max(8, rect.top - 4 - available) : rect.bottom + 4,
      width,
      // Reserve the footer line inside the popover.
      listMaxHeight: Math.max(80, Math.min(MENU_MAX_HEIGHT, available - 48))
    };
  }

  function showMenu(): void {
    measure();
    open = true;
  }

  function closeMenu(): void {
    open = false;
    highlighted = -1;
  }

  function commit(next: string): void {
    const trimmed = next.trim();
    if (!trimmed) {
      query = value;
      return;
    }
    query = trimmed;
    if (trimmed !== value) onchange(trimmed);
  }

  function choose(font: string): void {
    query = font;
    onchange(font);
    closeMenu();
    input?.focus();
  }

  onMount(() => {
    function onPointerDown(event: PointerEvent): void {
      const target = event.target as Node | null;
      if (!target) return;
      if (root?.contains(target) || menu?.contains(target)) return;
      if (open) {
        commit(query);
        closeMenu();
      }
    }
    function onViewportChange(event?: Event): void {
      if (!open) return;
      const target = event?.target;
      if (target instanceof Node && menu?.contains(target)) return;
      commit(query);
      closeMenu();
    }
    document.addEventListener('pointerdown', onPointerDown, true);
    window.addEventListener('resize', onViewportChange);
    window.addEventListener('scroll', onViewportChange, true);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true);
      window.removeEventListener('resize', onViewportChange);
      window.removeEventListener('scroll', onViewportChange, true);
    };
  });
</script>

<div bind:this={root} class="relative">
  <input
    bind:this={input}
    type="text"
    value={query}
    {placeholder}
    aria-label={ariaLabel}
    role="combobox"
    aria-autocomplete="list"
    aria-expanded={open}
    aria-controls={open ? listId : undefined}
    aria-activedescendant={open && highlighted >= 0 ? `${listId}-option-${highlighted}` : undefined}
    oninput={(event) => {
      query = event.currentTarget.value;
      showMenu();
      highlighted = -1;
    }}
    onfocus={() => showMenu()}
    onblur={() => {
      commit(query);
      closeMenu();
    }}
    onkeydown={(event) => {
      if (event.key === 'Escape') {
        if (open) {
          event.preventDefault();
          event.stopPropagation();
          query = value;
          closeMenu();
        }
        return;
      }
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        showMenu();
        if (visibleFonts.length > 0) {
          highlighted =
            highlighted < 0 ? 0 : Math.min(highlighted + 1, visibleFonts.length - 1);
        }
        return;
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault();
        showMenu();
        if (visibleFonts.length > 0) {
          highlighted =
            highlighted < 0 ? visibleFonts.length - 1 : Math.max(highlighted - 1, 0);
        }
        return;
      }
      if (event.key === 'Enter') {
        event.preventDefault();
        const picked = open && highlighted >= 0 ? visibleFonts[highlighted] : undefined;
        if (picked !== undefined) choose(picked);
        else {
          commit(query);
          closeMenu();
        }
      }
    }}
    class="h-9 w-full min-w-0 appearance-none rounded-md border border-input bg-transparent py-1 pr-18 pl-3 text-sm shadow-xs transition-[color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:bg-input/30"
  />
  <div class="absolute inset-y-0 right-2 flex items-center gap-1">
    {#if defaultValue !== undefined && query}
      <button
        type="button"
        aria-label="Reset font"
        title="Reset"
        onmousedown={(event) => event.preventDefault()}
        onclick={() => {
          query = defaultValue;
          onchange(defaultValue);
          showMenu();
          input?.focus();
        }}
        class="rounded-sm p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
      >
        <CircleX class="size-3.5" aria-hidden="true" />
      </button>
    {/if}
    <button
      type="button"
      aria-label="Toggle font suggestions"
      title="Fonts"
      onmousedown={(event) => event.preventDefault()}
      onclick={() => {
        if (open) closeMenu();
        else {
          showMenu();
          input?.focus();
        }
      }}
      class="rounded-sm p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
    >
      <ChevronsUpDown class="size-3.5" aria-hidden="true" />
    </button>
  </div>
</div>

{#if open && position}
  <div
    bind:this={menu}
    class="fixed z-[1000] overflow-hidden rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-lg"
    style={`left:${position.left}px; top:${position.top}px; width:${position.width}px;`}
  >
    <div
      class="overflow-y-auto scrollbar-sleek"
      style={`max-height:${position.listMaxHeight}px;`}
      role="listbox"
      id={listId}
      aria-label={ariaLabel ?? 'Fonts'}
    >
      {#if visibleFonts.length > 0}
        {#each visibleFonts as font, index (font)}
          <button
            type="button"
            id={`${listId}-option-${index}`}
            role="option"
            aria-selected={index === highlighted}
            onmousedown={(event) => event.preventDefault()}
            onmouseenter={() => {
              highlighted = index;
            }}
            onclick={() => choose(font)}
            class={`flex w-full items-center justify-between rounded-sm px-3 py-2 text-left text-sm transition-colors ${
              index === highlighted ? 'bg-accent text-accent-foreground' : 'hover:bg-muted/60'
            }`}
          >
            <span class="truncate" style={`font-family:${font}`}>{font}</span>
            {#if font === value}
              <Check class="ml-3 size-4 shrink-0" aria-hidden="true" />
            {/if}
          </button>
        {/each}
      {:else}
        <div class="px-3 py-3 text-sm text-muted-foreground">No matching fonts.</div>
      {/if}
    </div>
    <p class="border-t border-border px-3 py-2 text-xs text-muted-foreground">
      Installed fonts cannot be listed by this build. Type any installed font name to use it.
    </p>
  </div>
{/if}
