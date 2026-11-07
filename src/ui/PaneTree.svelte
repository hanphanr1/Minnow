<script lang="ts">
  import { getContext } from 'svelte';
  import {
    Maximize2,
    Minimize2,
    PanelLeft,
    PanelRight,
    SquareSplitHorizontal,
    SquareSplitVertical,
    SquareTerminal,
    X,
  } from 'lucide-svelte';
  import Terminal from '../Terminal.svelte';
  import WorkspaceTabStrip from './WorkspaceTabStrip.svelte';
  import {
    WS_API,
    WS_VIEW,
    clampRatio,
    tabFocusTerminal,
    tabHasTerminal,
    tabTerminalIds,
    type LayoutNode,
    type PaneNode,
    type SplitDirection,
    type WorkspaceApi,
    type WorkspaceTab,
    type WorkspaceView,
  } from './workspace-layout';

  interface Props {
    root: LayoutNode;
    showPanelStrips: boolean;
    sidebarVisible: boolean;
    rightSidebarVisible: boolean;
    onToggleSidebar: () => void;
    onToggleRightSidebar: () => void;
  }

  let {
    root,
    showPanelStrips,
    sidebarVisible,
    rightSidebarVisible,
    onToggleSidebar,
    onToggleRightSidebar,
  }: Props = $props();

  const view = getContext<WorkspaceView>(WS_VIEW);
  const api = getContext<WorkspaceApi>(WS_API);

  function tabLabel(tab: WorkspaceTab, fallback: string): string {
    return tab.title?.trim() || fallback;
  }

  function paneTitleSurface(): 'dark' | 'light' {
    if (typeof document !== 'undefined') {
      if (document.documentElement.classList.contains('dark')) return 'dark';
      if (document.documentElement.classList.contains('light')) return 'light';
    }
    return view.preferences.theme === 'light' ? 'light' : 'dark';
  }

  function paneTitleBackground(): string {
    return paneTitleSurface() === 'light' ? '#ffffff' : '#282c34';
  }

  /** Collapses a split tab's pane tree to the expanded pane; otherwise keeps the real tree. */
  function maximizedPaneRoot(tab: WorkspaceTab, tabActive: boolean): PaneNode {
    const value = view.maximizedPane;
    if (!tabActive || value === null || value.tabId !== tab.id) return tab.panes;
    if (value.terminalId !== tab.focusedTerminalId) return tab.panes;
    if (tabTerminalIds(tab).length < 2 || !tabHasTerminal(tab, value.terminalId)) return tab.panes;
    return { type: 'leaf', terminalId: value.terminalId };
  }

  function beginResize(
    event: PointerEvent,
    direction: SplitDirection,
    initialRatio: number,
    commit: (ratio: number) => void,
  ): void {
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    const container = handle.parentElement;
    if (!container) return;
    const first = handle.previousElementSibling as HTMLElement | null;
    const second = handle.nextElementSibling as HTMLElement | null;
    if (!first || !second) return;
    handle.setPointerCapture(event.pointerId);
    let rect = container.getBoundingClientRect();
    let latest: number | null = null;
    let observer: ResizeObserver | null = null;

    const onMove = (moveEvent: PointerEvent): void => {
      if (moveEvent.pointerId !== event.pointerId || !handle.hasPointerCapture(event.pointerId)) {
        return;
      }
      const ratio =
        direction === 'horizontal'
          ? (moveEvent.clientX - rect.left) / rect.width
          : (moveEvent.clientY - rect.top) / rect.height;
      const clamped = clampRatio(ratio);
      latest = clamped;
      first.style.flex = `${clamped} 1 0%`;
      second.style.flex = `${1 - clamped} 1 0%`;
    };

    const cleanup = (): void => {
      observer?.disconnect();
      observer = null;
      handle.removeEventListener('pointermove', onMove);
      handle.removeEventListener('pointerup', onUp);
      handle.removeEventListener('pointercancel', onCancel);
      handle.removeEventListener('lostpointercapture', onLostCapture);
      try {
        if (handle.hasPointerCapture(event.pointerId)) {
          handle.releasePointerCapture(event.pointerId);
        }
      } catch {
        // Chromium may have dropped capture when the split unmounted.
      }
    };

    const onUp = (upEvent: PointerEvent): void => {
      if (upEvent.pointerId !== event.pointerId) return;
      cleanup();
      if (latest !== null) commit(latest);
    };

    const resetPaneSizes = (): void => {
      first.style.flex = `${initialRatio} 1 0%`;
      second.style.flex = `${1 - initialRatio} 1 0%`;
    };

    const onCancel = (cancelEvent: PointerEvent): void => {
      if (cancelEvent.pointerId !== event.pointerId) return;
      cleanup();
      resetPaneSizes();
    };

    const onLostCapture = (lostEvent: PointerEvent): void => {
      if (lostEvent.pointerId !== event.pointerId) return;
      cleanup();
      resetPaneSizes();
    };

    handle.addEventListener('pointermove', onMove);
    handle.addEventListener('pointerup', onUp);
    handle.addEventListener('pointercancel', onCancel);
    handle.addEventListener('lostpointercapture', onLostCapture);
    // A pane resize must not be lost if the container remeasures mid-drag.
    observer = new ResizeObserver(() => {
      rect = container.getBoundingClientRect();
    });
    observer.observe(container);
  }
