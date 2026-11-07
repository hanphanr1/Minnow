<!--
  Accessible sidebar menu. Ported from Orca's dropdown/context menu recipes
  (components/ui/dropdown-menu.tsx + context-menu.tsx, MIT): 11px panel radius,
  4px padding, 13px rows, arrow-key roving focus, Escape/Tab dismissal and
  pointer-outside close. Rendered fixed so sidebar scroll clipping cannot cut it.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Check } from 'lucide-svelte';
  import type { MenuAnchor, SidebarMenuItem } from './presentation';

  let {
    anchor,
    side = 'bottom',
    width = 208,
    label,
    items,
    restoreTo = null,
    onClose
  }: {
    anchor: MenuAnchor;
    side?: 'bottom' | 'top' | 'right';
    width?: number;
    label: string;
    items: SidebarMenuItem[];
    restoreTo?: HTMLElement | null;
    onClose: () => void;
  } = $props();

  let panel = $state<HTMLDivElement | null>(null);
  let left = $state(0);
  let top = $state(0);
  let ready = $state(false);
  let opener: HTMLElement | null = null;

  function itemButtons(): HTMLButtonElement[] {
    if (!panel) return [];
    return Array.from(panel.querySelectorAll<HTMLButtonElement>('button[role^="menuitem"]:not(:disabled)'));
  }

  function focusAt(index: number): void {
    const buttons = itemButtons();
    if (buttons.length === 0) return;
    buttons[(index + buttons.length) % buttons.length]?.focus({ preventScroll: true });
  }

  function place(): void {
    if (!panel) return;
    const rect = panel.getBoundingClientRect();
    const margin = 8;
    let x: number;
    let y: number;
    if (side === 'right') {
      x = (anchor.right ?? anchor.left) + 8;
      y = anchor.top;
      if (x + rect.width > window.innerWidth - margin) {
        x = anchor.left - rect.width - 8;
      }
    } else {
      x = anchor.left;
      y = side === 'top' ? anchor.top - rect.height - 6 : anchor.bottom + 6;
      if (y + rect.height > window.innerHeight - margin) y = anchor.top - rect.height - 6;
    }
    if (y < margin) y = margin;
    if (x + rect.width > window.innerWidth - margin) x = window.innerWidth - rect.width - margin;
    if (x < margin) x = margin;
    left = x;
    top = y;
    ready = true;
    focusAt(0);
  }

  function close(): void {
    onClose();
  }

  function handleDocumentScroll(event: Event): void {
    if (panel && event.target instanceof Node && panel.contains(event.target)) return;
    close();
  }

  function choose(item: SidebarMenuItem): void {
    if (item.disabled) return;
    close();
    item.onSelect();
  }

  function handleKeydown(event: KeyboardEvent): void {
    const buttons = itemButtons();
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        focusAt(index < 0 ? 0 : index + 1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        focusAt(index < 0 ? buttons.length - 1 : index - 1);
        break;
      case 'Home':
        event.preventDefault();
        focusAt(0);
        break;
      case 'End':
        event.preventDefault();
        focusAt(buttons.length - 1);
        break;
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        close();
        break;
      case 'Tab':
        event.preventDefault();
        close();
        break;
      default:
        break;
    }
  }

  onMount(() => {
    opener = restoreTo ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
    requestAnimationFrame(place);
    window.addEventListener('resize', close);
    window.addEventListener('scroll', handleDocumentScroll, true);
    return () => {
      window.removeEventListener('resize', close);
      window.removeEventListener('scroll', handleDocumentScroll, true);
    };
  });

  onDestroy(() => {
    if (opener && document.contains(opener)) opener.focus({ preventScroll: true });
  });
</script>

<div
  class="fixed inset-0 z-[89]"
  role="presentation"
  onpointerdown={close}
  oncontextmenu={(event) => {
    event.preventDefault();
    close();
  }}
></div>

<div
  bind:this={panel}
  class="sb-menu scrollbar-sleek fixed z-[91] max-h-[min(70vh,480px)] overflow-x-hidden overflow-y-auto rounded-[11px] border border-black/10 bg-[rgba(255,255,255,0.78)] p-[4px] text-black shadow-[0_6px_16px_rgba(0,0,0,0.14),inset_0_1px_0_rgba(255,255,255,0.14)] backdrop-blur-xl backdrop-saturate-150 dark:border-black/60 dark:bg-popover/75 dark:text-white dark:shadow-[0_16px_34px_rgba(0,0,0,0.40),inset_0_0_0_1px_rgba(255,255,255,0.12)]"
  style:left={`${left}px`}
  style:top={`${top}px`}
  style:width={`${width}px`}
  style:opacity={ready ? 1 : 0}
  role="menu"
  aria-label={label}
  tabindex="-1"
  onkeydown={handleKeydown}
>
  {#each items as item (item.id)}
    {#if item.separatorBefore}
      <div class="mx-2.5 my-[5px] h-px bg-black/10 dark:bg-white/12" role="separator"></div>
    {/if}
    {@const ItemIcon = item.icon}
    <button
      type="button"
      role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
      aria-checked={item.checked === undefined ? undefined : item.checked}
      tabindex="-1"
      disabled={item.disabled}
      class={`relative flex w-full items-center gap-2 rounded-md px-2.5 py-1 text-left text-[13px] leading-4 font-normal outline-none select-none transition-colors disabled:pointer-events-none disabled:opacity-40 ${
        item.danger
          ? 'text-destructive focus:bg-destructive/10 focus:text-destructive'
          : 'text-popover-foreground focus:bg-black/8 focus:text-accent-foreground dark:focus:bg-white/14'
      }`}
      onclick={() => choose(item)}
    >
      {#if item.checked !== undefined}
        <span class="flex size-3.5 shrink-0 items-center justify-center">
          {#if item.checked}<Check class="size-3.5" />{/if}
        </span>
      {:else if ItemIcon}
        <ItemIcon class="size-3.5 shrink-0" />
      {/if}
      <span class="min-w-0 flex-1 truncate">{item.label}</span>
      {#if item.shortcut}
        <span class="ml-auto shrink-0 whitespace-nowrap pl-4 text-xs tracking-normal text-muted-foreground">{item.shortcut}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .sb-menu {
    animation: sb-menu-in 100ms ease-out;
    transform-origin: top left;
  }

  @keyframes sb-menu-in {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .sb-menu {
      animation: none;
    }
  }
</style>
