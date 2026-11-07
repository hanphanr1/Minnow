<script lang="ts">
  import { getContext, tick } from 'svelte';
  import {
    ChevronLeft,
    ChevronRight,
    Ellipsis,
    Minimize2,
    Pin,
    Plus,
    Play,
    SquareTerminal,
    X,
  } from 'lucide-svelte';
  import {
    WS_API,
    WS_VIEW,
    tabFocusTerminal,
    type WorkspaceApi,
    type WorkspaceTab,
    type WorkspaceView,
  } from './workspace-layout';

  interface Props {
    groupId: string;
    variant?: 'titlebar' | 'panel';
  }

  let { groupId, variant = 'titlebar' }: Props = $props();

  const view = getContext<WorkspaceView>(WS_VIEW);
  const api = getContext<WorkspaceApi>(WS_API);

  const group = $derived(view.snapshot.groups.find((candidate) => candidate.id === groupId) ?? null);
  const tabs: WorkspaceTab[] = $derived(
    (group?.tabIds ?? [])
      .map((id) => view.tabById.get(id))
      .filter((tab): tab is WorkspaceTab => tab !== undefined),
  );
  const activeTabId = $derived(group?.activeTabId ?? null);

  let strip = $state<HTMLDivElement | null>(null);
  let canLeft = $state(false);
  let canRight = $state(false);

  function updateOverflow(node: HTMLDivElement | null = strip): void {
    if (!node) return;
    canLeft = node.scrollLeft > 1;
    canRight = node.scrollLeft + node.clientWidth < node.scrollWidth - 1;
  }

  function trackOverflow(node: HTMLDivElement): { destroy: () => void } {
    const update = (): void => updateOverflow(node);
    node.addEventListener('scroll', update, { passive: true });
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(update);
    observer?.observe(node);
    update();
    return {
      destroy() {
        node.removeEventListener('scroll', update);
        observer?.disconnect();
      },
    };
  }

  function scrollStrip(direction: 1 | -1): void {
    strip?.scrollBy({ left: direction * 240, behavior: 'smooth' });
  }

  function labelFor(tab: WorkspaceTab): string {
    const terminal = view.terminalById.get(tab.terminalId);
    return tab.title?.trim() || terminal?.name || 'Terminal';
  }

  function statusClass(tab: WorkspaceTab): string {
    const terminal = view.terminalById.get(tabFocusTerminal(tab));
    if (terminal?.status === 'waiting-for-input') return 'text-amber-500';
    if (terminal?.status === 'exited') return 'text-muted-foreground/50';
    return 'text-muted-foreground';
  }

  function dropSide(index: number): 'left' | 'right' | null {
    const drag = view.ui.drag;
    if (!drag?.active || !drag.insertion || drag.insertion.groupId !== groupId) return null;
    if (drag.insertion.index === index) return 'left';
    if (index === tabs.length - 1 && drag.insertion.index >= tabs.length) return 'right';
    return null;
  }

  function focusInput(node: HTMLInputElement): void {
    node.focus();
    node.select();
  }

  $effect(() => {
    void tabs.length;
    void tick().then(() => updateOverflow());
  });
</script>

<div
  class="flex h-full min-w-0 flex-1 items-stretch overflow-hidden"
  role="toolbar"
  aria-label="Terminal tab strip"
  tabindex="-1"
  onpointerdown={() => api.focusGroup(groupId)}
