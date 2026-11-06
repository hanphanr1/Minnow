import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Project,
  Settings,
  Snapshot,
  TerminalChunk,
  TerminalSnapshot,
  Worker,
} from './types';

export type { UnlistenFn };

export function isDesktop(): boolean {
  return isTauri();
}

export function bootstrap(): Promise<Snapshot> {
  return invoke<Snapshot>('bootstrap');
}

export function addProject(path: string): Promise<Project> {
  return invoke<Project>('add_project', { path });
}

export function createWorkers(projectId: string, count: number, name: string): Promise<Worker[]> {
  return invoke<Worker[]>('create_workers', { projectId, count, name });
}

export function restartWorker(workerId: string): Promise<Worker> {
  return invoke<Worker>('restart_worker', { workerId });
}

export function killWorker(workerId: string): Promise<Worker> {
  return invoke<Worker>('kill_worker', { workerId });
}

export function deleteWorker(workerId: string): Promise<void> {
  return invoke<void>('delete_worker', { workerId });
}

export function diffWorker(workerId: string): Promise<string> {
  return invoke<string>('diff_worker', { workerId });
}

export function mergeWorker(workerId: string): Promise<string> {
  return invoke<string>('merge_worker', { workerId });
}

export function openEditor(workerId: string): Promise<void> {
  return invoke<void>('open_editor', { workerId });
}

export function saveSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>('save_settings', { settings });
}

export function writeTerminal(workerId: string, data: string): Promise<void> {
  return invoke<void>('write_terminal', { workerId, data });
}

export function resizeTerminal(workerId: string, cols: number, rows: number): Promise<void> {
  return invoke<void>('resize_terminal', { workerId, cols, rows });
}

export function ackTerminal(workerId: string, end: number): Promise<void> {
  return invoke<void>('ack_terminal', { workerId, end });
}

export function listenWorkerUpdated(handler: (worker: Worker) => void): Promise<UnlistenFn> {
  return listen<Worker>('worker-updated', (event) => handler(event.payload));
}

export function listenWorkerRemoved(handler: (workerId: string) => void): Promise<UnlistenFn> {
  return listen<string>('worker-removed', (event) => handler(event.payload));
}

/**
 * Subscribes to both worker events. Resolves with one disposer that always
 * removes both listeners, including when the second subscription fails.
 */
export async function subscribeWorkerEvents(
  onUpdated: (worker: Worker) => void,
  onRemoved: (workerId: string) => void,
): Promise<UnlistenFn> {
  const unlistenUpdated = await listenWorkerUpdated(onUpdated);
  try {
    const unlistenRemoved = await listenWorkerRemoved(onRemoved);
    return () => {
      unlistenUpdated();
      unlistenRemoved();
    };
  } catch (error) {
    unlistenUpdated();
    throw error;
  }
}

export function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message: unknown = error.message;
    if (typeof message === 'string') return message;
  }
  try {
    const encoded = JSON.stringify(error);
    if (encoded && encoded !== '{}') return encoded;
  } catch {
    // Fall through to the generic message below.
  }
  return 'Unknown error';
}

export class AttachSupersededError extends Error {
  constructor() {
    super('Terminal attach was superseded by a newer session.');
    this.name = 'AttachSupersededError';
  }
}

export function isAttachSuperseded(error: unknown): boolean {
  return error instanceof AttachSupersededError;
}

export interface TerminalLease {
  readonly workerId: string;
  readonly id: number;
}

export interface TerminalAttach {
  readonly lease: TerminalLease;
  readonly snapshot: Promise<TerminalSnapshot>;
}

let leaseCounter = 0;
const latestLeases = new Map<string, TerminalLease>();
const terminalTails = new Map<string, Promise<void>>();

/**
 * Runs terminal subscription commands for one worker in call order. A detach
 * queued before a newer attach therefore lands before that attach, and a
 * detach queued after it can be recognised as stale before it reaches Rust.
 */
function enqueueTerminal<T>(workerId: string, task: () => Promise<T>): Promise<T> {
  const previous = terminalTails.get(workerId) ?? Promise.resolve();
  const run = previous.then(task, task);
  const settled = run.then(
    () => undefined,
    () => undefined,
  );
  terminalTails.set(workerId, settled);
  void settled.then(() => {
    if (terminalTails.get(workerId) === settled) terminalTails.delete(workerId);
  });
  return run;
}

/**
 * Requests a subscription. The lease is returned synchronously so a teardown
 * that happens while the snapshot is in flight can detach the right session.
 */
export function attachTerminal(
  workerId: string,
  cols: number,
  rows: number,
  onOutput: (chunk: TerminalChunk) => void,
): TerminalAttach {
  const lease: TerminalLease = { workerId, id: ++leaseCounter };
  const previous = latestLeases.get(workerId);
  latestLeases.set(workerId, lease);

  const snapshot = enqueueTerminal(workerId, async () => {
    const latest = latestLeases.get(workerId);
    if (latest === undefined || latest.id !== lease.id) {
      throw new AttachSupersededError();
    }

    // Close the previous subscription before this one so a remount can never
    // race two attach calls, even when the old teardown detach was skipped.
    if (previous !== undefined && previous.id !== lease.id) {
      try {
        await invoke<void>('detach_terminal', { workerId });
      } catch (error) {
        // The previous lease may never have reached an attached state.
        console.info('Previous terminal subscription was already gone.', error);
      }
    }

    const latestAfterDetach = latestLeases.get(workerId);
    if (latestAfterDetach === undefined || latestAfterDetach.id !== lease.id) {
      throw new AttachSupersededError();
    }

    const channel = new Channel<TerminalChunk>();
    channel.onmessage = onOutput;
    return invoke<TerminalSnapshot>('attach_terminal', {
      workerId,
      cols,
      rows,
      onOutput: channel,
    });
  });

  return { lease, snapshot };
}

/**
 * A detach for a lease that a newer attach has already replaced is skipped:
 * sending it later would erase the new subscription. Attach calls are also
 * skipped when a newer lease exists, so a stale pair cannot attach at all.
 */
export function detachTerminal(lease: TerminalLease): Promise<void> {
  const latest = latestLeases.get(lease.workerId);
  if (latest !== undefined && latest.id > lease.id) {
    return Promise.resolve();
  }
  return enqueueTerminal(lease.workerId, () =>
    invoke<void>('detach_terminal', { workerId: lease.workerId }),
  );
}
