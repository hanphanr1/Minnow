import type { Worker } from '../types';
import type { UiPreferences } from './preferences';

export type SplitDirection = 'horizontal' | 'vertical';
export type DropPosition = 'left' | 'right' | 'up' | 'down';
export type DropZone = 'center' | DropPosition;

export interface WorkspaceGroup {
  id: string;
  tabIds: string[];
  activeTabId: string | null;
}

/** A terminal pane inside one tab. Worker IDs are real backend workers. */
export type PaneNode =
  | { type: 'leaf'; workerId: string }
  | {
      type: 'split';
      direction: SplitDirection;
      first: PaneNode;
      second: PaneNode;
      ratio: number;
    };

export interface WorkspaceTab {
  id: string;
  /** Primary worker (first leaf) kept for stable lookup and legacy migrations. */
  workerId: string;
  panes: PaneNode;
  focusedWorkerId: string;
  title: string | null;
  color: string | null;
  pinned: boolean;
}

export type LayoutNode =
  | { type: 'leaf'; groupId: string }
  | {
      type: 'split';
      direction: SplitDirection;
      first: LayoutNode;
      second: LayoutNode;
      ratio: number;
    };

export interface ClosedTabEntry {
  workerId: string;
  panes: PaneNode;
  focusedWorkerId: string;
  title: string | null;
  color: string | null;
  pinned: boolean;
  groupId: string;
  index: number;
}

/** A single pane expanded to fill its tab body; mutually exclusive with group maximize. */
export interface MaximizedPane {
  tabId: string;
  workerId: string;
}

export interface WorkspaceSnapshot {
  version: 2;
  tabs: WorkspaceTab[];
  groups: WorkspaceGroup[];
  layout: LayoutNode;
  activeGroupId: string;
  recentTabIds: string[];
  dismissedWorkerIds: string[];
  closedTabs: ClosedTabEntry[];
  maximizedGroupId: string | null;
  maximizedPane: MaximizedPane | null;
}

export interface TabInsertion {
  groupId: string;
  index: number;
  side: 'left' | 'right';
}

export interface DragState {
  tabId: string;
  active: boolean;
  x: number;
  y: number;
  insertion: TabInsertion | null;
  target: { groupId: string; zone: DropZone } | null;
}

export interface WorkspaceUi {
  drag: DragState | null;
  editingTabId: string | null;
  toast: string | null;
}

export interface TerminalHandle {
  focus(): void;
  focusSearch(): void;
  closeSearch(): void;
  isSearchOpen(): boolean;
  hasSelection(): boolean;
  copySelection(): Promise<boolean>;
  pasteClipboard(): Promise<boolean>;
  selectAll(): void;
  clear(): void;
  reset(): void;
  send(data: string): void;
}

export type WorkerAction = 'restart' | 'stop' | 'editor' | 'diff' | 'merge' | 'delete';

export interface WorkspaceApi {
  activateTab(tabId: string): void;
  activateWorker(workerId: string): void;
  focusPane(workerId: string): void;
  closeTab(tabId: string): void;
  closeOtherTabs(tabId: string): void;
  closeGroup(groupId: string): void;
  togglePin(tabId: string): void;
  setTabColor(tabId: string, color: string | null): void;
  setTabTitle(tabId: string, title: string | null): void;
  beginRename(tabId: string): void;
  commitRename(tabId: string, value: string): void;
  cancelRename(): void;
  moveTabToGroup(tabId: string, groupId: string, index?: number): void;
  moveTabToNewGroup(tabId: string, groupId: string, direction: DropPosition): void;
  splitWithNewWorker(groupId: string, direction: 'right' | 'down'): void;
  createTerminal(): void;
  reopenClosedTab(): void;
  focusGroup(groupId: string): void;
  setRatio(path: string, ratio: number): void;
  setPaneRatio(tabId: string, path: string, ratio: number): void;
  closePane(tabId: string, workerId: string): void;
  openPaneMenu(event: MouseEvent, tabId: string, workerId: string): void;
  equalizePanes(): void;
  toggleMaximize(groupId: string): void;
  beginTabDrag(event: PointerEvent, tabId: string): void;
  openNewTabMenu(anchor: DOMRect): void;
  openQuickCommandsMenu(anchor: DOMRect): void;
  openPaneActionsMenu(anchor: DOMRect, groupId: string): void;
  openTabMenu(event: MouseEvent, tab: WorkspaceTab): void;
  openTerminalMenu(event: MouseEvent, groupId: string): void;
  copyText(text: string, label: string): void;
  registerTerminal(
    workerId: string,
    handle: TerminalHandle | null,
    identity?: TerminalHandle,
  ): void;
  handleFor(workerId: string): TerminalHandle | undefined;
  isClickSuppressed(): boolean;
  showToast(message: string): void;
}

export interface WorkspaceView {
  snapshot: WorkspaceSnapshot;
  ui: WorkspaceUi;
  preferences: UiPreferences;
  workerById: Map<string, Worker>;
  tabById: Map<string, WorkspaceTab>;
  activeGroupId: string;
  hasSplits: boolean;
  maximizedGroupId: string | null;
  maximizedPane: MaximizedPane | null;
  sidebarVisible: boolean;
  allowTerminalFocus: boolean;
  activeTab: WorkspaceTab | null;
  activeWorker: Worker | null;
  onError: (message: string) => void;
}

