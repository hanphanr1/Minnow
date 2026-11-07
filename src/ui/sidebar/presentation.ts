/*
 * Sidebar presentation state ported from Orca e507dd7, MIT.
 * Orca keeps grouping/collapse in its store; Minnow persists the same concepts
 * locally because they only describe how workspaces are displayed.
 */
import type { Plus } from 'lucide-svelte';
import type { Worker } from '../../types';

export const sidebarPresentationKey = 'minnow.sidebar.presentation.v1';

export type SidebarGroupBy = 'project' | 'flat';

export interface SidebarPresentation {
  groupBy: SidebarGroupBy;
  collapsedProjectIds: string[];
  pinnedWorkerIds: string[];
  pinnedCollapsed: boolean;
}

export const defaultSidebarPresentation: SidebarPresentation = {
  groupBy: 'project',
  collapsedProjectIds: [],
  pinnedWorkerIds: [],
  pinnedCollapsed: false,
};

export interface MenuAnchor {
  left: number;
  right?: number;
  top: number;
  bottom: number;
}

/** lucide-svelte 0.577 declares icons as Svelte 4 classes; every icon shares this shape. */
export type SidebarMenuIcon = typeof Plus;

export interface SidebarMenuItem {
  id: string;
  label: string;
  icon?: SidebarMenuIcon;
  shortcut?: string;
  checked?: boolean;
  disabled?: boolean;
  danger?: boolean;
  separatorBefore?: boolean;
  onSelect: () => void;
}

export type WorkerStatusTone = 'working' | 'waiting' | 'idle' | 'failed' | 'stopped';

export interface WorkerStatusInfo {
  tone: WorkerStatusTone;
  label: string;
}

function stringList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((entry): entry is string => typeof entry === 'string');
}

export function loadSidebarPresentation(): SidebarPresentation {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(sidebarPresentationKey) ?? '{}');
    if (!parsed || typeof parsed !== 'object') return { ...defaultSidebarPresentation };
    const value = parsed as Record<string, unknown>;
    return {
      groupBy: value.groupBy === 'flat' ? 'flat' : 'project',
      collapsedProjectIds: stringList(value.collapsedProjectIds),
      pinnedWorkerIds: stringList(value.pinnedWorkerIds),
      pinnedCollapsed: value.pinnedCollapsed === true,
    };
  } catch {
    return { ...defaultSidebarPresentation };
  }
}

export function saveSidebarPresentation(value: SidebarPresentation): void {
  try {
    localStorage.setItem(
      sidebarPresentationKey,
      JSON.stringify({
        groupBy: value.groupBy,
        collapsedProjectIds: value.collapsedProjectIds,
        pinnedWorkerIds: value.pinnedWorkerIds,
        pinnedCollapsed: value.pinnedCollapsed,
      })
    );
  } catch {
    // Presentation state is a convenience; a locked-down webview must not break the sidebar.
  }
}

export function toggleListValue(list: readonly string[], id: string): string[] {
  return list.includes(id) ? list.filter((entry) => entry !== id) : [...list, id];
}

/** Project and workspace rows share one stable key space for roving focus. */
export function projectItemKey(projectId: string): string {
  return `project:${projectId}`;
}

export function workerItemKey(workerId: string): string {
  return `worker:${workerId}`;
}

export const pinnedItemKey = 'pinned';

export function sidebarItemSelector(key: string): string {
  const escaped = key.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  return `[data-sidebar-item="${escaped}"]`;
}

/** Orca's StatusIndicator states mapped onto the worker lifecycle the PTY engine exposes. */
export function workerStatusInfo(worker: Worker): WorkerStatusInfo {
  if (worker.status === 'running') return { tone: 'working', label: 'Working' };
  if (worker.status === 'waiting-for-input') return { tone: 'waiting', label: 'Waiting for input' };
  if (worker.status === 'idle') return { tone: 'idle', label: 'Idle' };
  if (worker.last_error) return { tone: 'failed', label: 'Exited with an error' };
  if (worker.exit_code === 0) return { tone: 'stopped', label: 'Session ended (code 0)' };
  if (worker.exit_code === null) return { tone: 'stopped', label: 'Session ended' };
  return { tone: 'failed', label: `Exited (code ${worker.exit_code})` };
}
