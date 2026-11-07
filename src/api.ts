import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Project,
  Settings,
  Snapshot,
  TerminalChunk,
  TerminalKind,
  TerminalSession,
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

export function createTerminal(
  workerId: string,
  sourceTerminalId: string | null,
  kind: TerminalKind,
): Promise<TerminalSession> {
  return invoke<TerminalSession>('create_terminal', { workerId, sourceTerminalId, kind });
}

export function restartTerminal(terminalId: string): Promise<TerminalSession> {
  return invoke<TerminalSession>('restart_terminal', { terminalId });
}

/** Stops the process but keeps the terminal record and its visible output. */
export function killTerminal(terminalId: string): Promise<TerminalSession> {
  return invoke<TerminalSession>('kill_terminal', { terminalId });
}

/** Stops the process, marks the terminal closed and keeps its identity for reopen. */
export function closeTerminal(terminalId: string): Promise<TerminalSession> {
  return invoke<TerminalSession>('close_terminal', { terminalId });
}

export function terminalCwd(terminalId: string): Promise<string> {
  return invoke<string>('terminal_cwd', { terminalId });
}

export function writeTerminal(terminalId: string, data: string): Promise<void> {
  return invoke<void>('write_terminal', { terminalId, data });
}

export function resizeTerminal(terminalId: string, cols: number, rows: number): Promise<void> {
  return invoke<void>('resize_terminal', { terminalId, cols, rows });
}

export function ackTerminal(terminalId: string, end: number): Promise<void> {
  return invoke<void>('ack_terminal', { terminalId, end });
}

export function listenWorkerUpdated(handler: (worker: Worker) => void): Promise<UnlistenFn> {
  return listen<Worker>('worker-updated', (event) => handler(event.payload));
}

export function listenWorkerRemoved(handler: (workerId: string) => void): Promise<UnlistenFn> {
  return listen<string>('worker-removed', (event) => handler(event.payload));
}

export function listenTerminalUpdated(
  handler: (terminal: TerminalSession) => void,
): Promise<UnlistenFn> {
  return listen<TerminalSession>('terminal-updated', (event) => handler(event.payload));
}

export function listenTerminalRemoved(handler: (terminalId: string) => void): Promise<UnlistenFn> {
  return listen<string>('terminal-removed', (event) => handler(event.payload));
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

/**
 * Subscribes to terminal update and removal events. Same disposable contract as
 * subscribeWorkerEvents so the root can queue terminal events before bootstrap.
 */
export async function subscribeTerminalEvents(
  onUpdated: (terminal: TerminalSession) => void,
  onRemoved: (terminalId: string) => void,
): Promise<UnlistenFn> {
  const unlistenUpdated = await listenTerminalUpdated(onUpdated);
  try {
    const unlistenRemoved = await listenTerminalRemoved(onRemoved);
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
  readonly terminalId: string;
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
 * Runs terminal subscription commands for one terminal in call order. A detach
 * queued before a newer attach therefore lands before that attach, and a detach
 * queued after it can be recognised as stale before it reaches Rust.
 */
function enqueueTerminal<T>(terminalId: string, task: () => Promise<T>): Promise<T> {
  const previous = terminalTails.get(terminalId) ?? Promise.resolve();
  const run = previous.then(task, task);
  const settled = run.then(
    () => undefined,
    () => undefined,
  );
  terminalTails.set(terminalId, settled);
  void settled.then(() => {
    if (terminalTails.get(terminalId) === settled) terminalTails.delete(terminalId);
  });
  return run;
}

/**
 * Requests a subscription. The lease is returned synchronously so a teardown
 * that happens while the snapshot is in flight can detach the right session.
 */
export function attachTerminal(
  terminalId: string,
  cols: number,
  rows: number,
  onOutput: (chunk: TerminalChunk) => void,
): TerminalAttach {
  const lease: TerminalLease = { terminalId, id: ++leaseCounter };
  const previous = latestLeases.get(terminalId);
  latestLeases.set(terminalId, lease);

  const snapshot = enqueueTerminal(terminalId, async () => {
    const latest = latestLeases.get(terminalId);
    if (latest === undefined || latest.id !== lease.id) {
      throw new AttachSupersededError();
    }

    // Close the previous subscription before this one so a remount can never
    // race two attach calls, even when the old teardown detach was skipped.
    if (previous !== undefined && previous.id !== lease.id) {
      try {
        await invoke<void>('detach_terminal', { terminalId });
      } catch (error) {
        // The previous lease may never have reached an attached state.
        console.info('Previous terminal subscription was already gone.', error);
      }
    }

    const latestAfterDetach = latestLeases.get(terminalId);
    if (latestAfterDetach === undefined || latestAfterDetach.id !== lease.id) {
      throw new AttachSupersededError();
    }

    const channel = new Channel<TerminalChunk>();
    channel.onmessage = onOutput;
    return invoke<TerminalSnapshot>('attach_terminal', {
      terminalId,
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
  const latest = latestLeases.get(lease.terminalId);
  if (latest !== undefined && latest.id > lease.id) {
    return Promise.resolve();
  }
  return enqueueTerminal(lease.terminalId, () =>
    invoke<void>('detach_terminal', { terminalId: lease.terminalId }),
  );
}
