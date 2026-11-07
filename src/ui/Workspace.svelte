<script lang="ts">
  import { onDestroy, onMount, setContext, tick, untrack } from 'svelte';
  import {
    ChevronLeft,
    ChevronRight,
    Clipboard,
    Copy,
    Eraser,
    ExternalLink,
    GitCompare,
    GitMerge,
    Maximize2,
    Minimize2,
    PanelLeft,
    PanelRight,
    PanelRightClose,
    PanelsTopLeft,
    Pencil,
    Pin,
    PinOff,
    RefreshCw,
    RotateCcw,
    Search,
    Square,
    SquareSplitHorizontal,
    SquareSplitVertical,
    SquareTerminal,
    TextSelect,
    Trash2,
    X,
  } from 'lucide-svelte';
  import type { Project, TerminalKind, TerminalSession, Worker } from '../types';
  import { errorMessage } from '../api';
  import { workspacePathMovedEvent, type PathMoved } from '../workspace-api';
  import type { UiPreferences } from './preferences';
  import PaneTree from './PaneTree.svelte';
  import WorkspaceTabStrip from './WorkspaceTabStrip.svelte';
  import {
    TAB_COLORS,
    WS_API,
    WS_VIEW,
    addDocumentTab,
    addTerminalTab,
    adjacentGroupId,
    adjacentTabId,
    clampRatio,
    clonePane,
    collapseGroup,
    createSnapshot,
    equalizePane,
    equalizeLayout,
    groupById,
    groupContainingTab,
    isDocumentTab,
    isPaneMaximizeValid,
    isTerminalLive,
    isTextEntryTarget,
    loadSnapshot,
    markRecent,
    moveTab,
    moveTabToNewGroup as splitTabToNewGroup,
    paneTerminalIds,
    pushClosedTab,
    reconcileTerminals,
    removePaneFromTab,
    removeTab,
    saveSnapshot,
    splitPane,
    tabById as findTabById,
    tabByTerminal as findTabForTerminal,
    tabForDocument,
    tabFocusTerminal,
    tabHasTerminal,
    tabTerminalIds,
    updatePaneRatio,
    updateRatio,
    type ClosedTabEntry,
    type DragState,
    type DropPosition,
    type EditorHandle,
    type LayoutNode,
    type TabDocument,
    type TabInsertion,
    type TerminalHandle,
    type WorkspaceApi,
    type WorkspaceSnapshot,
    type WorkspaceTab,
    type WorkspaceUi,
    type WorkspaceView,
    type WorkerAction,
  } from './workspace-layout';

  interface Props {
    project: Project;
    /** The selected worktree. File and Git panels follow it, not the focused terminal. */
    worker: Worker;
    /** This worktree's terminals, including closed records kept for reopen. */
    terminals: TerminalSession[];
    preferences: UiPreferences;
    sidebarVisible: boolean;
    rightSidebarVisible?: boolean;
    onActivateTerminal: (terminalId: string) => void;
    onCreateTerminal: (kind: TerminalKind, sourceTerminalId: string | null) => Promise<TerminalSession>;
    onRestartTerminal: (terminalId: string) => Promise<TerminalSession>;
    onStopTerminal: (terminalId: string) => Promise<TerminalSession>;
    onCloseTerminal: (terminalId: string) => Promise<TerminalSession>;
    onAction: (action: WorkerAction, worker: Worker) => void;
    onStopWorker?: (worker: Worker) => Promise<void>;
    onFeature: (name: string) => void;
    onError: (message: string) => void;
    onToggleSidebar: () => void;
    onToggleRightSidebar: () => void;
  }

  let {
    project,
    worker,
    terminals,
    preferences,
    sidebarVisible,
    rightSidebarVisible = true,
    onActivateTerminal,
    onCreateTerminal,
    onRestartTerminal,
    onStopTerminal,
    onCloseTerminal,
    onAction,
    onStopWorker,
    onFeature,
    onError,
    onToggleSidebar,
    onToggleRightSidebar,
  }: Props = $props();

  type MenuIcon = typeof SquareTerminal;

  type MenuEntry =
    | {
        kind: 'item';
        label: string;
        icon?: MenuIcon;
        shortcut?: string;
        danger?: boolean;
        disabled?: boolean;
        onSelect: () => void;
      }
    | { kind: 'submenu'; label: string; icon: MenuIcon; items: MenuEntry[] }
    | { kind: 'separator' }
    | { kind: 'colors'; selected: string | null; onPick: (value: string | null) => void };

  interface MenuState {
    x: number;
    y: number;
    width: number;
    items: MenuEntry[];
  }

  interface ConfirmState {
    title: string;
    message: string;
    confirmLabel: string;
    danger: boolean;
    onConfirm: () => void | Promise<void>;
  }

  let snapshot = $state<WorkspaceSnapshot>(createSnapshot([], null));
  let ui = $state<WorkspaceUi>({ drag: null, editingTabId: null, toast: null });
  let menu = $state<MenuState | null>(null);
  let recent = $state<{ ids: string[]; index: number } | null>(null);
  let confirmState = $state<ConfirmState | null>(null);
  let menuElement = $state<HTMLElement | null>(null);
  let subMenuElement = $state<HTMLElement|null>(null);
  let subMenu = $state<(MenuState & {label:string})|null>(null);
  let subMenuOpener: HTMLButtonElement|null = null;
  let confirmElement = $state<HTMLElement | null>(null);
  let confirmCancelButton = $state<HTMLButtonElement | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  let suppressClick = false;
  let suppressClickTimer: ReturnType<typeof setTimeout> | null = null;
  /** The project/worktree the current layout snapshot describes. Replaced, never mutated. */
  let scope: { projectId: string; terminalId: string } | null = null;
  /** Layout work that spans an async terminal call. Automatic tabbing waits for it to end. */
  let layoutBusy = 0;

  const terminalHandles = new Map<string, TerminalHandle>();
  /** Open file editors by tab id; each owns its own dirty document. */
  const editors = new Map<string, EditorHandle>();
  let dirtyTabIds = $state<Set<string>>(new Set());

  const terminalById = $derived(new Map(terminals.map((terminal) => [terminal.id, terminal])));
  const tabById = $derived(new Map(snapshot.tabs.map((tab) => [tab.id, tab])));
  const activeGroup = $derived(groupById(snapshot, snapshot.activeGroupId));
  const activeTab = $derived(
    activeGroup?.activeTabId ? (tabById.get(activeGroup.activeTabId) ?? null) : null,
  );
  const activeTerminal = $derived(
    activeTab ? (terminalById.get(tabFocusTerminal(activeTab)) ?? null) : null,
  );
  const hasSplits = $derived(snapshot.layout.type === 'split');
  const maximizedGroupId = $derived(snapshot.maximizedGroupId);

  const view: WorkspaceView = {
    get snapshot() {
      return snapshot;
    },
    get ui() {
      return ui;
    },
    get preferences() {
      return preferences;
    },
    get worker() {
      return worker;
    },
    get terminalById() {
      return terminalById;
    },
    get tabById() {
      return tabById;
    },
    get dirtyTabIds() {
      return dirtyTabIds;
    },
    get activeGroupId() {
      return snapshot.activeGroupId;
    },
    get hasSplits() {
      return hasSplits;
    },
    get maximizedGroupId() {
      return maximizedGroupId;
    },
    get maximizedPane() {
      return snapshot.maximizedPane;
    },
    get sidebarVisible() {
      return sidebarVisible;
    },
    get allowTerminalFocus() {
      return menu === null && confirmState === null && recent === null && ui.editingTabId === null;
    },
    get activeTab() {
      return activeTab;
    },
    get activeTerminal() {
      return activeTerminal;
    },
    get onError() {
      return onError;
    },
  };

  function persist(): void {
    if (scope !== null) saveSnapshot(scope.projectId, scope.terminalId, snapshot);
  }

  /** Drops stale panes and adds tabs for live terminals that have none. */
  function reconcile(): void {
    if (scope === null || layoutBusy > 0) return;
    // Terminal status events arrive constantly; only write storage when the layout really changed.
    const before = JSON.stringify(snapshot);
    reconcileTerminals(snapshot, terminals);
    if (JSON.stringify(snapshot) !== before) persist();
  }

  function beginLayoutWork(): void {
    layoutBusy += 1;
  }

  function endLayoutWork(): void {
    layoutBusy -= 1;
    if (layoutBusy === 0) void tick().then(reconcile);
  }

  function documentLabel(document: TabDocument): string {
    const name = (path: string): string => path.split('/').filter(Boolean).pop() ?? path;
    if (document.kind === 'file') return name(document.path);
    return `${document.path ? name(document.path) : 'Changes'} (diff${document.staged ? ', staged' : ''})`;
  }

  function tabLabel(tab: WorkspaceTab): string {
    if (tab.document) return tab.title?.trim() || documentLabel(tab.document);
    const terminal = terminalById.get(tab.terminalId);
    return tab.title?.trim() || terminal?.name || worker.name || 'Terminal';
  }

  function terminalsOfTab(tab: WorkspaceTab): TerminalSession[] {
    return tabTerminalIds(tab)
      .map((id) => terminalById.get(id))
      .filter((terminal): terminal is TerminalSession => terminal !== undefined);
  }

  function isWorkerLive(value: Worker): boolean {
    if (value.status === 'exited') return false;
    return value.pid !== null || value.desired_running;
  }

  /** Drops a pane expansion that no longer matches the visible focused pane. */
  function clampMaximizedPane(): void {
    const value = snapshot.maximizedPane;
    if (value !== null && !isPaneMaximizeValid(snapshot, value)) snapshot.maximizedPane = null;
  }

  /** Mirrors toggleMaximize's target: group expansion wins, then the focused pane of a split tab. */
  function isMaximizeTargetActive(groupId: string): boolean {
    if (snapshot.maximizedGroupId === groupId) return true;
    const tab = activeTabForGroup(groupId);
    if (tab && tabTerminalIds(tab).length > 1) {
      const value = snapshot.maximizedPane;
      return value !== null && value.tabId === tab.id && value.terminalId === tabFocusTerminal(tab);
    }
    return false;
  }

  function activeHandle(): TerminalHandle | undefined {
    return activeTab ? terminalHandles.get(tabFocusTerminal(activeTab)) : undefined;
  }

  function activateTabInternal(tabId: string, notifyParent: boolean, terminalId?: string): void {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return;
    const group = groupContainingTab(snapshot, tabId);
    if (!group) return;
    const focus =
      terminalId && tabTerminalIds(tab).includes(terminalId) ? terminalId : tabFocusTerminal(tab);
    tab.focusedTerminalId = focus;
    group.activeTabId = tabId;
    snapshot.activeGroupId = group.id;
    clampMaximizedPane();
    markRecent(snapshot, tabId);
    persist();
    if (notifyParent) onActivateTerminal(focus);
    void tick().then(() => {
      const handle = terminalHandles.get(focus);
      if (snapshot.activeGroupId === group.id && group.activeTabId === tabId && ui.editingTabId === null) handle?.focus();
    });
  }

  function activeTabForGroup(groupId: string): WorkspaceTab | null {
    const group = groupById(snapshot, groupId);
    if (!group?.activeTabId) return null;
    return findTabById(snapshot, group.activeTabId) ?? null;
  }

  function syncParentWithActiveTab(): void {
    const group = groupById(snapshot, snapshot.activeGroupId) ?? snapshot.groups[0];
    let tab = group?.activeTabId ? findTabById(snapshot, group.activeTabId) : null;
    if (!tab) {
      for (const candidate of snapshot.groups) {
        if (!candidate.activeTabId) continue;
        const found = findTabById(snapshot, candidate.activeTabId);
        if (found) {
          tab = found;
          snapshot.activeGroupId = candidate.id;
          break;
        }
      }
    }
    onActivateTerminal(tab ? tabFocusTerminal(tab) : '');
  }

  /** Records the closed tab for reopen and removes it. The native close already happened. */
  function finishCloseTab(tabId: string, notifyParent: boolean): boolean {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return false;
    const group = groupContainingTab(snapshot, tabId);
    const entry: ClosedTabEntry = {
      terminalId: tab.terminalId,
      panes: clonePane(tab.panes),
      focusedTerminalId: tabFocusTerminal(tab),
      title: tab.title,
      color: tab.color,
      pinned: tab.pinned,
      groupId: group?.id ?? snapshot.activeGroupId,
      index: group ? group.tabIds.indexOf(tabId) : 0,
    };
    // Reopen restarts terminals; a document tab is reopened from the Explorer instead.
    if (!isDocumentTab(tab)) pushClosedTab(snapshot, entry);
    removeTab(snapshot, tabId);
    editors.delete(tabId);
    if (dirtyTabIds.has(tabId)) {
      const next = new Set(dirtyTabIds);
      next.delete(tabId);
      dirtyTabIds = next;
    }
    if (group && group.tabIds.length === 0 && snapshot.groups.length > 1) {
      // The vanished group is already empty, so collapsing it only repairs the layout.
      collapseGroup(snapshot, group.id);
    }
    persist();
    if (notifyParent) syncParentWithActiveTab();
    return true;
  }

  /**
   * Stops and marks each terminal closed. A failure leaves the layout alone so the
   * user still sees the pane and can retry. Records that vanished need no close.
   */
  async function closeTerminalsNative(ids: string[]): Promise<boolean> {
    for (const id of ids) {
      const terminal = terminalById.get(id);
      if (!terminal) continue;
      try {
        await onCloseTerminal(id);
      } catch (error) {
        onError(`${terminal.name}: ${errorMessage(error)}`);
        return false;
      }
    }
    return true;
  }

  async function closeTabNow(tabId: string, notifyParent = true): Promise<boolean> {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return false;
    const started = scope;
    beginLayoutWork();
    try {
      // An editor with unsaved changes asks before its tab goes away.
      const editor = editors.get(tabId);
      if (editor && !(await editor.requestClose())) return false;
      if (!(await closeTerminalsNative(tabTerminalIds(tab)))) return false;
      if (scope !== started) return false;
      return finishCloseTab(tabId, notifyParent);
    } finally {
      endLayoutWork();
    }
  }

  function liveTerminals(tabs: WorkspaceTab[]): TerminalSession[] {
    return tabs.flatMap(terminalsOfTab).filter(isTerminalLive);
  }

  function requestCloseTab(tabId: string): void {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return;
    const live = liveTerminals([tab]);
    const closeIt = async (): Promise<void> => {
      await closeTabNow(tabId);
    };
    if (tab.pinned) {
      confirmState = {
        title: 'Close pinned tab',
        message: live.length
          ? `${tabLabel(tab)} is pinned and ${live.length} terminal${live.length === 1 ? ' is' : 's are'} still running. Closing stops ${live.length === 1 ? 'it' : 'them'}; the worktree and session history are kept.`
          : `${tabLabel(tab)} is pinned. Close it anyway?`,
        confirmLabel: live.length ? 'Stop and close' : 'Close tab',
        danger: live.length > 0,
        onConfirm: closeIt,
      };
      return;
    }
    if (live.length > 0) {
      confirmState = {
        title: 'Stop terminals and close tab',
        message: `Closing ${tabLabel(tab)} stops ${live.length} running terminal${live.length === 1 ? '' : 's'}. The worktree, branch and session history are kept.`,
        confirmLabel: 'Stop and close',
        danger: true,
        onConfirm: closeIt,
      };
      return;
    }
    void closeIt();
  }

  /** Closes tabs one by one; returns how many could not be closed. */
  async function closeTabsNow(tabs: WorkspaceTab[]): Promise<number> {
    let failed = 0;
    for (const tab of tabs) {
      if (!(await closeTabNow(tab.id, false))) failed += 1;
    }
    return failed;
  }

  function requestCloseOthers(tabId: string): void {
    const group = groupContainingTab(snapshot, tabId);
    if (!group) return;
    const others = group.tabIds
      .filter((id) => id !== tabId)
      .map((id) => findTabById(snapshot, id))
      .filter((tab): tab is WorkspaceTab => tab !== null);
    const closable = others.filter((tab) => !tab.pinned);
    if (closable.length === 0) {
      if (others.length > 0) showToast('Pinned tabs were kept.');
      return;
    }
    const live = liveTerminals(closable);
    confirmState = {
      title: 'Close other tabs',
      message:
        live.length > 0
          ? `Close ${closable.length} other tabs and stop ${live.length} running terminal${live.length === 1 ? '' : 's'}? Worktrees are kept.`
          : `Close ${closable.length} other tabs?`,
      confirmLabel: 'Close others',
      danger: live.length > 0,
      onConfirm: async () => {
        const failed = await closeTabsNow(closable);
        syncParentWithActiveTab();
        if (failed > 0) onError(`${failed} tab${failed === 1 ? '' : 's'} could not be closed; kept open.`);
        if (others.some((tab) => tab.pinned)) showToast('Pinned tabs were kept.');
      },
    };
  }

  function requestCloseGroup(groupId: string): void {
    const group = groupById(snapshot, groupId);
    if (!group) return;
    if (snapshot.groups.length <= 1) {
      showToast('The last pane stays open. Close its tabs instead.');
      return;
    }
    const tabs = group.tabIds
      .map((id) => findTabById(snapshot, id))
      .filter((tab): tab is WorkspaceTab => tab !== null);
    const closable = tabs.filter((tab) => !tab.pinned);
    if (closable.length === 0) {
      showToast('Pinned tabs keep this pane open.');
      return;
    }
    const live = liveTerminals(closable);
    confirmState = {
      title: 'Close pane',
      message:
        live.length > 0
          ? `Close this pane, its ${closable.length} tabs, and stop ${live.length} running terminal${live.length === 1 ? '' : 's'}? Worktrees are kept.`
          : `Close this pane and its ${closable.length} tabs?`,
      confirmLabel: 'Close pane',
      danger: live.length > 0,
      onConfirm: async () => {
        const failed = await closeTabsNow(closable);
        if (tabs.some((tab) => tab.pinned)) showToast('Pinned tabs were kept.');
        if (failed > 0) {
          onError(`${failed} tab${failed === 1 ? '' : 's'} could not be closed; kept open.`);
          syncParentWithActiveTab();
          return;
        }
        const current = groupById(snapshot, groupId);
        if (current && current.tabIds.length === 0) {
          collapseGroup(snapshot, groupId);
          persist();
        }
        syncParentWithActiveTab();
      },
    };
  }

  /** Removes the given terminals from every tab except `keepTabId`. */
  function detachFromOtherTabs(ids: string[], keepTabId: string | null): void {
    for (const other of [...snapshot.tabs]) {
      if (other.id === keepTabId) continue;
      for (const id of ids) {
        if (!findTabById(snapshot, other.id) || !tabHasTerminal(other, id)) continue;
        if (!removePaneFromTab(other, id)) removeTab(snapshot, other.id);
      }
    }
  }

  /** Reopens the most recent closed tab by restarting the same terminal identities. */
  async function reopenClosedTab(): Promise<void> {
    const entry = snapshot.closedTabs[0];
    if (!entry) {
      showToast('No recently closed tab to reopen.');
      return;
    }
    const started = scope;
    const dropEntry = (): void => {
      snapshot.closedTabs = snapshot.closedTabs.filter((candidate) => candidate !== entry);
    };
    const ids = paneTerminalIds(entry.panes).filter((id) => terminalById.has(id));
    if (ids.length === 0) {
      dropEntry();
      persist();
      showToast('Those terminals no longer exist.');
      return;
    }
    beginLayoutWork();
    try {
      for (const id of ids) {
        try {
          await onRestartTerminal(id);
        } catch (error) {
          // The entry stays so the user can try again; restarted panes keep running.
          onError(`${terminalById.get(id)?.name ?? 'Terminal'}: ${errorMessage(error)}`);
          return;
        }
      }
      await tick();
      if (scope !== started) return;
      dropEntry();
      const present = ids.filter((id) => terminalById.has(id));
      let group = groupById(snapshot, entry.groupId) ?? groupById(snapshot, snapshot.activeGroupId);
      if (!group) group = snapshot.groups[0] ?? null;
      if (!group || present.length === 0) {
        persist();
        return;
      }
      let tab = findTabForTerminal(snapshot, present[0]);
      if (!tab) {
        const tabId = addTerminalTab(snapshot, present[0], group.id, Math.max(0, entry.index));
        tab = tabId ? findTabById(snapshot, tabId) : null;
      }
      if (!tab) {
        persist();
        return;
      }
      detachFromOtherTabs(present, tab.id);
      tab.panes = clonePane(entry.panes);
      for (const terminalId of tabTerminalIds(tab)) {
        if (!terminalById.has(terminalId)) removePaneFromTab(tab, terminalId);
      }
      const remaining = tabTerminalIds(tab);
      tab.terminalId = remaining[0];
      tab.focusedTerminalId = remaining.includes(entry.focusedTerminalId)
        ? entry.focusedTerminalId
        : remaining[0];
      tab.title = entry.title;
      tab.color = entry.color;
      tab.pinned = entry.pinned;
      activateTabInternal(tab.id, true, tab.focusedTerminalId);
    } finally {
      endLayoutWork();
    }
  }

  /** Opens (or focuses) a file or diff as a tab in the active group. */
  export function openDocument(document: TabDocument): void {
    if (scope === null) return;
    const group = groupById(snapshot, snapshot.activeGroupId) ?? snapshot.groups[0];
    if (!group) return;
    const existing = tabForDocument(snapshot, document);
    if (existing?.document?.kind === 'file' && document.kind === 'file') {
      existing.document = { ...existing.document, line: document.line };
    }
    const tabId = existing?.id ?? addDocumentTab(snapshot, document, group.id);
    if (!tabId) return;
    persist();
    activateTabInternal(tabId, true);
  }

  /**
   * Asks every open editor with unsaved changes whether to keep or discard them.
   * False when the user cancelled, so navigation can stay where it is.
   */
  export async function requestLeave(): Promise<boolean> {
    for (const editor of [...editors.values()]) {
      if (!(await editor.requestClose())) return false;
    }
    return true;
  }

  /** Adds a new terminal tab in the selected worktree. Never creates a worktree. */
  export async function createTerminal(kind: TerminalKind = 'shell'): Promise<void> {
    const started = scope;
    if (started === null) return;
    beginLayoutWork();
    try {
      const created = await onCreateTerminal(kind, null);
      await tick();
      if (scope !== started) return;
      const group = groupById(snapshot, snapshot.activeGroupId) ?? snapshot.groups[0];
      if (!group) return;
      const tabId = addTerminalTab(snapshot, created.id, group.id);
      if (tabId) activateTabInternal(tabId, true);
      else persist();
    } catch (error) {
      onError(`Could not create a terminal: ${errorMessage(error)}`);
    } finally {
      endLayoutWork();
    }
  }

  let splitInFlight = false;
  let lastSplitAt = 0;

  async function createSplit(groupId: string, direction: 'right' | 'down'): Promise<void> {
    if (splitInFlight || Date.now() - lastSplitAt < 400) return;
    splitInFlight = true;
    lastSplitAt = Date.now();
    const started = scope;
    beginLayoutWork();
    try {
      const group = groupById(snapshot, groupId) ?? groupById(snapshot, snapshot.activeGroupId);
      if (!group) return;
      // Resolve the source before the async roster update can move the active tab.
      const target = activeTabForGroup(group.id) ?? activeTab;
      if (!target) {
        // An empty pane has no source terminal, so a split is a plain new terminal here.
        const created = await onCreateTerminal('shell', null);
        await tick();
        if (scope !== started) return;
        const tabId = addTerminalTab(snapshot, created.id, group.id);
        if (tabId) activateTabInternal(tabId, true);
        return;
      }
      if (target.document) {
        // A document has no terminal to copy: the split gets a fresh shell in its own group.
        const created = await onCreateTerminal('shell', null);
        await tick();
        if (scope !== started) return;
        const tabId = addTerminalTab(snapshot, created.id, group.id);
        if (!tabId) return;
        splitTabToNewGroup(snapshot, tabId, group.id, direction, true);
        persist();
        activateTabInternal(tabId, true);
        return;
      }
      const targetId = target.id;
      const sourceId = tabFocusTerminal(target);
      snapshot.activeGroupId = group.id;
      group.activeTabId = targetId;
      target.focusedTerminalId = sourceId;
      onActivateTerminal(sourceId);
      persist();
      // A split copies the source's kind and, natively, its live cwd.
      const created = await onCreateTerminal(terminalById.get(sourceId)?.kind ?? 'shell', sourceId);
      await tick();
      if (scope !== started) return;

      const targetTab = findTabById(snapshot, targetId);
      // The new terminal must never keep a competing tab of its own.
      detachFromOtherTabs([created.id], targetTab?.id ?? null);
      if (targetTab) {
        if (!tabHasTerminal(targetTab, created.id)) {
          const at = tabHasTerminal(targetTab, sourceId) ? sourceId : tabFocusTerminal(targetTab);
          targetTab.panes = splitPane(targetTab.panes, at, created.id, direction);
        }
        if (snapshot.maximizedPane?.tabId === targetTab.id) snapshot.maximizedPane = null;
        targetTab.focusedTerminalId = created.id;
        snapshot.activeGroupId =
          groupContainingTab(snapshot, targetTab.id)?.id ?? snapshot.activeGroupId;
      } else if (!addTerminalTab(snapshot, created.id, group.id)) {
        return;
      }
      persist();
      const finalTab = findTabById(snapshot, targetId) ?? findTabForTerminal(snapshot, created.id);
      if (finalTab) {
        const owner = groupContainingTab(snapshot, finalTab.id);
        if (owner) owner.activeTabId = finalTab.id;
        activateTabInternal(finalTab.id, true, created.id);
      }
      void tick().then(() => activeHandle()?.focus());
    } catch (error) {
      onError(`Could not split the terminal: ${errorMessage(error)}`);
    } finally {
      splitInFlight = false;
      endLayoutWork();
    }
  }

  function cycleTab(direction: 1 | -1): void {
    const groupId = snapshot.activeGroupId;
    const tabId = adjacentTabId(snapshot, groupId, direction);
    if (tabId) activateTabInternal(tabId, true);
  }

  function cycleGroup(direction: 1 | -1): void {
    const groupId = adjacentGroupId(snapshot, snapshot.activeGroupId, direction);
    if (!groupId) return;
    snapshot.activeGroupId = groupId;
    persist();
    const tab = activeTabForGroup(groupId);
    if (tab) {
      const focus = tabFocusTerminal(tab);
      onActivateTerminal(focus);
      terminalHandles.get(focus)?.focus();
    }
  }

  function togglePin(tabId: string): void {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return;
    tab.pinned = !tab.pinned;
    persist();
    showToast(tab.pinned ? 'Tab pinned.' : 'Tab unpinned.');
  }

  function setTabColorValue(tabId: string, color: string | null): void {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return;
    tab.color = color;
    persist();
  }

  function setTabTitleValue(tabId: string, value: string): void {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return;
    const title = value.trim();
    tab.title = title.length > 0 ? title : null;
    ui.editingTabId = null;
    persist();
  }

  function beginRename(tabId: string): void {
    if (!findTabById(snapshot, tabId)) return;
    ui.editingTabId = tabId;
    activateTabInternal(tabId, false);
  }

  function cancelRename(): void {
    ui.editingTabId = null;
  }

  function showToast(message: string): void {
    ui.toast = message;
    if (toastTimer !== null) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      ui.toast = null;
      toastTimer = null;
    }, 2800);
  }

  async function writeClipboardText(text: string): Promise<boolean> {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      const area = document.createElement('textarea');
      area.value = text;
      area.setAttribute('readonly', '');
      area.style.position = 'fixed';
      area.style.opacity = '0';
      document.body.appendChild(area);
      area.select();
      let copied = false;
      try {
        copied = document.execCommand('copy');
      } catch {
        copied = false;
      }
      area.remove();
      return copied;
    }
  }

  async function copyText(text: string, label: string): Promise<void> {
    if (!text) {
      showToast(`No ${label.toLowerCase()} to copy.`);
      return;
    }
    const copied = await writeClipboardText(text);
    showToast(copied ? `${label} copied.` : `Could not copy ${label.toLowerCase()}.`);
  }

  function closeMenu(): void {
    subMenu = null;
    menu = null;
  }

  function restoreTerminalFocus(): void {
    void tick().then(() => activeHandle()?.focus());
  }

  function openMenuAt(x: number, y: number, items: MenuEntry[], width = 232): void {
    subMenu = null;
    const height = Math.min(430, items.length * 28 + 24);
    menu = {
      x: Math.max(8, Math.min(x, window.innerWidth - width - 8)),
      y: Math.max(8, Math.min(y, window.innerHeight - height - 8)),
      width,
      items,
    };
    void tick().then(() => {
      menuElement?.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus();
    });
  }

  function openSubMenu(opener: HTMLButtonElement, entry: Extract<MenuEntry,{kind:'submenu'}>, focus: boolean): void {
    if (subMenu?.label === entry.label && !focus) return;
    const rect = opener.getBoundingClientRect();
    const width = 208;
    subMenuOpener = opener;
    subMenu = {
      label: entry.label, items: entry.items, width,
      x: rect.right+width+8 > innerWidth ? Math.max(8,rect.left-width-4) : rect.right+4,
      y: Math.max(8,Math.min(rect.top,innerHeight-entry.items.length*28-16)),
    };
    if (focus) void tick().then(() => subMenuElement?.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus());
  }

  function onMenuFocusOut(event: FocusEvent): void {
    const next = event.relatedTarget;
    if (next instanceof Node && (menuElement?.contains(next) || subMenuElement?.contains(next))) return;
    queueMicrotask(() => {
      const active = document.activeElement;
      if (menu && !menuElement?.contains(active) && !subMenuElement?.contains(active)) closeMenu();
    });
  }

  function menuItem(
    label: string,
    icon: MenuIcon | undefined,
    shortcut: string | undefined,
    onSelect: () => void,
    options?: { danger?: boolean; disabled?: boolean },
  ): MenuEntry {
    return {
      kind: 'item',
      label,
      icon,
      shortcut,
      onSelect,
      danger: options?.danger ?? false,
      disabled: options?.disabled ?? false,
    };
  }

  function separator(): MenuEntry {
    return { kind: 'separator' };
  }

  function colorEntry(tab: WorkspaceTab): MenuEntry {
    return {
      kind: 'colors',
      selected: tab.color,
      onPick: (value) => setTabColorValue(tab.id, value),
    };
  }

  function openNewTabMenu(anchor: DOMRect): void {
    const groupId = snapshot.activeGroupId;
    openMenuAt(anchor.left, anchor.bottom + 6, [
      menuItem('New Terminal', SquareTerminal, 'Ctrl+T', () => void createTerminal('shell')),
      menuItem('New Agent Terminal', SquareTerminal, undefined, () => void createTerminal('agent')),
      menuItem('Split Terminal Right', SquareSplitVertical, 'Ctrl+Shift+D', () =>
        void createSplit(groupId, 'right'),
      ),
      menuItem('Split Terminal Down', SquareSplitHorizontal, 'Alt+Shift+D', () =>
        void createSplit(groupId, 'down'),
      ),
      separator(),
      menuItem('New Browser Tab', ExternalLink, 'Unavailable', () => onFeature('Browser'), {
        disabled: true,
      }),
      menuItem('New Markdown Preview', ExternalLink, 'Unavailable', () => onFeature('Markdown'), {
        disabled: true,
      }),
    ], 288);
  }

  function openQuickCommandsMenu(anchor: DOMRect): void {
    openMenuAt(anchor.left, anchor.bottom + 6, [
      menuItem('No quick commands configured', undefined, undefined, () => {}, {
        disabled: true,
      }),
      menuItem('Manage Quick Commands', undefined, undefined, () => onFeature('Quick Commands')),
    ], 288);
  }

  function openPaneActionsMenu(anchor: DOMRect, groupId: string): void {
    snapshot.activeGroupId = groupId;
    const current = activeTabForGroup(groupId);
    if (current) onActivateTerminal(tabFocusTerminal(current));
    const maximized = isMaximizeTargetActive(groupId);
    openMenuAt(anchor.right - 232, anchor.bottom + 6, [
      menuItem('Split Terminal Right', SquareSplitVertical, 'Ctrl+Shift+D', () =>
        void createSplit(groupId, 'right'),
      ),
      menuItem('Split Terminal Down', SquareSplitHorizontal, 'Alt+Shift+D', () =>
        void createSplit(groupId, 'down'),
      ),
      separator(),
      menuItem('Equalize Pane Sizes', PanelsTopLeft, undefined, equalizePanes, {
        disabled: !hasSplits && (!current || current.panes.type !== 'split'),
      }),
      menuItem(
        maximized ? 'Collapse Pane' : 'Expand Pane',
        maximized ? Minimize2 : Maximize2,
        'Ctrl+Shift+Enter',
        () => toggleMaximize(groupId),
      ),
      separator(),
      menuItem(
        'Close Pane',
        PanelRightClose,
        undefined,
        () => {
          if (current && tabTerminalIds(current).length > 1) {
            closePane(current.id, tabFocusTerminal(current));
          } else {
            requestCloseGroup(groupId);
          }
        },
        {
          danger: true,
          disabled:
            snapshot.groups.length <= 1 &&
            (!current || tabTerminalIds(current).length <= 1),
        },
      ),
    ]);
  }

  async function runTerminalAction(
    action: () => Promise<TerminalSession>,
    success: string,
  ): Promise<void> {
    try {
      await action();
      showToast(success);
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  /** Process actions for one terminal; they never touch the worktree. */
  function terminalMenuEntries(terminalId: string): MenuEntry[] {
    const terminal = terminalById.get(terminalId);
    if (!terminal) return [];
    return [
      menuItem('Restart Terminal', RefreshCw, undefined, () => {
        void runTerminalAction(() => onRestartTerminal(terminalId), `${terminal.name} restarted.`);
      }),
      menuItem(
        'Stop Terminal',
        Square,
        undefined,
        () => {
          confirmState = {
            title: 'Stop terminal',
            message: `Stop ${terminal.name}? Its process ends; the tab, worktree and session history stay.`,
            confirmLabel: 'Stop terminal',
            danger: true,
            onConfirm: async () => {
              await onStopTerminal(terminalId);
              showToast(`${terminal.name} stopped.`);
            },
          };
        },
        { disabled: !isTerminalLive(terminal) },
      ),
    ];
  }

  function workerMenuEntries(): MenuEntry[] {
    const target = worker;
    return [
      separator(),
      menuItem('Open in Editor', ExternalLink, undefined, () => onAction('editor', target)),
      menuItem('View Changes', GitCompare, undefined, () => onAction('diff', target)),
      menuItem('Restart Worker', RefreshCw, undefined, () => onAction('restart', target)),
      menuItem(
        'Stop Worker',
        Square,
        undefined,
        () => {
          confirmState = {
            title: 'Stop worker',
            message: `Stop every terminal in ${target.name}? Their processes end; the worktree and session history stay.`,
            confirmLabel: 'Stop worker',
            danger: true,
            onConfirm: async () => {
              if (!onStopWorker) {
                onError('Stopping a worker is not available in this shell.');
                return;
              }
              await onStopWorker(target);
              showToast(`${target.name} stopped.`);
            },
          };
        },
        { disabled: !isWorkerLive(target) },
      ),
      menuItem('Merge Worktree...', GitMerge, undefined, () => onAction('merge', target)),
      menuItem(
        'Delete Workspace...',
        Trash2,
        undefined,
        () => onAction('delete', target),
        { danger: true },
      ),
    ];
  }

  function openTabMenu(event: MouseEvent, tab: WorkspaceTab): void {
    event.preventDefault();
    event.stopPropagation();
    activateTabInternal(tab.id, true);
    const group = groupContainingTab(snapshot, tab.id);
    const groupId = group?.id ?? snapshot.activeGroupId;
    const items: MenuEntry[] = [
      {kind:'submenu',label:'Move Tab to Split',icon:PanelsTopLeft,items:[
        ...(['right','down','left','up'] as const).map(direction => menuItem(
          direction[0].toUpperCase()+direction.slice(1),
          direction === 'left' ? ChevronLeft : direction === 'right' ? ChevronRight : direction === 'up' ? Maximize2 : Minimize2,
          undefined,() => moveTabToNewGroup(tab.id,groupId,direction),
          {disabled:snapshot.groups.length === 1 && (group?.tabIds.length ?? 0) < 2},
        )),
      ]},
      {kind:'submenu',label:'Split terminal',icon:SquareSplitVertical,items:[
        menuItem('Split Right',SquareSplitVertical,'Ctrl+Shift+D',() => void createSplit(groupId,'right')),
        menuItem('Split Down',SquareSplitHorizontal,'Alt+Shift+D',() => void createSplit(groupId,'down')),
      ]},
      separator(),
      menuItem(tab.pinned ? 'Unpin Tab' : 'Pin Tab', tab.pinned ? PinOff : Pin, undefined, () => togglePin(tab.id)),
      ...(tab.document?.kind === 'file' ? [menuItem('Copy Path',Copy,undefined,() => void copyText(tab.document?.kind === 'file' ? tab.document.path : '','Path'))] : []),
      menuItem('Copy Tab ID',Copy,undefined,() => void copyText(tab.id,'Tab ID')),
      separator(),
      menuItem('Close', X, 'Ctrl+W', () => requestCloseTab(tab.id), { danger: true }),
      menuItem('Close Others', X, undefined, () => requestCloseOthers(tab.id)),
      menuItem('Change Title', Pencil, undefined, () => beginRename(tab.id)),
      colorEntry(tab),
      menuItem(
        'Close Pane',
        PanelRightClose,
        undefined,
        () => {
          if (tabTerminalIds(tab).length > 1) closePane(tab.id, tabFocusTerminal(tab));
          else requestCloseGroup(groupId);
        },
        {
          danger: true,
          disabled: tabTerminalIds(tab).length <= 1 && snapshot.groups.length <= 1,
        },
      ),
    ];
    items.push(separator(), ...terminalMenuEntries(tabFocusTerminal(tab)), ...workerMenuEntries());
    openMenuAt(event.clientX, event.clientY, items);
  }

  function openTerminalMenu(event: MouseEvent, groupId: string): void {
    event.preventDefault();
    event.stopPropagation();
    snapshot.activeGroupId = groupId;
    const tab = activeTabForGroup(groupId);
    const focus = tab ? tabFocusTerminal(tab) : null;
    if (focus) onActivateTerminal(focus);
    const handle = focus ? terminalHandles.get(focus) : undefined;
    const maximized = isMaximizeTargetActive(groupId);
    openMenuAt(event.clientX, event.clientY, [
      menuItem(
        'Copy',
        Copy,
        'Ctrl+Shift+C',
        () => {
          void handle?.copySelection().then((ok) => {
            if (ok) showToast('Selection copied.');
          });
        },
        { disabled: !handle?.hasSelection() },
      ),
      menuItem('Paste', Clipboard, 'Ctrl+V', () => {
        void handle?.pasteClipboard().then((ok) => {
          if (!ok) showToast('Clipboard is empty or unavailable.');
        });
      }),
      menuItem('Select All', TextSelect, 'Ctrl+Shift+A', () => handle?.selectAll()),
      menuItem('Search', Search, 'Ctrl+F', () => handle?.focusSearch()),
      separator(),
      menuItem('Split Terminal Right', SquareSplitVertical, 'Ctrl+Shift+D', () =>
        void createSplit(groupId, 'right'),
      ),
      menuItem('Split Terminal Down', SquareSplitHorizontal, 'Alt+Shift+D', () =>
        void createSplit(groupId, 'down'),
      ),
      menuItem('Equalize Pane Sizes', PanelsTopLeft, undefined, equalizePanes, {
        disabled: !hasSplits && (!tab || tab.panes.type !== 'split'),
      }),
      menuItem(
        maximized ? 'Collapse Pane' : 'Expand Pane',
        maximized ? Minimize2 : Maximize2,
        'Ctrl+Shift+Enter',
        () => toggleMaximize(groupId),
      ),
      separator(),
      menuItem('Set Title...', Pencil, undefined, () => {
        if (tab) beginRename(tab.id);
      }),
      menuItem('Clear Screen', Eraser, 'Ctrl+K', () => handle?.clear()),
      menuItem('Reset Terminal', RotateCcw, undefined, () => handle?.reset()),
      ...(focus ? [separator(), ...terminalMenuEntries(focus)] : []),
      separator(),
      menuItem('Close Pane', PanelRightClose, undefined, () => requestCloseGroup(groupId), {
        danger: true,
        disabled: snapshot.groups.length <= 1,
      }),
    ], 240);
  }

  function openPaneMenu(event: MouseEvent, tabId: string, terminalId: string): void {
    event.preventDefault();
    event.stopPropagation();
    const tab = findTabById(snapshot, tabId);
    if (!tab) return;
    const group = groupContainingTab(snapshot, tabId);
    if (!group) return;
    snapshot.activeGroupId = group.id;
    tab.focusedTerminalId = terminalId;
    clampMaximizedPane();
    onActivateTerminal(terminalId);
    const handle = terminalHandles.get(terminalId);
    const paneCount = tabTerminalIds(tab).length;
    const maximized = isMaximizeTargetActive(group.id);
    openMenuAt(event.clientX, event.clientY, [
      menuItem(
        'Copy',
        Copy,
        'Ctrl+Shift+C',
        () => {
          void handle?.copySelection().then((ok) => {
            if (ok) showToast('Selection copied.');
          });
        },
        { disabled: !handle?.hasSelection() },
      ),
      menuItem('Paste', Clipboard, 'Ctrl+V', () => {
        void handle?.pasteClipboard().then((ok) => {
          if (!ok) showToast('Clipboard is empty or unavailable.');
        });
      }),
      menuItem('Select All', TextSelect, 'Ctrl+Shift+A', () => handle?.selectAll()),
      menuItem('Search', Search, 'Ctrl+F', () => handle?.focusSearch()),
      separator(),
      menuItem('Split Terminal Right', SquareSplitVertical, 'Ctrl+Shift+D', () =>
        void createSplit(group.id, 'right'),
      ),
      menuItem('Split Terminal Down', SquareSplitHorizontal, 'Alt+Shift+D', () =>
        void createSplit(group.id, 'down'),
      ),
      menuItem('Equalize Pane Sizes', PanelsTopLeft, undefined, equalizePanes, {
        disabled: tab.panes.type !== 'split',
      }),
      menuItem(
        maximized ? 'Collapse Pane' : 'Expand Pane',
        maximized ? Minimize2 : Maximize2,
        'Ctrl+Shift+Enter',
        () => toggleMaximize(group.id),
      ),
      separator(),
      menuItem('Set Title...', Pencil, undefined, () => beginRename(tab.id)),
      menuItem('Clear Screen', Eraser, 'Ctrl+K', () => handle?.clear()),
      menuItem('Reset Terminal', RotateCcw, undefined, () => handle?.reset()),
      separator(),
      ...terminalMenuEntries(terminalId),
      separator(),
      paneCount > 1
        ? menuItem('Close Pane', PanelRightClose, undefined, () => closePane(tab.id, terminalId), {
            danger: true,
          })
        : menuItem('Close Tab', X, 'Ctrl+W', () => requestCloseTab(tab.id), { danger: true }),
    ], 240);
  }

  function toggleMaximize(groupId: string): void {
    snapshot.activeGroupId = groupId;
    const tab = activeTabForGroup(groupId);
    const focus = tab ? tabFocusTerminal(tab) : null;
    if (tab && focus !== null) tab.focusedTerminalId = focus;
    if (snapshot.maximizedGroupId === groupId) {
      // Collapse the group expansion before a split tab can expand one pane.
      snapshot.maximizedGroupId = null;
      snapshot.maximizedPane = null;
    } else if (tab && focus !== null && tabTerminalIds(tab).length > 1) {
      // A split tab expands its focused terminal; this replaces any group expansion.
      const current = snapshot.maximizedPane;
      const expanded =
        current !== null && current.tabId === tab.id && current.terminalId === focus;
      snapshot.maximizedGroupId = null;
      snapshot.maximizedPane = expanded ? null : { tabId: tab.id, terminalId: focus };
    } else {
      // A single-pane group expands the whole group.
      snapshot.maximizedPane = null;
      snapshot.maximizedGroupId = groupId;
    }
    persist();
    if (focus !== null) onActivateTerminal(focus);
    void tick().then(() => {
      const handle = focus !== null ? terminalHandles.get(focus) : undefined;
      handle?.focus();
    });
  }

  function equalizePanes(): void {
    snapshot.layout = equalizeLayout(snapshot.layout);
    if (activeTab) activeTab.panes = equalizePane(activeTab.panes);
    persist();
  }

  function setRatio(path: string, ratio: number): void {
    snapshot.layout = updateRatio(snapshot.layout, path, clampRatio(ratio));
    persist();
  }

  function setPaneRatio(tabId: string, path: string, ratio: number): void {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return;
    tab.panes = updatePaneRatio(tab.panes, path, clampRatio(ratio));
    persist();
  }

  async function closePaneNow(tabId: string, terminalId: string): Promise<void> {
    const started = scope;
    beginLayoutWork();
    try {
      // Only this pane's terminal stops; its siblings and the worktree keep running.
      if (!(await closeTerminalsNative([terminalId]))) return;
      if (scope !== started) return;
      const tab = findTabById(snapshot, tabId);
      if (!tab) return;
      const wasFocused = tabFocusTerminal(tab) === terminalId;
      if (!removePaneFromTab(tab, terminalId)) {
        finishCloseTab(tabId, true);
        return;
      }
      clampMaximizedPane();
      persist();
      if (wasFocused) activateTabInternal(tab.id, true, tabFocusTerminal(tab));
      else syncParentWithActiveTab();
    } finally {
      endLayoutWork();
    }
  }

  function closePane(tabId: string, terminalId: string): void {
    const tab = findTabById(snapshot, tabId);
    if (!tab) return;
    if (tabTerminalIds(tab).length <= 1) {
      requestCloseTab(tabId);
      return;
    }
    const terminal = terminalById.get(terminalId);
    if (!terminal || !isTerminalLive(terminal)) {
      void closePaneNow(tabId, terminalId);
      return;
    }
    confirmState = {
      title: 'Stop pane and close',
      message: `Closing this pane stops ${terminal.name}. Other panes and the worktree stay open.`,
      confirmLabel: 'Stop and close',
      danger: true,
      onConfirm: () => closePaneNow(tabId, terminalId),
    };
  }

  function focusGroup(groupId: string): void {
    if (snapshot.activeGroupId === groupId) return;
    snapshot.activeGroupId = groupId;
    persist();
    const tab = activeTabForGroup(groupId);
    if (tab) onActivateTerminal(tabFocusTerminal(tab));
  }

  function focusPane(terminalId: string): void {
    const tab = findTabForTerminal(snapshot, terminalId);
    if (!tab) return;
    const group = groupContainingTab(snapshot, tab.id);
    if (!group) return;
    if (group.id !== snapshot.activeGroupId || group.activeTabId !== tab.id) return;
    if (tab.focusedTerminalId === terminalId) return;
    tab.focusedTerminalId = terminalId;
    clampMaximizedPane();
    persist();
  }

  /** Brings a terminal's tab and pane forward, adding a tab if a live terminal has none. */
  export function activateTerminal(terminalId: string): void {
    const terminal = terminalById.get(terminalId);
    if (!terminal) return;
    let tab = findTabForTerminal(snapshot, terminalId);
    if (!tab) {
      if (terminal.closed) return;
      const group = groupById(snapshot, snapshot.activeGroupId) ?? snapshot.groups[0];
      if (!group) return;
      const tabId = addTerminalTab(snapshot, terminalId, group.id);
      if (!tabId) return;
      tab = findTabById(snapshot, tabId);
      persist();
    }
    if (tab) {
      const group = groupContainingTab(snapshot, tab.id);
      if (
        group?.id === snapshot.activeGroupId &&
        group.activeTabId === tab.id &&
        tabFocusTerminal(tab) === terminalId
      ) {
        // Already the active pane. Do not notify the parent: terminal autofocus
        // during mount must not overwrite the restored selection.
        return;
      }
      activateTabInternal(tab.id, true, terminalId);
    }
  }

  function moveTabToGroup(tabId: string, groupId: string, index?: number): void {
    if (moveTab(snapshot, tabId, groupId, index)) {
      persist();
      syncParentWithActiveTab();
    }
  }

  function moveTabToNewGroup(tabId: string, groupId: string, direction: DropPosition): void {
    if (splitTabToNewGroup(snapshot, tabId, groupId, direction)) {
      persist();
      syncParentWithActiveTab();
      void tick().then(() => activeHandle()?.focus());
    }
  }

  function isClickSuppressed(): boolean {
    return suppressClick;
  }

  interface DropHit {
    insertion: TabInsertion | null;
    target: { groupId: string; zone: 'center' | DropPosition } | null;
  }

  function hitTest(x: number, y: number): DropHit {
    const stack = document.elementsFromPoint(x, y);
    for (const element of stack) {
      const tabElement = element.closest<HTMLElement>('[data-tab-id]');
      if (!tabElement) continue;
      const targetTabId = tabElement.dataset.tabId;
      if (!targetTabId) continue;
      const group = groupContainingTab(snapshot, targetTabId);
      if (!group) continue;
      const rect = tabElement.getBoundingClientRect();
      const index = group.tabIds.indexOf(targetTabId);
      const left = x < rect.left + rect.width / 2;
      return {
        insertion: {
          groupId: group.id,
          index: left ? index : index + 1,
          side: left ? 'left' : 'right',
        },
        target: null,
      };
    }
    for (const element of stack) {
      const body = element.closest<HTMLElement>('[data-drop-group]');
      if (!body) continue;
      const groupId = body.dataset.dropGroup;
      if (!groupId) continue;
      const rect = body.getBoundingClientRect();
      const relX = rect.width > 0 ? (x - rect.left) / rect.width : 0.5;
      const relY = rect.height > 0 ? (y - rect.top) / rect.height : 0.5;
      let zone: 'center' | DropPosition = 'center';
      if (relX < 0.25) zone = 'left';
      else if (relX > 0.75) zone = 'right';
      else if (relY < 0.25) zone = 'up';
      else if (relY > 0.75) zone = 'down';
      return { insertion: null, target: { groupId, zone } };
    }
    return { insertion: null, target: null };
  }

  function beginTabDrag(event: PointerEvent, tabId: string): void {
    if (event.button !== 0 || ui.editingTabId === tabId) return;
    const startX = event.clientX;
    const startY = event.clientY;
    let active = false;
    let latest: DragState | null = null;

    const onMove = (moveEvent: PointerEvent): void => {
      if (moveEvent.pointerId !== event.pointerId) return;
      if (!active && Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY) < 5) return;
      active = true;
      moveEvent.preventDefault();
      const hit = hitTest(moveEvent.clientX, moveEvent.clientY);
      const source = groupContainingTab(snapshot, tabId);
      const invalidEdge =
        hit.target !== null &&
        source !== null &&
        source.id === hit.target.groupId &&
        source.tabIds.length <= 1 &&
        hit.target.zone !== 'center';
      latest = {
        tabId,
        active: true,
        x: moveEvent.clientX,
        y: moveEvent.clientY,
        insertion: hit.insertion,
        target: invalidEdge ? null : hit.target,
      };
      ui.drag = latest;
    };

    const cleanup = (): void => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onCancel);
    };

    const onUp = (): void => {
      cleanup();
      const drag = latest;
      ui.drag = null;
      latest = null;
      if (!active || !drag) return;
      suppressClick = true;
      if (suppressClickTimer !== null) clearTimeout(suppressClickTimer);
      suppressClickTimer = setTimeout(() => {
        suppressClick = false;
        suppressClickTimer = null;
      }, 80);
      applyDrop(drag);
    };

    const onCancel = (): void => {
      cleanup();
      ui.drag = null;
      latest = null;
    };

    window.addEventListener('pointermove', onMove, { passive: false });
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onCancel);
  }

  function applyDrop(drag: DragState): void {
    if (drag.insertion) {
      const changed = moveTab(
        snapshot,
        drag.tabId,
        drag.insertion.groupId,
        drag.insertion.index,
      );
      if (changed) {
        persist();
        syncParentWithActiveTab();
      }
      return;
    }
    if (drag.target) {
      if (drag.target.zone === 'center') {
        if (moveTab(snapshot, drag.tabId, drag.target.groupId)) {
          persist();
          syncParentWithActiveTab();
        }
      } else {
        moveTabToNewGroup(drag.tabId, drag.target.groupId, drag.target.zone);
      }
    }
  }

  function recentTabs(): WorkspaceTab[] {
    const ordered: string[] = [];
    const push = (id: string): void => {
      if (ordered.includes(id)) return;
      if (!tabById.has(id)) return;
      const tab = tabById.get(id);
      if (!tab || (!tab.document && !tabTerminalIds(tab).some((terminalId) => terminalById.has(terminalId)))) return;
      ordered.push(id);
    };
    for (const id of snapshot.recentTabIds) push(id);
    for (const tab of snapshot.tabs) push(tab.id);
    return ordered
      .map((id) => tabById.get(id))
      .filter((tab): tab is WorkspaceTab => tab !== undefined);
  }

  function openRecent(direction: 1 | -1): void {
    const tabs = recentTabs();
    if (tabs.length < 2) return;
    let index: number;
    if (recent) {
      index = (recent.index + direction + tabs.length) % tabs.length;
    } else {
      const currentIndex = activeTab ? tabs.findIndex((tab) => tab.id === activeTab!.id) : -1;
      index =
        direction === 1
          ? (currentIndex + 1 + tabs.length) % tabs.length
          : (currentIndex - 1 + tabs.length) % tabs.length;
    }
    recent = { ids: tabs.map((tab) => tab.id), index };
  }

  function commitRecent(): void {
    const state = recent;
    recent = null;
    if (!state) return;
    const tabId = state.ids[state.index];
    if (tabId) activateTabInternal(tabId, true);
  }

  function onWindowKeyup(event: KeyboardEvent): void {
    if (!recent) return;
    if (event.key === 'Control' || event.key === 'ControlLeft' || event.key === 'ControlRight') {
      commitRecent();
    }
  }

  function onWindowBlur(): void {
    recent = null;
    closeMenu();
  }

  function onWindowPointerDown(event: PointerEvent): void {
    if (menu) {
      const target = event.target;
      if (!(target instanceof Node) || (!menuElement?.contains(target) && !subMenuElement?.contains(target))) closeMenu();
    }
    if (confirmState) {
      const target = event.target;
      if (!(target instanceof Node) || !confirmElement?.contains(target)) confirmState = null;
    }
  }

  function onMenuKeydown(event: KeyboardEvent): void {
    if (event.defaultPrevented) return;
    if (event.key === 'ArrowLeft' && subMenu) {
      event.preventDefault(); event.stopPropagation(); subMenu = null; subMenuOpener?.focus(); return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      closeMenu();
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp' && event.key !== 'Home' && event.key !== 'End') {
      return;
    }
    event.preventDefault();
    const buttons = Array.from(
      (event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)'),
    );
    if (buttons.length === 0) return;
    const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
    let next = current;
    if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = buttons.length - 1;
    else if (event.key === 'ArrowDown') next = (current + 1 + buttons.length) % buttons.length;
    else next = current < 0 ? buttons.length - 1 : (current - 1 + buttons.length) % buttons.length;
    buttons[next]?.focus();
  }

  async function runConfirm(): Promise<void> {
    const current = confirmState;
    if (!current) return;
    confirmState = null;
    try {
      await current.onConfirm();
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  function internalEscape(event: KeyboardEvent): void {
    if (event.defaultPrevented || event.key !== 'Escape') return;
    if (ui.editingTabId !== null) {
      event.preventDefault();
      cancelRename();
      restoreTerminalFocus();
      return;
    }
    if (confirmState) {
      event.preventDefault();
      confirmState = null;
      restoreTerminalFocus();
      return;
    }
    if (menu) {
      event.preventDefault();
      closeMenu();
      restoreTerminalFocus();
      return;
    }
    if (recent) {
      event.preventDefault();
      recent = null;
      restoreTerminalFocus();
    }
  }

  function typingInEditableField(target: EventTarget | null): boolean {
    if (target instanceof HTMLElement && target.closest('.xterm')) return false;
    return isTextEntryTarget(target);
  }

  export function handleShortcut(event: KeyboardEvent): boolean {
    if (event.isComposing) return false;
    const key = event.key;

    if (key === 'Escape' && !event.ctrlKey && !event.altKey && !event.metaKey) {
      if (ui.editingTabId !== null) {
        cancelRename();
        restoreTerminalFocus();
        event.preventDefault();
        return true;
      }
      if (menu) {
        if (subMenu) { subMenu = null; subMenuOpener?.focus(); event.preventDefault(); return true; }
        closeMenu();
        restoreTerminalFocus();
        event.preventDefault();
        return true;
      }
      if (recent) {
        recent = null;
        restoreTerminalFocus();
        event.preventDefault();
        return true;
      }
      if (confirmState) {
        confirmState = null;
        restoreTerminalFocus();
        event.preventDefault();
        return true;
      }
      const handle = activeHandle();
      if (handle?.isSearchOpen()) {
        handle.closeSearch();
        event.preventDefault();
        return true;
      }
      return false;
    }

    if (typingInEditableField(event.target)) return false;

    const mod = event.ctrlKey && !event.altKey && !event.metaKey;
    const shift = event.shiftKey;
    const groupId = snapshot.activeGroupId;

    if (event.repeat && key.toLowerCase() === 'd') {
      event.preventDefault();
      return true;
    }

    if (mod && key === 'Tab') {
      event.preventDefault();
      openRecent(shift ? -1 : 1);
      return true;
    }

    if (mod && !shift && key.toLowerCase() === 't') {
      event.preventDefault();
      void createTerminal('shell');
      return true;
    }
    if (mod && shift && key.toLowerCase() === 't') {
      event.preventDefault();
      void reopenClosedTab();
      return true;
    }
    if (mod && !shift && key.toLowerCase() === 'w') {
      const tab = activeTab;
      if (tab) {
        event.preventDefault();
        if (tabTerminalIds(tab).length > 1) closePane(tab.id,tabFocusTerminal(tab));
        else requestCloseTab(tab.id);
        return true;
      }
      return false;
    }
    if (mod && !shift && key === 'PageUp') {
      event.preventDefault();
      cycleTab(-1);
      return true;
    }
    if (mod && !shift && key === 'PageDown') {
      event.preventDefault();
      cycleTab(1);
      return true;
    }
    if (mod && shift && (key === '[' || key === '{')) {
      event.preventDefault();
      cycleTab(-1);
      return true;
    }
    if (mod && shift && (key === ']' || key === '}')) {
      event.preventDefault();
      cycleTab(1);
      return true;
    }
    if (event.ctrlKey && event.altKey && (key === '[' || key === '{')) {
      event.preventDefault();
      cycleTab(-1);
      return true;
    }
    if (event.ctrlKey && event.altKey && (key === ']' || key === '}')) {
      event.preventDefault();
      cycleTab(1);
      return true;
    }
    if (mod && !shift && (key === '[' || key === '{')) {
      event.preventDefault();
      cycleGroup(-1);
      return true;
    }
    if (mod && !shift && (key === ']' || key === '}')) {
      event.preventDefault();
      cycleGroup(1);
      return true;
    }
    if (event.altKey && !event.ctrlKey && !event.metaKey && /^[1-9]$/.test(key)) {
      const group = groupById(snapshot, groupId);
      const index = Number(key) - 1;
      const tabId = group?.tabIds[index];
      if (tabId) {
        event.preventDefault();
        activateTabInternal(tabId, true);
        return true;
      }
      return false;
    }
    if (mod && shift && key.toLowerCase() === 'd') {
      event.preventDefault();
      void createSplit(groupId, 'right');
      return true;
    }
    if (event.altKey && shift && !event.ctrlKey && key.toLowerCase() === 'd') {
      event.preventDefault();
      void createSplit(groupId, 'down');
      return true;
    }
    if (mod && shift && key === 'Enter') {
      event.preventDefault();
      toggleMaximize(groupId);
      return true;
    }
    if (mod && !shift && key.toLowerCase() === 'f') {
      event.preventDefault();
      activeHandle()?.focusSearch();
      return true;
    }
    if (mod && !shift && key.toLowerCase() === 'k') {
      event.preventDefault();
      activeHandle()?.clear();
      return true;
    }
    if (mod && shift && key.toLowerCase() === 'a') {
      event.preventDefault();
      activeHandle()?.selectAll();
      return true;
    }
    if (mod && key.toLowerCase() === 'c') {
      const handle = activeHandle();
      if (!shift && !handle?.hasSelection()) return false;
      if (!handle) return false;
      event.preventDefault();
      void handle.copySelection().then((ok) => {
        if (ok) showToast('Selection copied.');
      });
      return true;
    }
    if (mod && key.toLowerCase() === 'v') {
      const handle = activeHandle();
      if (!handle) return false;
      event.preventDefault();
      void handle.pasteClipboard().then((ok) => {
        if (!ok) showToast('Clipboard is empty or unavailable.');
      });
      return true;
    }
    if (!mod && shift && key === 'Insert') {
      const handle = activeHandle();
      if (!handle) return false;
      event.preventDefault();
      void handle.pasteClipboard().then((ok) => {
        if (!ok) showToast('Clipboard is empty or unavailable.');
      });
      return true;
    }
    if (mod && shift && key.toLowerCase() === 'e') {
      event.preventDefault();
      onFeature('Explorer');
      return true;
    }
    if (mod && shift && key.toLowerCase() === 'f') {
      event.preventDefault();
      onFeature('Search');
      return true;
    }
    if (mod && shift && key.toLowerCase() === 'g') {
      event.preventDefault();
      onFeature('Source Control');
      return true;
    }
    if (mod && shift && key.toLowerCase() === 'b') {
      event.preventDefault();
      onFeature('Browser');
      return true;
    }
    if (mod && shift && key.toLowerCase() === 'm') {
      event.preventDefault();
      onFeature('Markdown');
      return true;
    }
    if (mod && shift && key.toLowerCase() === 'o') {
      event.preventDefault();
      onFeature('Editor');
      return true;
    }
    if (mod && shift && key.toLowerCase() === 'y') {
      event.preventDefault();
      onFeature('Plugins');
      return true;
    }
    if (mod && !shift && key.toLowerCase() === 'b') {
      event.preventDefault();
      onToggleSidebar();
      return true;
    }
    if (mod && !shift && key.toLowerCase() === 'l') {
      event.preventDefault();
      onToggleRightSidebar();
      return true;
    }
    return false;
  }

  export function focusActive(): void {
    const handle = activeHandle();
    if (handle) {
      if (handle.isSearchOpen()) handle.focusSearch();
      else handle.focus();
      return;
    }
    const tab = activeTab;
    if (tab) terminalHandles.get(tabFocusTerminal(tab))?.focus();
  }

  const api: WorkspaceApi = {
    activateTab(tabId) {
      activateTabInternal(tabId, true);
    },
    activateTerminal,
    focusPane,
    closeTab(tabId) {
      requestCloseTab(tabId);
    },
    closeOtherTabs(tabId) {
      requestCloseOthers(tabId);
    },
    closeGroup(groupId) {
      requestCloseGroup(groupId);
    },
    togglePin,
    setTabColor: setTabColorValue,
    setTabTitle: setTabTitleValue,
    beginRename,
    commitRename(tabId, value) {
      setTabTitleValue(tabId, value);
    },
    cancelRename,
    moveTabToGroup,
    moveTabToNewGroup,
    splitWithNewTerminal(groupId, direction) {
      void createSplit(groupId, direction);
    },
    createTerminal() {
      void createTerminal('shell');
    },
    reopenClosedTab() {
      void reopenClosedTab();
    },
    focusGroup,
    setRatio,
    setPaneRatio,
    closePane,
    openPaneMenu,
    equalizePanes,
    toggleMaximize,
    beginTabDrag,
    openNewTabMenu,
    openQuickCommandsMenu,
    openPaneActionsMenu,
    openTabMenu,
    openTerminalMenu,
    copyText(text, label) {
      void copyText(text, label);
    },
    registerEditor(tabId, handle) {
      if (handle) editors.set(tabId, handle);
      else editors.delete(tabId);
    },
    setTabDirty(tabId, dirty) {
      if (dirtyTabIds.has(tabId) === dirty) return;
      const next = new Set(dirtyTabIds);
      if (dirty) next.add(tabId);
      else next.delete(tabId);
      dirtyTabIds = next;
    },
    registerTerminal(terminalId, handle, identity) {
      if (handle) {
        terminalHandles.set(terminalId, handle);
        return;
      }
      if (!identity || terminalHandles.get(terminalId) === identity) {
        terminalHandles.delete(terminalId);
      }
    },
    handleFor(terminalId) {
      return terminalHandles.get(terminalId);
    },
    isClickSuppressed,
    showToast,
  };
  setContext(WS_VIEW, view);
  setContext(WS_API, api);

  $effect(() => {
    if (!confirmState) return;
    void tick().then(() => confirmCancelButton?.focus());
  });

  $effect(() => {
    const projectId = project.id;
    const terminalId = worker.id;
    if (scope?.projectId === projectId && scope.terminalId === terminalId) return;
    untrack(() => {
      scope = { projectId, terminalId };
      snapshot = loadSnapshot(projectId, terminalId, terminals, null);
      ui = { drag: null, editingTabId: null, toast: null };
      menu = null;
      subMenu = null;
      recent = null;
      confirmState = null;
      terminalHandles.clear();
      editors.clear();
      dirtyTabIds = new Set();
      // loadSnapshot reconciled against the live roster after it saved; store the result.
      persist();
      // Never notify the parent while mounting: it owns the restored selection.
    });
  });

  $effect(() => {
    const list = terminals;
    if (scope === null || scope.terminalId !== worker.id || layoutBusy > 0) return;
    untrack(() => {
      void list;
      reconcile();
    });
  });


  /** Editors follow a renamed file and close for a deleted one (asking first if they are dirty). */
  function onPathMoved(event: Event): void {
    const detail = (event as CustomEvent<PathMoved>).detail;
    if (!detail || detail.workerId !== worker.id) return;
    let changed = false;
    for (const tab of [...snapshot.tabs]) {
      const document = tab.document;
      if (!document || document.kind !== 'file') continue;
      if (document.path !== detail.from && !document.path.startsWith(`${detail.from}/`)) continue;
      if (detail.to === null) {
        void closeTabNow(tab.id);
      } else {
        tab.document = { ...document, path: detail.to + document.path.slice(detail.from.length) };
        changed = true;
      }
    }
    if (changed) persist();
  }

  onMount(() => {
    window.addEventListener(workspacePathMovedEvent, onPathMoved);
    window.addEventListener('keyup', onWindowKeyup);
    window.addEventListener('blur', onWindowBlur);
    return () => {
      window.removeEventListener(workspacePathMovedEvent, onPathMoved);
      window.removeEventListener('keyup', onWindowKeyup);
      window.removeEventListener('blur', onWindowBlur);
    };
  });

  onDestroy(() => {
    if (toastTimer !== null) clearTimeout(toastTimer);
    if (suppressClickTimer !== null) clearTimeout(suppressClickTimer);
    terminalHandles.clear();
  });

  const rootLayout: LayoutNode = $derived(
    maximizedGroupId
      ? { type: 'leaf', groupId: maximizedGroupId }
      : snapshot.layout,
  );
  const draggedTab = $derived(ui.drag ? (tabById.get(ui.drag.tabId) ?? null) : null);
</script>

<svelte:window onpointerdown={onWindowPointerDown} onkeydown={internalEscape} />

<div class="flex h-full min-h-0 flex-col bg-background">
  {#if !hasSplits}
    <div class="titlebar" data-tauri-drag-region>
      {#if !sidebarVisible}
        <button
          type="button"
          class="mn-icon-button ml-1.5"
          aria-label="Show workspace sidebar"
          title="Show workspace sidebar (Ctrl+B)"
          onclick={onToggleSidebar}
        >
          <PanelLeft size={16} />
        </button>
      {/if}
      <div class="min-h-0 min-w-0 flex-1 self-stretch" data-tauri-drag-region>
        <WorkspaceTabStrip groupId={snapshot.activeGroupId} variant="titlebar" />
      </div>
      {#if !rightSidebarVisible}
      <button
        type="button"
        class="mn-icon-button mr-1.5"
        aria-label="Toggle right sidebar"
        title="Toggle right sidebar (Ctrl+L)"
        onclick={onToggleRightSidebar}
      >
        <PanelRight size={16} />
      </button>
        <div class="window-controls-titlebar-spacer"></div>
      {/if}
    </div>
  {/if}

  <div class="relative flex min-h-0 flex-1 flex-col overflow-hidden">
    <PaneTree
      root={rootLayout}
      showPanelStrips={hasSplits}
      {sidebarVisible}
      {rightSidebarVisible}
      {onToggleSidebar}
      {onToggleRightSidebar}
    />
  </div>
</div>

{#if menu}
  <div
    bind:this={menuElement}
    class="fixed z-[100] min-w-[13rem] max-w-[calc(100vw-1rem)] max-h-[min(70vh,640px)] overflow-y-auto overscroll-contain rounded-[11px] border border-border/80 bg-popover p-1 text-popover-foreground shadow-[0_16px_36px_rgba(0,0,0,0.24)] scrollbar-sleek"
    style:left={`${menu.x}px`}
    style:top={`${menu.y}px`}
    style:width={`${menu.width}px`}
    role="menu"
    tabindex="-1"
    onkeydown={onMenuKeydown}
    onfocusout={onMenuFocusOut}
  >
    {#each menu.items as entry}
      {#if entry.kind === 'separator'}
        <div class="my-1 h-px bg-border"></div>
      {:else if entry.kind === 'colors'}
        <div class="px-2 pb-1.5 pt-1">
          <div class="mb-1.5 text-2xs font-semibold text-muted-foreground">Tab Color</div>
          <div class="flex flex-wrap gap-2">
            {#each TAB_COLORS as color (color.label)}
              <button
                type="button"
                class="relative size-4 min-w-4 rounded-full border {color.value
                  ? 'border-transparent'
                  : 'border-muted-foreground/50 bg-transparent'}"
                style="background-color: {color.value ?? 'transparent'}"
                aria-label={color.label}
                title={color.label}
                onclick={() => {
                  const pick = entry.onPick;
                  closeMenu();
                  pick(color.value);
                }}
              >
                {#if color.value === null}
                  <span class="absolute left-1/2 top-1/2 h-px w-3 -translate-x-1/2 rotate-45 bg-muted-foreground/80"></span>
                {/if}
                {#if entry.selected === color.value}
                  <span class="pointer-events-none absolute inset-0 rounded-full ring-1 ring-foreground/70 ring-offset-1 ring-offset-popover"></span>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {:else if entry.kind === 'submenu'}
        <button type="button" role="menuitem" aria-haspopup="menu" aria-expanded={subMenu?.label === entry.label}
          class="flex h-7 w-full items-center gap-2 rounded-sm px-2 text-left text-xs hover:bg-accent"
          onpointerenter={event => openSubMenu(event.currentTarget,entry,false)}
          onkeydown={event => { if (event.key === 'ArrowRight') { event.preventDefault(); openSubMenu(event.currentTarget,entry,true); } }}
          onclick={event => openSubMenu(event.currentTarget,entry,true)}>
          <entry.icon size={14}/><span class="flex-1">{entry.label}</span><ChevronRight size={12}/>
        </button>
      {:else}
        {@const Icon = entry.icon}
        <button
          type="button"
          role="menuitem"
          class="flex h-7 w-full items-center gap-2 rounded-sm px-2 text-left text-xs transition-colors {entry.danger
            ? 'text-destructive hover:bg-destructive/10'
            : 'text-foreground hover:bg-accent hover:text-accent-foreground'} disabled:cursor-default disabled:opacity-40"
          disabled={entry.disabled}
          onpointerenter={() => { subMenu = null; }}
          onclick={() => {
            if (entry.disabled) return;
            const run = entry.onSelect;
            closeMenu();
            run();
          }}
        >
          {#if Icon}
            <Icon size={14} class="shrink-0" />
          {:else}
            <span class="size-3.5 shrink-0"></span>
          {/if}
          <span class="min-w-0 flex-1 truncate">{entry.label}</span>
          {#if entry.shortcut}
            <span class="ml-auto pl-3 text-2xs text-muted-foreground">{entry.shortcut}</span>
          {/if}
        </button>
      {/if}
    {/each}
  </div>
  {#if subMenu}
    <div bind:this={subMenuElement} role="menu" aria-label={subMenu.label} tabindex="-1" onkeydown={onMenuKeydown} onfocusout={onMenuFocusOut}
      class="fixed z-[101] rounded-[11px] border border-border/80 bg-popover p-1 text-popover-foreground shadow-[0_16px_36px_rgba(0,0,0,0.24)]"
      style:left={`${subMenu.x}px`} style:top={`${subMenu.y}px`} style:width={`${subMenu.width}px`}>
      {#each subMenu.items as entry}
        {#if entry.kind === 'item'}
          <button type="button" role="menuitem" disabled={entry.disabled}
            class="flex h-7 w-full items-center gap-2 rounded-sm px-2 text-left text-xs hover:bg-accent disabled:opacity-40"
            onclick={() => { closeMenu(); entry.onSelect(); }}>
            {#if entry.icon}<entry.icon size={14}/>{/if}<span class="flex-1">{entry.label}</span>
            {#if entry.shortcut}<span class="text-2xs text-muted-foreground">{entry.shortcut}</span>{/if}
          </button>
        {/if}
      {/each}
    </div>
  {/if}
{/if}

{#if confirmState}
  <dialog
    open
    class="fixed inset-0 z-[120] m-0 flex h-full w-full max-w-none items-center justify-center border-0 bg-black/50 p-4 text-foreground"
    aria-label={confirmState.title}
    oncancel={(event) => event.preventDefault()}
  >
    <div
      bind:this={confirmElement}
      class="w-full max-w-md overflow-hidden rounded-xl border border-border bg-dialog-surface p-6 text-foreground shadow-[0_24px_72px_rgba(0,0,0,0.55)]"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="workspace-confirm-title"
      aria-describedby="workspace-confirm-message"
    >
      <h2 id="workspace-confirm-title" class="text-lg font-semibold">{confirmState.title}</h2>
      <p id="workspace-confirm-message" class="mt-3 text-sm leading-relaxed text-muted-foreground">
        {confirmState.message}
      </p>
      <div class="mt-6 flex justify-end gap-2">
        <button
          type="button"
          class="mn-button"
          bind:this={confirmCancelButton}
          onclick={() => {
            confirmState = null;
          }}
        >
          Cancel
        </button>
        <button
          type="button"
          class="mn-button {confirmState.danger ? 'danger' : 'primary'}"
          onclick={() => {
            void runConfirm();
          }}
        >
          {confirmState.confirmLabel}
        </button>
      </div>
    </div>
  </dialog>
{/if}

{#if recent}
  <div class="pointer-events-none fixed inset-0 z-[110] flex items-start justify-center pt-[12vh]">
    <div
      class="pointer-events-auto w-[min(22.5rem,calc(100vw-3rem))] overflow-hidden rounded-lg border border-border bg-popover text-popover-foreground shadow-floating"
      role="listbox"
      aria-label="Switch tabs"
    >
      <div class="border-b border-border px-2 py-1 text-2xs font-semibold text-muted-foreground">
        Switch Tab
      </div>
      <div class="max-h-[min(18rem,60vh)] overflow-hidden py-0.5">
        {#each recent.ids as id, index (id)}
          {@const tab = tabById.get(id)}
          {@const switchTarget = tab ? terminalById.get(tabFocusTerminal(tab)) : null}
          {#if tab && (tab.document || switchTarget)}
            <button
              type="button"
              role="option"
              aria-selected={index === recent.index}
              class="flex h-6 w-full items-center gap-1.5 px-2 text-left text-xs {index ===
              recent.index
                ? 'bg-accent text-accent-foreground'
                : 'text-foreground hover:bg-accent/50'}"
              onclick={() => {
                if (!recent) return;
                recent = { ids: recent.ids, index };
                commitRecent();
              }}
            >
              <SquareTerminal size={14} class="shrink-0 text-muted-foreground" />
              <span class="min-w-0 flex-1 truncate">{tabLabel(tab)}</span>
              {#if tab.pinned}
                <Pin size={11} class="shrink-0 text-muted-foreground" />
              {/if}
              {#if tab.color}
                <span class="size-2 shrink-0 rounded-full" style="background-color: {tab.color}"></span>
              {/if}
            </button>
          {/if}
        {/each}
      </div>
    </div>
  </div>
{/if}

{#if ui.drag?.active && draggedTab}
  <div
    class="pointer-events-none fixed z-[105] flex h-7 max-w-[240px] items-center gap-1.5 rounded-md border border-border bg-popover px-2 text-xs text-popover-foreground shadow-floating"
    style:left={`${ui.drag.x + 14}px`}
    style:top={`${ui.drag.y + 12}px`}
  >
    <SquareTerminal size={13} class="shrink-0 text-muted-foreground" />
    <span class="truncate">{tabLabel(draggedTab)}</span>
  </div>
{/if}

{#if ui.toast}
  <div
    class="fixed bottom-9 right-4 z-[110] flex max-w-[356px] items-start gap-2 rounded-lg border border-border bg-popover px-3 py-2 text-xs text-popover-foreground shadow-floating"
    role="status"
  >
    <span class="min-w-0 flex-1">{ui.toast}</span>
    <button
      type="button"
      class="text-muted-foreground hover:text-foreground"
      aria-label="Dismiss notice"
      onclick={() => {
        ui.toast = null;
      }}
    >
      <X size={13} />
    </button>
  </div>
{/if}