</script>

{#snippet renderNode(
  node: LayoutNode,
  path: string,
  touchesLeft: boolean,
  touchesRight: boolean,
  touchesBottom: boolean,
  touchesTop: boolean,
)}
  {#if node.type === 'leaf'}
    {@render renderGroup(node.groupId, touchesLeft, touchesRight, touchesBottom, touchesTop)}
  {:else}
    {@const horizontal = node.direction === 'horizontal'}
    <div
      class="flex flex-1 min-h-0 min-w-0 overflow-hidden"
      style="flex-direction: {horizontal ? 'row' : 'column'}"
    >
      <div class="flex min-h-0 min-w-0 overflow-hidden" style:flex={`${node.ratio} 1 0%`}>
        {@render renderNode(
          node.first,
          path.length > 0 ? `${path}.first` : 'first',
          touchesLeft,
          horizontal ? false : touchesRight,
          horizontal ? touchesBottom : false,
          touchesTop,
        )}
      </div>
      <div
        class="tab-group-split-resize-handle {horizontal ? 'is-vertical' : 'is-horizontal'}"
        role="separator"
        aria-orientation={horizontal ? 'vertical' : 'horizontal'}
        aria-label="Resize panes"
        onpointerdown={(event) =>
          beginResize(event, node.direction, node.ratio, (ratio) => api.setRatio(path, ratio))}
      ></div>
      <div class="flex min-h-0 min-w-0 overflow-hidden" style:flex={`${1 - node.ratio} 1 0%`}>
        {@render renderNode(
          node.second,
          path.length > 0 ? `${path}.second` : 'second',
          horizontal ? false : touchesLeft,
          touchesRight,
          touchesBottom,
          horizontal ? touchesTop : false,
        )}
      </div>
    </div>
  {/if}
{/snippet}

{#snippet renderTabPanes(
  tab: WorkspaceTab,
  node: PaneNode,
  path: string,
  tabActive: boolean,
  groupId: string,
)}
  {#if node.type === 'leaf'}
    {@const terminal = view.terminalById.get(node.terminalId)}
    {#if terminal}
      {@const paneFocused =
        tabActive &&
        view.activeGroupId === groupId &&
        tabFocusTerminal(tab) === node.terminalId}
      {@const maximizedPane = view.maximizedPane}
      {@const paneExpanded =
        view.maximizedGroupId === groupId
          ? true
          : tabTerminalIds(tab).length > 1 &&
            maximizedPane !== null &&
            maximizedPane.tabId === tab.id &&
            maximizedPane.terminalId === node.terminalId}
      <div
        class="relative flex min-h-0 min-w-0 flex-1 overflow-hidden {view.preferences.paneTitles
          ? 'pt-[24px]'
          : ''}"
        data-pane-terminal={node.terminalId}
        data-active-pane={paneFocused ? '' : undefined}
        role="application"
        aria-label="Terminal pane"
        onpointerdown={() => api.activateTerminal(node.terminalId)}
        onfocusin={() => api.focusPane(node.terminalId)}
        oncontextmenu={(event) => api.openPaneMenu(event, tab.id, node.terminalId)}
      >
        {#key terminal.id}
        <Terminal
          {terminal}
          active={paneFocused && view.allowTerminalFocus}
          preferences={view.preferences}
          onActivate={(terminalId) => api.focusPane(terminalId)}
          onError={(message) => view.onError(message)}
          onReady={(handle, identity, terminalId) =>
            api.registerTerminal(terminalId, handle, identity)}
        />
        {/key}
        <div
          class="pane-title-bar"
          data-chromeless={view.preferences.paneTitles ? undefined : ''}
          data-pane-title-surface={paneTitleSurface()}
          data-active-pane={paneFocused ? '' : undefined}
          style="--orca-pane-title-bg: {paneTitleBackground()}"
        >
          {#if view.preferences.paneTitles}
            <button
              type="button"
              class="pane-title-text"
              title="Rename tab"
              onclick={() => api.beginRename(tab.id)}
            >
              {tabLabel(tab, terminal.name)}
            </button>
          {/if}
          <div class="pane-title-actions">
            <button
              type="button"
              class="pane-title-split-trigger"
              aria-label="Split terminal right"
              title="Split terminal right (Ctrl+Shift+D)"
              onclick={(event) => {
                event.stopPropagation();
                api.splitWithNewTerminal(groupId, 'right');
              }}
            >
              <SquareSplitVertical size={12} />
            </button>
            <button
              type="button"
              class="pane-title-split-trigger"
              aria-label="Split terminal down"
              title="Split terminal down (Alt+Shift+D)"
              onclick={(event) => {
                event.stopPropagation();
                api.splitWithNewTerminal(groupId, 'down');
              }}
            >
              <SquareSplitHorizontal size={12} />
            </button>
            <button
              type="button"
              class="pane-title-split-trigger"
              aria-label={paneExpanded ? 'Collapse pane' : 'Expand pane'}
              title={paneExpanded ? 'Collapse pane' : 'Expand pane'}
              onclick={(event) => {
                event.stopPropagation();
                api.toggleMaximize(groupId);
              }}
            >
              {#if paneExpanded}
                <Minimize2 size={12} />
              {:else}
                <Maximize2 size={12} />
              {/if}
            </button>
            {#if tabTerminalIds(tab).length > 1}
              <button
                type="button"
                class="pane-title-close"
                aria-label="Close pane"
                title="Close pane"
                onclick={(event) => {
                  event.stopPropagation();
                  api.closePane(tab.id, node.terminalId);
                }}
              >
                <X size={12} />
              </button>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  {:else}
    {@const horizontal = node.direction === 'horizontal'}
    <div
      class="flex flex-1 min-h-0 min-w-0 overflow-hidden"
      style="flex-direction: {horizontal ? 'row' : 'column'}"
    >
      <div class="flex min-h-0 min-w-0 overflow-hidden" style:flex={`${node.ratio} 1 0%`}>
        {@render renderTabPanes(
          tab,
          node.first,
          path.length > 0 ? `${path}.first` : 'first',
          tabActive,
          groupId,
        )}
      </div>
      <div
        class="tab-group-split-resize-handle {horizontal ? 'is-vertical' : 'is-horizontal'}"
        role="separator"
        aria-orientation={horizontal ? 'vertical' : 'horizontal'}
        aria-label="Resize terminal panes"
        onpointerdown={(event) =>
          beginResize(event, node.direction, node.ratio, (ratio) =>
            api.setPaneRatio(tab.id, path, ratio))}
      ></div>
      <div class="flex min-h-0 min-w-0 overflow-hidden" style:flex={`${1 - node.ratio} 1 0%`}>
        {@render renderTabPanes(
          tab,
          node.second,
          path.length > 0 ? `${path}.second` : 'second',
          tabActive,
          groupId,
        )}
      </div>
    </div>
  {/if}
{/snippet}

{#snippet renderGroup(
  groupId: string,
  touchesLeft: boolean,
  touchesRight: boolean,
  touchesBottom: boolean,
  touchesTop: boolean,
)}
  {@const group = view.snapshot.groups.find((candidate) => candidate.id === groupId) ?? null}
  {@const activeId = group?.activeTabId ?? null}
  {@const focused = view.activeGroupId === groupId}
  {@const drag = view.ui.drag}
  {@const dropZone =
    drag?.active && drag.target?.groupId === groupId ? drag.target.zone : null}
  <div
    class="group/tab-group relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border-border {touchesLeft
      ? 'border-l'
      : ''} {touchesRight ? 'border-r' : ''} {touchesBottom ? 'border-b' : ''} {focused &&
    touchesBottom
      ? 'border-b-accent'
      : ''}"
    style:opacity={view.hasSplits && !focused ? 0.95 : 1}
  >
    {#if showPanelStrips}
      {#if touchesTop}
        <div
          class="h-[4px] shrink-0 bg-tab-bar"
          data-tauri-drag-region
          aria-hidden="true"
        ></div>
      {/if}
      <div
        class="@container/tab-strip h-[32px] shrink-0 border-b border-border bg-tab-bar"
        data-tab-group-strip-id={groupId}
        data-terminal-focus-release-surface="true"
      >
        <div class="flex h-full items-stretch pr-1.5">
          {#if touchesTop && touchesLeft && !sidebarVisible}
            <button
              type="button"
              class="mn-icon-button ml-1 self-center"
              aria-label="Show workspace sidebar"
              title="Show workspace sidebar (Ctrl+B)"
              onclick={onToggleSidebar}
            >
              <PanelLeft size={16} />
            </button>
          {/if}
          <div class="h-full min-w-0 flex-1">
            <WorkspaceTabStrip {groupId} variant="panel" />
          </div>
          {#if touchesTop && touchesRight && !rightSidebarVisible}
            <button
              type="button"
              class="mn-icon-button mr-0.5 self-center"
              aria-label="Toggle right sidebar"
              title="Toggle right sidebar (Ctrl+L)"
              onclick={onToggleRightSidebar}
            >
              <PanelRight size={16} />
            </button>
            <div class="window-controls-titlebar-spacer" aria-hidden="true"></div>
          {/if}
        </div>
      </div>
    {/if}

    <div
      class="relative min-h-0 flex-1 overflow-hidden"
      data-drop-group={groupId}
      role="tabpanel"
      aria-label="Terminal pane"
      tabindex="-1"
      onpointerdown={() => api.focusGroup(groupId)}
      onfocusin={() => api.focusGroup(groupId)}
      oncontextmenu={(event) => api.openTerminalMenu(event, groupId)}
    >
      {#if group && group.tabIds.length > 0}
        {#each group.tabIds as tabId (tabId)}
          {@const tab = view.tabById.get(tabId)}
          {#if tab}
            {@const active = activeId === tabId}
            {@const paneRoot = maximizedPaneRoot(tab, active)}
            <div class="absolute inset-0 flex {active ? '' : 'hidden'}" data-tab-pane={tabId}>
              {@render renderTabPanes(tab, paneRoot, '', active, groupId)}
            </div>
          {/if}
        {/each}
      {:else}
        <div
          class="flex h-full flex-col items-center justify-center gap-3 p-6 text-center text-sm text-muted-foreground"
        >
          <SquareTerminal size={22} class="text-muted-foreground/70" />
          <p class="m-0">No terminals in this pane.</p>
          <div class="flex flex-wrap items-center justify-center gap-2">
            <button type="button" class="mn-button" onclick={() => api.createTerminal()}>
              New Terminal
            </button>
            {#if view.snapshot.closedTabs.length > 0}
              <button type="button" class="mn-button" onclick={() => api.reopenClosedTab()}>
                Reopen closed tab
              </button>
            {/if}
          </div>
        </div>
      {/if}

      {#if dropZone === 'center'}
        <div
          class="pointer-events-none absolute inset-1.5 z-30 rounded-md border-2 border-blue-500/70 bg-blue-500/5"
          aria-hidden="true"
        ></div>
      {:else if dropZone}
        <div
          class="pointer-events-none absolute z-30 border border-blue-500 bg-blue-500/10 {dropZone ===
          'left'
            ? 'inset-y-0 left-0 w-1/2'
            : dropZone === 'right'
              ? 'inset-y-0 right-0 w-1/2'
              : dropZone === 'up'
                ? 'inset-x-0 top-0 h-1/2'
                : 'inset-x-0 bottom-0 h-1/2'}"
          aria-hidden="true"
        ></div>
      {/if}
    </div>
  </div>
{/snippet}

{@render renderNode(
  root,
  '',
  view.hasSplits,
  view.hasSplits,
  view.hasSplits,
  showPanelStrips,
)}

<style>
  /* Ported from Orca assets/terminal.css and main.css. */
  .tab-group-split-resize-handle {
    position: relative;
    flex-shrink: 0;
    background: transparent;
  }

  .tab-group-split-resize-handle.is-vertical {
    width: 6px;
    cursor: col-resize;
  }

  .tab-group-split-resize-handle.is-horizontal {
    height: 6px;
    cursor: row-resize;
  }

  .tab-group-split-resize-handle::after {
    content: '';
    position: absolute;
    border-radius: 1px;
    background: var(--split-divider, #3f3f46);
    transition: background 100ms ease;
  }

  .tab-group-split-resize-handle.is-vertical::after {
    top: 0;
    bottom: 0;
    left: 50%;
    width: 3px;
    transform: translateX(-50%);
  }

  .tab-group-split-resize-handle.is-horizontal::after {
    left: 0;
    right: 0;
    top: 50%;
    height: 3px;
    transform: translateY(-50%);
  }

  .tab-group-split-resize-handle:hover::after {
    background: var(--split-divider-hover, #71717a);
  }

  .pane-title-bar[data-pane-title-surface='dark'] {
    --orca-pane-title-fg: var(--terminal-pane-title-on-dark-fg);
    --orca-pane-title-button-fg: var(--terminal-pane-title-on-dark-button-fg);
    --orca-pane-title-button-hover-fg: var(--terminal-pane-title-on-dark-button-hover-fg);
    --orca-pane-title-separator: var(--terminal-pane-title-on-dark-separator);
  }

  .pane-title-bar[data-pane-title-surface='light'] {
    --orca-pane-title-fg: var(--terminal-pane-title-on-light-fg);
    --orca-pane-title-button-fg: var(--terminal-pane-title-on-light-button-fg);
    --orca-pane-title-button-hover-fg: var(--terminal-pane-title-on-light-button-hover-fg);
    --orca-pane-title-separator: var(--terminal-pane-title-on-light-separator);
  }

  .pane-title-bar {
    position: absolute;
    top: 0;
    left: 0;
    z-index: 20;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    width: 100%;
    height: var(--orca-pane-title-height, 24px);
    padding: 0 8px;
    border-bottom: 1px solid var(--orca-pane-title-separator, rgb(255 255 255 / 0.06));
    background: var(--orca-pane-title-bg, #000);
    color: var(--orca-pane-title-fg, rgb(255 255 255 / 0.52));
    font-family: var(--font-mono);
    font-size: 13px;
    user-select: none;
  }

  .pane-title-text {
    align-self: stretch;
    display: block;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    margin: 0;
    padding: 0;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    line-height: var(--orca-pane-title-height, 24px);
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }

  .pane-title-text:focus-visible {
    outline: 1px solid var(--ring);
    outline-offset: 1px;
  }

  .pane-title-bar[data-chromeless] {
    padding: 0 2px;
    border-bottom: none;
    background: transparent;
    pointer-events: none;
  }

  .pane-title-bar[data-chromeless] .pane-title-actions {
    pointer-events: auto;
  }

  .pane-title-bar[data-chromeless] .pane-title-split-trigger,
  .pane-title-bar[data-chromeless] .pane-title-close {
    position: relative;
    width: 18px;
    min-width: 18px;
    height: var(--orca-pane-title-height, 24px);
    padding: 0;
  }

  .pane-title-bar[data-chromeless] .pane-title-split-trigger::before,
  .pane-title-bar[data-chromeless] .pane-title-close::before {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    width: 23px;
    height: 23px;
    border-radius: 9999px;
    background: radial-gradient(
      circle,
      var(--orca-pane-title-bg, var(--background)) 0 44%,
      color-mix(in srgb, var(--orca-pane-title-bg, var(--background)) 74%, transparent) 62%,
      transparent 100%
    );
    transform: translate(-50%, -50%);
  }


  .pane-title-actions {
    position: relative;
    z-index: 3;
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 0;
    margin-left: auto;
  }

  .pane-title-split-trigger,
  .pane-title-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    min-width: 18px;
    height: var(--orca-pane-title-height, 24px);
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--orca-pane-title-button-fg, rgb(255 255 255 / 0.3));
    opacity: 0;
    transition:
      opacity 120ms ease,
      color 120ms ease;
  }

  .pane-title-split-trigger:hover,
  .pane-title-close:hover {
    color: var(--orca-pane-title-button-hover-fg);
  }

  .pane-title-bar[data-active-pane] .pane-title-split-trigger,
  .pane-title-bar[data-active-pane] .pane-title-close,
  .pane-title-bar:hover .pane-title-split-trigger,
  .pane-title-bar:hover .pane-title-close,
  .pane-title-bar:focus-within .pane-title-split-trigger,
  .pane-title-bar:focus-within .pane-title-close {
    opacity: 1;
  }
</style>