>
  {#if canLeft}
    <button
      type="button"
      class="mx-0.5 my-auto flex h-6 w-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent/50 hover:text-foreground"
      aria-label="Scroll tabs left"
      title="Scroll tabs left"
      onclick={() => scrollStrip(-1)}
    >
      <ChevronLeft size={14} />
    </button>
  {/if}

  <div
    bind:this={strip}
    use:trackOverflow
    role="tablist"
    aria-label="Terminal tabs"
    class="terminal-tab-strip flex h-full w-max min-w-0 max-w-full shrink items-stretch overflow-x-auto overflow-y-hidden border-r border-border/70 scrollbar-none"
    onscroll={() => updateOverflow()}
    onwheel={(event) => {
      if (!strip) return;
      if (Math.abs(event.deltaY) > Math.abs(event.deltaX)) {
        strip.scrollLeft += event.deltaY;
      }
    }}
    data-tab-group-strip-id={groupId}
  data-variant={variant}
  >
    {#each tabs as tab, index (tab.id)}
      {@const active = tab.id === activeTabId}
      {@const side = dropSide(index)}
      <div
        class="w-[300px] max-w-[300px] min-w-[144px] flex-1 {active
          ? 'sticky inset-x-0 z-10'
          : ''}"
        data-tab-strip-slot={tab.id}
      >
        <div
          role="tab"
          tabindex={active ? 0 : -1}
          aria-selected={active}
          data-tab-id={tab.id}
          data-tab-title={labelFor(tab)}
          data-pinned={tab.pinned ? 'true' : 'false'}
          data-active={active ? 'true' : 'false'}
          class="group relative flex h-full w-full cursor-pointer select-none items-center border-r border-t border-border px-1.5 text-xs outline-none focus:outline-none focus-visible:outline-none {active
            ? 'bg-tab-active text-tab-active-foreground'
            : 'bg-tab-inactive text-tab-inactive-foreground hover:text-tab-active-foreground'} {index ===
          tabs.length - 1
            ? 'border-r-transparent'
            : ''}"
          onclick={() => {
            if (api.isClickSuppressed()) return;
            api.activateTab(tab.id);
          }}
          onkeydown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault();
              api.activateTab(tab.id);
            } else if (event.key === 'F2') {
              event.preventDefault();
              api.beginRename(tab.id);
            } else if (event.key === 'Delete' && !tab.pinned) {
              event.preventDefault();
              api.closeTab(tab.id);
            }
          }}
          onauxclick={(event) => {
            if (event.button !== 1) return;
            event.preventDefault();
            if (!tab.pinned) api.closeTab(tab.id);
          }}
          oncontextmenu={(event) => api.openTabMenu(event, tab)}
          ondblclick={(event) => {
            event.stopPropagation();
            if (view.ui.editingTabId !== tab.id) api.beginRename(tab.id);
          }}
          onpointerdown={(event) => api.beginTabDrag(event, tab.id)}
        >
          {#if active && tabs.length > 1}
            <span
              class="pointer-events-none absolute inset-x-0 bottom-0 z-20 h-[2px] bg-tab-active-indicator"
              aria-hidden="true"
            ></span>
          {/if}
          {#if side === 'left'}
            <span
              class="pointer-events-none absolute inset-y-0 left-0 z-10 w-[2px] bg-blue-500"
              aria-hidden="true"
            ></span>
          {:else if side === 'right'}
            <span
              class="pointer-events-none absolute inset-y-0 right-0 z-10 w-[2px] bg-blue-500"
              aria-hidden="true"
            ></span>
          {/if}
          <SquareTerminal size={13} class="mr-1 shrink-0 {statusClass(tab)}" aria-hidden="true" />
          {#if tab.pinned}
            <Pin size={12} class="mr-1 shrink-0 text-muted-foreground" aria-hidden="true" />
          {/if}
          {#if view.ui.editingTabId === tab.id}
            <input
              value={tab.title ?? ''}
              placeholder={labelFor(tab)}
              aria-label="Rename tab"
              spellcheck="false"
              use:focusInput
              class="mr-1 min-w-0 flex-1 rounded-sm border border-input bg-background px-1 py-0.5 text-xs text-foreground"
              onpointerdown={(event) => event.stopPropagation()}
              onmousedown={(event) => event.stopPropagation()}
              onclick={(event) => event.stopPropagation()}
              ondblclick={(event) => event.stopPropagation()}
              onauxclick={(event) => event.stopPropagation()}
              onkeydown={(event) => {
                event.stopPropagation();
                if (event.key === 'Enter' || event.key === 'Tab') {
                  event.preventDefault();
                  api.commitRename(tab.id, event.currentTarget.value);
                } else if (event.key === 'Escape') {
                  event.preventDefault();
                  api.cancelRename();
                }
              }}
              onblur={(event) => {
                if (view.ui.editingTabId === tab.id) {
                  api.commitRename(tab.id, event.currentTarget.value);
                }
              }}
            />
          {:else}
            <span class="tab-label mr-1 min-w-0 flex-1 truncate">{labelFor(tab)}</span>
          {/if}
          {#if tab.color && view.ui.editingTabId !== tab.id}
            <span
              class="mr-1.5 size-2 shrink-0 rounded-full"
              style="background-color: {tab.color}"
            ></span>
          {/if}
          {#if (view.maximizedGroupId === groupId || (view.maximizedPane?.tabId === tab.id && view.maximizedPane.terminalId === tabFocusTerminal(tab))) && active && view.ui.editingTabId !== tab.id}
            <button
              type="button"
              class="mr-1 flex size-4 shrink-0 items-center justify-center rounded-sm bg-transparent text-muted-foreground hover:bg-muted hover:text-foreground"
              title="Collapse pane"
              aria-label="Collapse pane"
              onpointerdown={(event) => event.stopPropagation()}
              onclick={(event) => {
                event.stopPropagation();
                api.toggleMaximize(groupId);
              }}
            >
              <Minimize2 size={12} />
            </button>
          {/if}
          {#if !tab.pinned && view.ui.editingTabId !== tab.id}
            <button
              type="button"
              class="relative z-10 flex size-4 shrink-0 items-center justify-center rounded-sm bg-transparent hover:bg-muted hover:text-foreground focus-visible:bg-muted focus-visible:text-foreground {active
                ? 'text-muted-foreground'
                : 'text-transparent group-hover:text-muted-foreground'}"
              aria-label="Close tab {labelFor(tab)}"
              title="Close tab"
              data-tab-close-button="true"
              onpointerdown={(event) => event.stopPropagation()}
              onmousedown={(event) => event.stopPropagation()}
              onclick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                api.closeTab(tab.id);
              }}
            >
              <X size={12} />
            </button>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  {#if canRight}
    <button
      type="button"
      class="mx-0.5 my-auto flex h-6 w-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent/50 hover:text-foreground"
      aria-label="Scroll tabs right"
      title="Scroll tabs right"
      onclick={() => scrollStrip(1)}
    >
      <ChevronRight size={14} />
    </button>
  {/if}

  <button
    type="button"
    class="ml-2 my-auto flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-accent/50 hover:text-foreground"
    title="New tab"
    aria-label="New tab"
    onclick={(event) => {
      api.openNewTabMenu(event.currentTarget.getBoundingClientRect());
    }}
  >
    <Plus size={14} />
  </button>

  <span class="min-w-0 flex-1"></span>
  <button
    type="button"
    class="my-auto ml-1 inline-flex h-7 shrink-0 items-center gap-1 rounded-md px-1.5 text-xs text-muted-foreground hover:bg-accent/50 hover:text-foreground"
    aria-label="Quick commands"
    title="Quick commands"
    onclick={event => api.openQuickCommandsMenu(event.currentTarget.getBoundingClientRect())}
  >
    <Play size={14}/>{#if variant === 'titlebar'}<span>Command</span>{/if}
  </button>

  {#if view.hasSplits}
    <button
      type="button"
      class="my-auto mr-1 ml-1.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-accent/50 hover:text-foreground"
      title="Pane actions"
      aria-label="Pane actions"
      onclick={(event) => {
        api.openPaneActionsMenu(event.currentTarget.getBoundingClientRect(), groupId);
      }}
    >
      <Ellipsis size={15} />
    </button>
  {/if}
</div>
