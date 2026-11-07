<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { isDesktop } from '../api';

  let { onError, onBeforeClose }: { onError: (message: string) => void; onBeforeClose: () => Promise<boolean> } = $props();
  let maximized = $state(false);

  async function operate(action: 'minimize' | 'maximize' | 'close') {
    if (!isDesktop()) return;
    try {
      const window = getCurrentWindow();
      if (action === 'minimize') await window.minimize();
      else if (action === 'maximize') {
        await window.toggleMaximize();
        maximized = await window.isMaximized();
      } else await window.close();
    } catch (error) {
      onError(String(error));
    }
  }

  onMount(() => {
    if (!isDesktop()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenClose: (() => void) | undefined;
    let closeApproved = false;
    let decidingClose = false;
    const window = getCurrentWindow();
    void window.isMaximized().then(value => { if (!disposed) maximized = value; }).catch(error => onError(String(error)));
    void window.onResized(async () => {
      try { const value = await window.isMaximized(); if (!disposed) maximized = value; }
      catch (error) { if (!disposed) onError(String(error)); }
    }).then(remove => { if (disposed) remove(); else unlisten = remove; }).catch(error => onError(String(error)));
    void window.onCloseRequested(event => {
      if (closeApproved) return;
      event.preventDefault();
      if (decidingClose) return;
      decidingClose = true;
      void onBeforeClose().then(async allowed => {
        if (!allowed || disposed) return;
        closeApproved = true;
        try { await window.close(); }
        catch (error) { closeApproved = false; onError(String(error)); }
      }).catch(error => onError(String(error))).finally(() => { decidingClose = false; });
    }).then(remove => { if (disposed) remove(); else unlistenClose = remove; }).catch(error => onError(String(error)));
    return () => { disposed = true; unlisten?.(); unlistenClose?.(); };
  });
</script>

<!-- Orca WindowControls.tsx, MIT; SVG paths retained for Windows chrome parity. -->
<div class="window-controls">
  <button class="window-controls-btn" aria-label="Minimize" onclick={() => operate('minimize')}>
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10v1H0z" fill="currentColor" /></svg>
  </button>
  <button class="window-controls-btn" aria-label={maximized ? 'Restore' : 'Maximize'} onclick={() => operate('maximize')}>
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <path d={maximized ? 'M2 0v2H0v8h8V8h2V0H2zm6 9H1V3h7v6zM9 7H8V2H3V1h6v6z' : 'M0 0v10h10V0H0zm9 9H1V1h8v8z'} fill="currentColor" />
    </svg>
  </button>
  <button class="window-controls-btn window-controls-close" aria-label="Close" onclick={() => operate('close')}>
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0L0 1l4 4-4 4 1 1 4-4 4 4 1-1-4-4 4-4-1-1-4 4-4-4z" fill="currentColor" /></svg>
  </button>
</div>
