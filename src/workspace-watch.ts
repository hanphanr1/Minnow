import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { announceWorkspaceChange } from './workspace-api';

let tail: Promise<void> = Promise.resolve();
function enqueue(operation: () => Promise<void>): Promise<void> {
  const run = tail.then(operation, operation);
  tail = run.then(() => undefined, () => undefined);
  return run;
}

/** Each visible workspace owns a lease; old teardown cannot stop a newer watch. */
export function watchWorkspace(workerId: string, onError: (error: unknown) => void): () => void {
  const lease = crypto.randomUUID();
  let active = true;
  let unlisten: (() => void) | undefined;
  const started = (async () => {
    const off = await listen<{ worker_id: string }>('workspace-files-changed', event => {
      if (active && event.payload.worker_id === workerId) announceWorkspaceChange(workerId);
    });
    if (!active) { off(); return; }
    unlisten = off;
    await enqueue(() => active
      ? invoke<void>('watch_workspace', { workerId, lease })
      : Promise.resolve());
  })();
  void started.catch(error => { if (active) onError(error); });
  return () => {
    active = false;
    unlisten?.();
    void started.catch(() => undefined).then(() => enqueue(() =>
      invoke<void>('watch_workspace', { workerId: null, lease })
    )).catch(error => console.warn('Could not release workspace file observer.', error));
  };
}
