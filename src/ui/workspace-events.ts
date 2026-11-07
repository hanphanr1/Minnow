/**
 * Reads the worker id carried by `minnow-workspace-changed` without asserting a
 * CustomEvent shape onto every Event that reaches the listener.
 */
export function workspaceEventDetail(event: Event): unknown {
  return 'detail' in event ? event.detail : undefined;
}