export const WS_VIEW = 'minnow.workspace.view';
export const WS_API = 'minnow.workspace.api';

export const TAB_COLORS = [
  { label: 'None', value: null },
  { label: 'Blue', value: '#3b82f6' },
  { label: 'Purple', value: '#a855f7' },
  { label: 'Pink', value: '#ec4899' },
  { label: 'Red', value: '#ef4444' },
  { label: 'Orange', value: '#f97316' },
  { label: 'Yellow', value: '#eab308' },
  { label: 'Green', value: '#22c55e' },
  { label: 'Teal', value: '#14b8a6' },
  { label: 'Gray', value: '#9ca3af' },
] as const;

export const MIN_RATIO = 0.15;
export const MAX_RATIO = 0.85;

const STORAGE_PREFIX = 'minnow.orca.workspace.v1.';
const MAX_RECENT_TABS = 24;
const MAX_CLOSED_TABS = 12;

export function newId(prefix: string): string {
  const random = typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
    ? crypto.randomUUID()
    : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
  return `${prefix}-${random}`;
}

export function clampRatio(value: number): number {
  if (!Number.isFinite(value)) return 0.5;
  return Math.min(MAX_RATIO, Math.max(MIN_RATIO, value));
}

export function createSnapshot(workers: Worker[], activeWorkerId: string | null): WorkspaceSnapshot {
  const group: WorkspaceGroup = { id: newId('group'), tabIds: [], activeTabId: null };
  const snapshot: WorkspaceSnapshot = {
    version: 2,
    tabs: [],
    groups: [group],
    layout: { type: 'leaf', groupId: group.id },
    activeGroupId: group.id,
    recentTabIds: [],
    dismissedWorkerIds: [],
    closedTabs: [],
    maximizedGroupId: null,
    maximizedPane: null,
  };
  for (const worker of workers) {
    if (worker.status === 'exited' || (worker.pid === null && !worker.desired_running)) continue;
    const tab = addWorkerTab(snapshot, worker.id, group.id);
    if (tab !== null && group.activeTabId === null) group.activeTabId = tab;
  }
  const preferred = activeWorkerId
    ? (snapshot.tabs.find((tab) => tabHasWorker(tab, activeWorkerId)) ?? null)
    : null;
  if (preferred) {
    group.activeTabId = preferred.id;
    preferred.focusedWorkerId = activeWorkerId ?? preferred.workerId;
    markRecent(snapshot, preferred.id);
  }
  return snapshot;
}

export function loadSnapshot(
  projectId: string,
  workers: Worker[],
  activeWorkerId: string | null,
): WorkspaceSnapshot {
  const stored = readStored(projectId);
  const snapshot = stored ?? createSnapshot(workers, activeWorkerId);
  reconcileWorkers(snapshot, workers);
  return snapshot;
}

export function saveSnapshot(projectId: string, snapshot: WorkspaceSnapshot): void {
  if (typeof localStorage === 'undefined') return;
  const payload: WorkspaceSnapshot = {
    version: 2,
    tabs: snapshot.tabs.map((tab) => ({
      id: tab.id,
      workerId: tab.workerId,
      panes: clonePane(tab.panes ?? paneLeaf(tab.workerId)),
      focusedWorkerId: tab.focusedWorkerId || tab.workerId,
      title: tab.title,
      color: tab.color,
      pinned: tab.pinned,
    })),
    groups: snapshot.groups.map((group) => ({
      id: group.id,
      tabIds: [...group.tabIds],
      activeTabId: group.activeTabId,
    })),
    layout: cloneLayout(snapshot.layout),
    activeGroupId: snapshot.activeGroupId,
    recentTabIds: [...snapshot.recentTabIds],
    dismissedWorkerIds: [...snapshot.dismissedWorkerIds],
    closedTabs: snapshot.closedTabs.map((entry) => ({
      ...entry,
      panes: clonePane(entry.panes ?? paneLeaf(entry.workerId)),
    })),
    maximizedGroupId: snapshot.maximizedGroupId,
    maximizedPane: snapshot.maximizedPane ? { ...snapshot.maximizedPane } : null,
  };
  try {
    localStorage.setItem(STORAGE_PREFIX + projectId, JSON.stringify(payload));
  } catch {
    // A full or unavailable store must not break the workspace.
  }
}

function readStored(projectId: string): WorkspaceSnapshot | null {
  if (typeof localStorage === 'undefined') return null;
  try {
    const raw = localStorage.getItem(STORAGE_PREFIX + projectId);
    if (!raw) return null;
    return parseSnapshot(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function cloneLayout(node: LayoutNode): LayoutNode {
  if (node.type === 'leaf') return { type: 'leaf', groupId: node.groupId };
  return {
    type: 'split',
    direction: node.direction,
    first: cloneLayout(node.first),
    second: cloneLayout(node.second),
    ratio: node.ratio,
  };
}

export function leafIds(node: LayoutNode): string[] {
  if (node.type === 'leaf') return [node.groupId];
  return [...leafIds(node.first), ...leafIds(node.second)];
}

export function layoutHasGroup(node: LayoutNode, groupId: string): boolean {
  if (node.type === 'leaf') return node.groupId === groupId;
  return layoutHasGroup(node.first, groupId) || layoutHasGroup(node.second, groupId);
}

export function replaceLeaf(node: LayoutNode, groupId: string, replacement: LayoutNode): LayoutNode {
  if (node.type === 'leaf') return node.groupId === groupId ? replacement : node;
  return {
    ...node,
    first: replaceLeaf(node.first, groupId, replacement),
    second: replaceLeaf(node.second, groupId, replacement),
  };
}

export function removeLeaf(node: LayoutNode, groupId: string): LayoutNode | null {
  if (node.type === 'leaf') return node.groupId === groupId ? null : node;
  const first = removeLeaf(node.first, groupId);
  const second = removeLeaf(node.second, groupId);
  if (first === null) return second;
  if (second === null) return first;
  return { ...node, first, second };
}

export function updateRatio(node: LayoutNode, path: string, ratio: number): LayoutNode {
  if (node.type !== 'split') return node;
  if (path.length === 0) return { ...node, ratio: clampRatio(ratio) };
  const [head, ...rest] = path.split('.');
  const next = rest.join('.');
  if (head === 'first') return { ...node, first: updateRatio(node.first, next, ratio) };
  if (head === 'second') return { ...node, second: updateRatio(node.second, next, ratio) };
  return node;
}

export function equalizeLayout(node: LayoutNode): LayoutNode {
  if (node.type === 'leaf') return node;
  return {
    ...node,
    ratio: 0.5,
    first: equalizeLayout(node.first),
    second: equalizeLayout(node.second),
  };
}

export function paneLeaf(workerId: string): PaneNode {
  return { type: 'leaf', workerId };
}

export function clonePane(node: PaneNode): PaneNode {
  if (node.type === 'leaf') return { type: 'leaf', workerId: node.workerId };
  return {
    type: 'split',
    direction: node.direction,
    first: clonePane(node.first),
    second: clonePane(node.second),
    ratio: node.ratio,
  };
}

export function paneWorkerIds(node: PaneNode): string[] {
  if (node.type === 'leaf') return [node.workerId];
  return [...paneWorkerIds(node.first), ...paneWorkerIds(node.second)];
}

export function paneFirstWorker(node: PaneNode): string | null {
  if (node.type === 'leaf') return node.workerId;
  return paneFirstWorker(node.first);
}

export function paneHasWorker(node: PaneNode, workerId: string): boolean {
  return paneWorkerIds(node).includes(workerId);
}

export function replacePaneLeaf(
  node: PaneNode,
  workerId: string,
  replacement: PaneNode,
): PaneNode {
  if (node.type === 'leaf') return node.workerId === workerId ? replacement : node;
  return {
    ...node,
    first: replacePaneLeaf(node.first, workerId, replacement),
    second: replacePaneLeaf(node.second, workerId, replacement),
  };
}

export function removePaneLeaf(node: PaneNode, workerId: string): PaneNode | null {
  if (node.type === 'leaf') return node.workerId === workerId ? null : node;
  const first = removePaneLeaf(node.first, workerId);
  const second = removePaneLeaf(node.second, workerId);
  if (first === null) return second;
  if (second === null) return first;
  return { ...node, first, second };
}

export function updatePaneRatio(node: PaneNode, path: string, ratio: number): PaneNode {
  if (node.type === 'leaf') return node;
  if (path.length === 0) return { ...node, ratio: clampRatio(ratio) };
  const [head, ...rest] = path.split('.');
  const next = rest.join('.');
  if (head === 'first') return { ...node, first: updatePaneRatio(node.first, next, ratio) };
  if (head === 'second') return { ...node, second: updatePaneRatio(node.second, next, ratio) };
  return node;
}

export function equalizePane(node: PaneNode): PaneNode {
  if (node.type === 'leaf') return node;
  return {
    ...node,
    ratio: 0.5,
    first: equalizePane(node.first),
    second: equalizePane(node.second),
  };
}

export function splitPane(
  node: PaneNode,
  targetWorkerId: string,
  newWorkerId: string,
  direction: 'right' | 'down',
): PaneNode {
  const target: PaneNode = { type: 'leaf', workerId: targetWorkerId };
  const replacement: PaneNode = {
    type: 'split',
    direction: direction === 'right' ? 'horizontal' : 'vertical',
    first: target,
    second: { type: 'leaf', workerId: newWorkerId },
    ratio: 0.5,
  };
  return replacePaneLeaf(node, targetWorkerId, replacement);
}

/** Normalises legacy/partial tab data so callers can always traverse the pane tree. */
export function normalizeTab(tab: WorkspaceTab): void {
  if (!tab.panes || !('type' in tab.panes)) tab.panes = paneLeaf(tab.workerId);
  const ids = paneWorkerIds(tab.panes);
  if (ids.length === 0) tab.panes = paneLeaf(tab.workerId);
  const workers = paneWorkerIds(tab.panes);
  tab.workerId = workers[0];
  if (!tab.focusedWorkerId || !workers.includes(tab.focusedWorkerId)) {
    tab.focusedWorkerId = workers[0];
  }
}

export function tabWorkerIds(tab: WorkspaceTab): string[] {
  if (!tab.panes || !('type' in tab.panes)) return [tab.workerId];
  return paneWorkerIds(tab.panes);
}

export function tabHasWorker(tab: WorkspaceTab, workerId: string): boolean {
  return tabWorkerIds(tab).includes(workerId);
}

export function tabFocusWorker(tab: WorkspaceTab): string {
  const workers = tabWorkerIds(tab);
  return workers.includes(tab.focusedWorkerId) ? tab.focusedWorkerId : (workers[0] ?? tab.workerId);
}

/**
 * True when a pane expansion still describes the tab's visible focused pane.
 * Group expansion is mutually exclusive, so any group maximize invalidates it.
 */
export function isPaneMaximizeValid(snapshot: WorkspaceSnapshot, value: MaximizedPane): boolean {
  if (snapshot.maximizedGroupId !== null) return false;
  const tab = tabById(snapshot, value.tabId);
  if (!tab || !tabHasWorker(tab, value.workerId)) return false;
  if (tab.focusedWorkerId !== value.workerId) return false;
  return tabWorkerIds(tab).length > 1;
}

/** Removes one pane worker. Returns false when the tab has no remaining panes. */
export function removePaneFromTab(tab: WorkspaceTab, workerId: string): boolean {
  const next = removePaneLeaf(tab.panes ?? paneLeaf(tab.workerId), workerId);
  if (next === null) return false;
  tab.panes = next;
  normalizeTab(tab);
  return true;
}

export function splitDirectionFor(position: DropPosition): SplitDirection {
  return position === 'left' || position === 'right' ? 'horizontal' : 'vertical';
}

export function splitPositionFor(position: DropPosition): 'first' | 'second' {
  return position === 'left' || position === 'up' ? 'first' : 'second';
}

export function groupById(snapshot: WorkspaceSnapshot, groupId: string): WorkspaceGroup | null {
  return snapshot.groups.find((group) => group.id === groupId) ?? null;
}

export function tabById(snapshot: WorkspaceSnapshot, tabId: string): WorkspaceTab | null {
  return snapshot.tabs.find((tab) => tab.id === tabId) ?? null;
}

export function tabByWorker(snapshot: WorkspaceSnapshot, workerId: string): WorkspaceTab | null {
  return snapshot.tabs.find((tab) => tabHasWorker(tab, workerId)) ?? null;
}

export function groupContainingTab(snapshot: WorkspaceSnapshot, tabId: string): WorkspaceGroup | null {
  return snapshot.groups.find((group) => group.tabIds.includes(tabId)) ?? null;
}

export function activeTabOfGroup(snapshot: WorkspaceSnapshot, groupId: string): WorkspaceTab | null {
  const group = groupById(snapshot, groupId);
  if (!group || group.activeTabId === null) return null;
  return tabById(snapshot, group.activeTabId);
}

export function groupTabIndex(snapshot: WorkspaceSnapshot, tabId: string): number {
  const group = groupContainingTab(snapshot, tabId);
  return group ? group.tabIds.indexOf(tabId) : -1;
}

/** Next tab inside one group, wrapping at the ends. Returns null when the group is empty. */
export function adjacentTabId(
  snapshot: WorkspaceSnapshot,
  groupId: string,
  offset: 1 | -1,
): string | null {
  const group = groupById(snapshot, groupId);
  if (!group || group.tabIds.length === 0) return null;
  const current = group.activeTabId === null ? -1 : group.tabIds.indexOf(group.activeTabId);
  const index = current === -1 ? (offset === 1 ? 0 : group.tabIds.length - 1) : current;
  const next = (index + offset + group.tabIds.length) % group.tabIds.length;
  return group.tabIds[next] ?? null;
}

/** Adjacent group in layout order for pane focus shortcuts. */
export function adjacentGroupId(
  snapshot: WorkspaceSnapshot,
  groupId: string,
  offset: 1 | -1,
): string | null {
  const order = leafIds(snapshot.layout).filter((id) => groupById(snapshot, id) !== null);
  if (order.length === 0) return null;
  const index = order.indexOf(groupId);
  const next = (index === -1 ? 0 : index + offset + order.length) % order.length;
  return order[next] ?? null;
}

export function addWorkerTab(
  snapshot: WorkspaceSnapshot,
  workerId: string,
  groupId: string,
  index?: number,
): string | null {
  const existing = tabByWorker(snapshot, workerId);
  if (existing) return existing.id;
  const group = groupById(snapshot, groupId);
  if (!group) return null;
  const tab: WorkspaceTab = {
    id: newId('tab'),
    workerId,
    panes: paneLeaf(workerId),
    focusedWorkerId: workerId,
    title: null,
    color: null,
    pinned: false,
  };
  snapshot.tabs = [...snapshot.tabs, tab];
  const at = index === undefined ? group.tabIds.length : Math.min(Math.max(index, 0), group.tabIds.length);
  group.tabIds = [...group.tabIds.slice(0,at),tab.id,...group.tabIds.slice(at)];
  if (group.activeTabId === null) group.activeTabId = tab.id;
  return tab.id;
}

export function removeTab(snapshot: WorkspaceSnapshot, tabId: string): void {
  snapshot.tabs = snapshot.tabs.filter((tab) => tab.id !== tabId);
  snapshot.recentTabIds = snapshot.recentTabIds.filter((id) => id !== tabId);
  if (snapshot.maximizedPane?.tabId === tabId) snapshot.maximizedPane = null;
  for (const group of snapshot.groups) {
    const index = group.tabIds.indexOf(tabId);
    if (index === -1) continue;
    group.tabIds.splice(index, 1);
    if (group.activeTabId === tabId) {
      const next = group.tabIds[index] ?? group.tabIds[index - 1] ?? null;
      group.activeTabId = next;
    }
  }
}

export function collapseGroup(snapshot: WorkspaceSnapshot, groupId: string): void {
  if (snapshot.groups.length <= 1) return;
  const layout = removeLeaf(snapshot.layout, groupId);
  if (layout === null) return;
  snapshot.layout = layout;
  const group = groupById(snapshot, groupId);
  if (group) {
    for (const tabId of [...group.tabIds]) removeTab(snapshot, tabId);
  }
  snapshot.groups = snapshot.groups.filter((candidate) => candidate.id !== groupId);
  if (snapshot.activeGroupId === groupId) {
    snapshot.activeGroupId = leafIds(snapshot.layout)[0];
  }
  if (snapshot.maximizedGroupId === groupId) snapshot.maximizedGroupId = null;
}

export function moveTab(
  snapshot: WorkspaceSnapshot,
  tabId: string,
  targetGroupId: string,
  index?: number,
  collapseEmptySource = true,
): boolean {
  const tab = tabById(snapshot, tabId);
  const source = groupContainingTab(snapshot, tabId);
  const target = groupById(snapshot, targetGroupId);
  if (!tab || !source || !target) return false;
  let insertion = index;
  const sourceIndex = source.tabIds.indexOf(tabId);
  if (source === target) {
    if (insertion === undefined) return false;
    if (insertion > sourceIndex) insertion -= 1;
    if (insertion === sourceIndex) return false;
  }
  source.tabIds.splice(sourceIndex, 1);
  const at = insertion === undefined
    ? target.tabIds.length
    : Math.min(Math.max(insertion, 0), target.tabIds.length);
  target.tabIds.splice(at, 0, tabId);
  if (source.activeTabId === tabId) {
    source.activeTabId = source.tabIds[sourceIndex] ?? source.tabIds[sourceIndex - 1] ?? null;
  }
  target.activeTabId = tabId;
  snapshot.activeGroupId = target.id;
  markRecent(snapshot, tabId);
  if (collapseEmptySource && source !== target && source.tabIds.length === 0) {
    collapseGroup(snapshot, source.id);
  }
  return true;
}

export function splitWithTab(
  snapshot: WorkspaceSnapshot,
  tabId: string,
  targetGroupId: string,
  position: DropPosition,
): boolean {
  if (!layoutHasGroup(snapshot.layout, targetGroupId)) return false;
  const tab = tabById(snapshot, tabId);
  const source = groupContainingTab(snapshot, tabId);
  const target = groupById(snapshot, targetGroupId);
  if (!tab || !source || !target) return false;
  if (source.tabIds.length <= 1) return false;
  const newGroup: WorkspaceGroup = { id: newId('group'), tabIds: [tabId], activeTabId: tabId };
  const sourceIndex = source.tabIds.indexOf(tabId);
  source.tabIds.splice(sourceIndex, 1);
  if (source.activeTabId === tabId) {
    source.activeTabId = source.tabIds[sourceIndex] ?? source.tabIds[sourceIndex - 1] ?? null;
  }
  snapshot.groups.push(newGroup);
  const replacement: LayoutNode = {
    type: 'split',
    direction: splitDirectionFor(position),
    first:
      splitPositionFor(position) === 'first'
        ? { type: 'leaf', groupId: newGroup.id }
        : { type: 'leaf', groupId: targetGroupId },
    second:
      splitPositionFor(position) === 'second'
        ? { type: 'leaf', groupId: newGroup.id }
        : { type: 'leaf', groupId: targetGroupId },
    ratio: 0.5,
  };
  snapshot.layout = replaceLeaf(snapshot.layout, targetGroupId, replacement);
  snapshot.activeGroupId = newGroup.id;
  markRecent(snapshot, tabId);
  return true;
}

/**
 * Moves a tab into a fresh group inserted beside `targetGroupId`.
 * `allowEmptySource` lets a split command leave the original pane empty; a
 * drag of the only tab out of its own pane should not silently become a move.
 */
export function moveTabToNewGroup(
  snapshot: WorkspaceSnapshot,
  tabId: string,
  targetGroupId: string,
  position: DropPosition,
  allowEmptySource = false,
): boolean {
  const tab = tabById(snapshot, tabId);
  const source = groupContainingTab(snapshot, tabId);
  const target = groupById(snapshot, targetGroupId);
  if (!tab || !source || !target) return false;
  if (!layoutHasGroup(snapshot.layout, targetGroupId)) return false;
  if (source.id === target.id && source.tabIds.length <= 1 && !allowEmptySource) return false;

  const newGroup: WorkspaceGroup = { id: newId('group'), tabIds: [], activeTabId: null };
  snapshot.groups.push(newGroup);
  if (!moveTab(snapshot, tabId, newGroup.id, undefined, false)) {
    snapshot.groups = snapshot.groups.filter((group) => group.id !== newGroup.id);
    return false;
  }
  const replacement: LayoutNode = {
    type: 'split',
    direction: splitDirectionFor(position),
    first:
      splitPositionFor(position) === 'first'
        ? { type: 'leaf', groupId: newGroup.id }
        : { type: 'leaf', groupId: targetGroupId },
    second:
      splitPositionFor(position) === 'second'
        ? { type: 'leaf', groupId: newGroup.id }
        : { type: 'leaf', groupId: targetGroupId },
    ratio: 0.5,
  };
  snapshot.layout = replaceLeaf(snapshot.layout, targetGroupId, replacement);
  snapshot.activeGroupId = newGroup.id;
  snapshot.maximizedGroupId = null;
  markRecent(snapshot, tabId);
  // Removing the emptied source leaf after the split is inserted keeps the
  // target leaf and the new group intact.
  if (source.id !== target.id && source.tabIds.length === 0) {
    collapseGroup(snapshot, source.id);
  }
  return true;
}

export function splitWithWorker(
  snapshot: WorkspaceSnapshot,
  workerId: string,
  targetGroupId: string,
  direction: 'right' | 'down',
): boolean {
  const target = groupById(snapshot, targetGroupId);
  if (!target) return false;
  const existing = tabByWorker(snapshot, workerId);
  let tabId = existing?.id ?? null;
  if (tabId === null) {
    tabId = addWorkerTab(snapshot, workerId, targetGroupId);
  } else {
    const source = groupContainingTab(snapshot, tabId);
    if (!source) return false;
    if (source.id !== targetGroupId && !moveTab(snapshot, tabId, targetGroupId)) return false;
  }
  if (tabId === null) return false;
  return moveTabToNewGroup(
    snapshot,
    tabId,
    targetGroupId,
    direction === 'right' ? 'right' : 'down',
    true,
  );
}

export function markRecent(snapshot: WorkspaceSnapshot, tabId: string): void {
  snapshot.recentTabIds = [tabId, ...snapshot.recentTabIds.filter((id) => id !== tabId)].slice(
    0,
    MAX_RECENT_TABS,
  );
}

export function pushClosedTab(snapshot: WorkspaceSnapshot, entry: ClosedTabEntry): void {
  snapshot.closedTabs = [entry, ...snapshot.closedTabs].slice(0, MAX_CLOSED_TABS);
}

export function dismissWorker(snapshot: WorkspaceSnapshot, workerId: string): void {
  if (!snapshot.dismissedWorkerIds.includes(workerId)) snapshot.dismissedWorkerIds.push(workerId);
}

export function undismissWorker(snapshot: WorkspaceSnapshot, workerId: string): void {
  snapshot.dismissedWorkerIds = snapshot.dismissedWorkerIds.filter((id) => id !== workerId);
}

function prunePaneWorkers(node: PaneNode, ids: Set<string>): PaneNode | null {
  if (node.type === 'leaf') return ids.has(node.workerId) ? node : null;
  const first = prunePaneWorkers(node.first, ids);
  const second = prunePaneWorkers(node.second, ids);
  if (first === null) return second;
  if (second === null) return first;
  return { ...node, first, second };
}

export function reconcileWorkers(snapshot: WorkspaceSnapshot, workers: Worker[]): void {
  const ids = new Set(workers.map((worker) => worker.id));

  for (const tab of [...snapshot.tabs]) {
    normalizeTab(tab);
    let empty = false;
    for (const workerId of tabWorkerIds(tab)) {
      if (ids.has(workerId)) continue;
      if (!removePaneFromTab(tab, workerId)) {
        removeTab(snapshot, tab.id);
        empty = true;
        break;
      }
    }
    if (!empty) normalizeTab(tab);
  }

  snapshot.closedTabs = snapshot.closedTabs
    .map((entry) => {
      const panes = prunePaneWorkers(entry.panes ?? paneLeaf(entry.workerId), ids);
      if (panes === null) return null;
      const paneIds = paneWorkerIds(panes);
      return {
        ...entry,
        workerId: paneIds[0],
        panes,
        focusedWorkerId: paneIds.includes(entry.focusedWorkerId)
          ? entry.focusedWorkerId
          : paneIds[0],
      };
    })
    .filter((entry): entry is ClosedTabEntry => entry !== null);
  snapshot.dismissedWorkerIds = snapshot.dismissedWorkerIds.filter((id) => ids.has(id));

  const seen = new Set<string>();
  for (const tab of [...snapshot.tabs]) {
    normalizeTab(tab);
    let removed = false;
    for (const workerId of tabWorkerIds(tab)) {
      if (seen.has(workerId)) {
        if (!removePaneFromTab(tab, workerId)) {
          removeTab(snapshot, tab.id);
          removed = true;
          break;
        }
      } else {
        seen.add(workerId);
      }
    }
    if (!removed) normalizeTab(tab);
  }

  let reopened = false;
  for (const worker of workers) {
    if (seen.has(worker.id)) continue;
    // Exited/stopped workers stay hidden: reconcile never revives them.
    if (worker.status === 'exited' || (worker.pid === null && !worker.desired_running)) continue;
    if (snapshot.dismissedWorkerIds.includes(worker.id)) {
      // A dismissed worker is normally a closed tab (stopped). A still-live
      // dismissed worker is an orphan from an interrupted split; restore it.
      undismissWorker(snapshot, worker.id);
    }
    const group = groupById(snapshot, snapshot.activeGroupId) ?? snapshot.groups[0];
    if (!group) continue;
    const tabId = addWorkerTab(snapshot, worker.id, group.id);
    if (tabId) {
      seen.add(worker.id);
      if (!reopened && group.activeTabId === null) group.activeTabId = tabId;
      reopened = true;
    }
  }

  for (const group of snapshot.groups) {
    if (group.activeTabId !== null && !group.tabIds.includes(group.activeTabId)) {
      group.activeTabId = group.tabIds[0] ?? null;
    }
    if (group.activeTabId === null && group.tabIds.length > 0) {
      group.activeTabId = group.tabIds[0];
    }
  }

  // Persisted layouts from older builds can retain blank split groups.
  for (const group of [...snapshot.groups]) {
    if (group.tabIds.length === 0 && snapshot.groups.length > 1) {
      collapseGroup(snapshot, group.id);
    }
  }

  if (!layoutHasGroup(snapshot.layout, snapshot.activeGroupId)) {
    snapshot.activeGroupId = leafIds(snapshot.layout)[0];
  }
  if (
    snapshot.maximizedGroupId !== null &&
    !layoutHasGroup(snapshot.layout, snapshot.maximizedGroupId)
  ) {
    snapshot.maximizedGroupId = null;
  }
  if (snapshot.maximizedPane && !isPaneMaximizeValid(snapshot, snapshot.maximizedPane)) {
    snapshot.maximizedPane = null;
  }
  snapshot.recentTabIds = snapshot.recentTabIds.filter((id) =>
    snapshot.tabs.some((tab) => tab.id === id),
  );
}

function parseSnapshot(raw: unknown): WorkspaceSnapshot | null {
  if (typeof raw !== 'object' || raw === null) return null;
  if (!('version' in raw) || (raw.version !== 1 && raw.version !== 2)) return null;

  const tabs: WorkspaceTab[] = [];
  if ('tabs' in raw && Array.isArray(raw.tabs)) {
    for (const value of raw.tabs) {
      const tab = parseWorkspaceTab(value);
      if (tab) tabs.push(tab);
    }
  }

  const layout = parseLayout('layout' in raw ? raw.layout : null);
  if (!layout) return null;
  const leaves = new Set(leafIds(layout));

  const groups: WorkspaceGroup[] = [];
  const assigned = new Set<string>();
  if ('groups' in raw && Array.isArray(raw.groups)) {
    for (const value of raw.groups) {
      const group = parseGroup(value, leaves, tabs, assigned);
      if (!group) continue;
      groups.push(group);
    }
  }
  if (groups.length !== leaves.size) return null;
  if (tabs.some((tab) => !assigned.has(tab.id))) return null;
  const workerIds = new Set<string>();
  for (const tab of tabs) {
    normalizeTab(tab);
    for (const workerId of tabWorkerIds(tab)) {
      if (workerIds.has(workerId)) return null;
      workerIds.add(workerId);
    }
  }

  const activeGroupId =
    'activeGroupId' in raw &&
    typeof raw.activeGroupId === 'string' &&
    groups.some((group) => group.id === raw.activeGroupId)
      ? raw.activeGroupId
      : groups[0].id;

  const recentTabIds: string[] = [];
  if ('recentTabIds' in raw && Array.isArray(raw.recentTabIds)) {
    for (const value of raw.recentTabIds) {
      if (typeof value === 'string' && tabs.some((tab) => tab.id === value) && !recentTabIds.includes(value)) {
        recentTabIds.push(value);
      }
    }
  }

  const dismissedWorkerIds: string[] = [];
  if ('dismissedWorkerIds' in raw && Array.isArray(raw.dismissedWorkerIds)) {
    for (const value of raw.dismissedWorkerIds) {
      if (typeof value === 'string' && !dismissedWorkerIds.includes(value)) dismissedWorkerIds.push(value);
    }
  }

  const closedTabs: ClosedTabEntry[] = [];
  if ('closedTabs' in raw && Array.isArray(raw.closedTabs)) {
    for (const value of raw.closedTabs) {
      const entry = parseClosedTab(value, groups[0].id);
      if (!entry) continue;
      closedTabs.push(entry);
      if (closedTabs.length >= MAX_CLOSED_TABS) break;
    }
  }

  const maximizedGroupId =
    'maximizedGroupId' in raw &&
    typeof raw.maximizedGroupId === 'string' &&
    leaves.has(raw.maximizedGroupId)
      ? raw.maximizedGroupId
      : null;

  let maximizedPane: MaximizedPane | null = null;
  if (
    maximizedGroupId === null &&
    'maximizedPane' in raw &&
    typeof raw.maximizedPane === 'object' &&
    raw.maximizedPane !== null
  ) {
    const value = raw.maximizedPane as Record<string, unknown>;
    if (typeof value.tabId === 'string' && typeof value.workerId === 'string') {
      const tab = tabs.find((candidate) => candidate.id === value.tabId);
      if (
        tab &&
        tab.focusedWorkerId === value.workerId &&
        tabWorkerIds(tab).length > 1 &&
        tabHasWorker(tab, value.workerId)
      ) {
        maximizedPane = { tabId: tab.id, workerId: value.workerId };
      }
    }
  }

  return {
    version: 2,
    tabs,
    groups,
    layout,
    activeGroupId,
    recentTabIds,
    dismissedWorkerIds,
    closedTabs,
    maximizedGroupId,
    maximizedPane,
  };
}

function parseWorkspaceTab(value: unknown): WorkspaceTab | null {
  if (typeof value !== 'object' || value === null) return null;
  if (!('id' in value) || typeof value.id !== 'string') return null;
  if (!('workerId' in value) || typeof value.workerId !== 'string') return null;
  const parsed = 'panes' in value ? parsePane(value.panes) : null;
  const panes = parsed ?? paneLeaf(value.workerId);
  const workers = paneWorkerIds(panes);
  if (workers.length === 0 || new Set(workers).size !== workers.length) return null;
  const focused =
    'focusedWorkerId' in value &&
    typeof value.focusedWorkerId === 'string' &&
    workers.includes(value.focusedWorkerId)
      ? value.focusedWorkerId
      : workers[0];
  return {
    id: value.id,
    workerId: workers[0],
    panes,
    focusedWorkerId: focused,
    title: 'title' in value && typeof value.title === 'string' ? value.title : null,
    color: 'color' in value && typeof value.color === 'string' ? value.color : null,
    pinned: 'pinned' in value && value.pinned === true,
  };
}

function parsePane(raw: unknown): PaneNode | null {
  if (typeof raw !== 'object' || raw === null) return null;
  if (!('type' in raw)) return null;
  if (raw.type === 'leaf') {
    if (!('workerId' in raw) || typeof raw.workerId !== 'string') return null;
    return { type: 'leaf', workerId: raw.workerId };
  }
  if (raw.type !== 'split') return null;
  if (!('direction' in raw) || (raw.direction !== 'horizontal' && raw.direction !== 'vertical')) {
    return null;
  }
  if (!('first' in raw) || !('second' in raw)) return null;
  const first = parsePane(raw.first);
  const second = parsePane(raw.second);
  if (!first || !second) return null;
  const ratio = 'ratio' in raw && typeof raw.ratio === 'number' ? clampRatio(raw.ratio) : 0.5;
  return { type: 'split', direction: raw.direction, first, second, ratio };
}

function parseGroup(
  value: unknown,
  leaves: Set<string>,
  tabs: WorkspaceTab[],
  assigned: Set<string>,
): WorkspaceGroup | null {
  if (typeof value !== 'object' || value === null) return null;
  if (!('id' in value) || typeof value.id !== 'string' || !leaves.has(value.id)) return null;
  const id = value.id;
  if (assigned.has(`group:${id}`)) return null;
  const tabIds: string[] = [];
  if ('tabIds' in value && Array.isArray(value.tabIds)) {
    for (const candidate of value.tabIds) {
      if (typeof candidate !== 'string' || assigned.has(candidate)) continue;
      if (!tabs.some((tab) => tab.id === candidate)) continue;
      assigned.add(candidate);
      tabIds.push(candidate);
    }
  }
  const activeTabId =
    'activeTabId' in value && typeof value.activeTabId === 'string' && tabIds.includes(value.activeTabId)
      ? value.activeTabId
      : (tabIds[0] ?? null);
  return { id, tabIds, activeTabId };
}

function parseClosedTab(value: unknown, fallbackGroupId: string): ClosedTabEntry | null {
  if (typeof value !== 'object' || value === null) return null;
  if (!('workerId' in value) || typeof value.workerId !== 'string') return null;
  const parsed = 'panes' in value ? parsePane(value.panes) : null;
  const panes = parsed ?? paneLeaf(value.workerId);
  const workers = paneWorkerIds(panes);
  if (workers.length === 0 || new Set(workers).size !== workers.length) return null;
  const focused =
    'focusedWorkerId' in value &&
    typeof value.focusedWorkerId === 'string' &&
    workers.includes(value.focusedWorkerId)
      ? value.focusedWorkerId
      : workers[0];
  return {
    workerId: workers[0],
    panes,
    focusedWorkerId: focused,
    title: 'title' in value && typeof value.title === 'string' ? value.title : null,
    color: 'color' in value && typeof value.color === 'string' ? value.color : null,
    pinned: 'pinned' in value && value.pinned === true,
    groupId: 'groupId' in value && typeof value.groupId === 'string' ? value.groupId : fallbackGroupId,
    index:
      'index' in value && typeof value.index === 'number' && Number.isFinite(value.index)
        ? value.index
        : 0,
  };
}

function parseLayout(raw: unknown): LayoutNode | null {
  if (typeof raw !== 'object' || raw === null) return null;
  if (!('type' in raw)) return null;
  if (raw.type === 'leaf') {
    if (!('groupId' in raw) || typeof raw.groupId !== 'string') return null;
    return { type: 'leaf', groupId: raw.groupId };
  }
  if (raw.type !== 'split') return null;
  if (!('direction' in raw) || (raw.direction !== 'horizontal' && raw.direction !== 'vertical')) {
    return null;
  }
  if (!('first' in raw) || !('second' in raw)) return null;
  const first = parseLayout(raw.first);
  const second = parseLayout(raw.second);
  if (!first || !second) return null;
  const ratio = 'ratio' in raw && typeof raw.ratio === 'number' ? clampRatio(raw.ratio) : 0.5;
  return { type: 'split', direction: raw.direction, first, second, ratio };
}

export function isTextEntryTarget(target: EventTarget | null): boolean {
  const element = target instanceof HTMLElement ? target : null;
  if (!element) return false;
  const tag = element.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  return element.isContentEditable;
}
