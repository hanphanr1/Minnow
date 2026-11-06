<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { Terminal as Xterm, type IDisposable } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import type { WebglAddon } from '@xterm/addon-webgl';
  import '@xterm/xterm/css/xterm.css';
  import type { TerminalChunk, TerminalSnapshot, Worker } from './types';
  import {
    ackTerminal,
    attachTerminal,
    detachTerminal,
    errorMessage,
    isAttachSuperseded,
    resizeTerminal,
    writeTerminal,
    type TerminalLease,
  } from './api';

  interface Props {
    worker: Worker;
    onActivate?: (workerId: string) => void;
    onError?: (message: string) => void;
  }

  let { worker, onActivate, onError }: Props = $props();

  const XTERM_THEME = {
    background: '#0b0d0f',
    foreground: '#e8ecef',
    cursor: '#e3a857',
    cursorAccent: '#0b0d0f',
    selectionBackground: '#31404a',
    selectionInactiveBackground: '#222c33',
    black: '#1a1f24',
    red: '#e08d80',
    green: '#86c79a',
    yellow: '#e3a857',
    blue: '#7fa8c9',
    magenta: '#b394c9',
    cyan: '#7fc4c0',
    white: '#cfd6dc',
    brightBlack: '#7c8790',
    brightRed: '#f0a79b',
    brightGreen: '#a4dbb4',
    brightYellow: '#f0c47e',
    brightBlue: '#9dc3e0',
    brightMagenta: '#cdb3dd',
    brightCyan: '#9fd9d5',
    brightWhite: '#f2f5f7',
  };

  const GAP_NOTICE = '\r\n\x1b[33m[older output discarded]\x1b[0m\r\n';
  const MAX_PENDING_BYTES = 1024 * 1024;

  let host: HTMLDivElement | null = null;
  let surface: HTMLDivElement | null = null;

  let inView = $state(false);
  let pageVisible = $state(
    typeof document === 'undefined' ? true : document.visibilityState === 'visible',
  );
  let lastPid = untrack(() => worker.pid);
  let connecting = $state(false);
  let failure = $state<string | null>(null);

  let term: Xterm | null = null;
  let fitAddon: FitAddon | null = null;
  let webglAddon: WebglAddon | null = null;
  let webglUnavailable = false;
  let dataDisposable: IDisposable | null = null;
  let resizeDisposable: IDisposable | null = null;
  let textareaFocusHandler: (() => void) | null = null;
  let textareaPointerHandler: (() => void) | null = null;

  let intersectionObserver: IntersectionObserver | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let visibilityHandler: (() => void) | null = null;
  let fitFrame = 0;

  let generation = 0;
  let disposed = false;
  let connected = false;
  let attached = false;
  let lease: TerminalLease | null = null;
  let cursor = 0;
  let pending: TerminalChunk[] = [];
  let pendingBytes = 0;
  let snapshotWriteDone: ((ok: boolean) => void) | null = null;
  let sentCols = 0;
  let sentRows = 0;
  let inputErrorShown = false;
  let overflowResetting = false;

  let ackWanted = 0;
  let ackedEnd = 0;
  let ackInFlight = false;
  let ackFailed = false;

  let gapReported = false;

  function report(message: string): void {
    onError?.(message);
  }

  function toBytes(data: number[] | Uint8Array): Uint8Array {
    return data instanceof Uint8Array ? data : Uint8Array.from(data);
  }

  function scheduleFit(): void {
    if (disposed || fitFrame !== 0) return;
    fitFrame = requestAnimationFrame(() => {
      fitFrame = 0;
      fitAndSync();
    });
  }

  function fitAndSync(): void {
    if (!term || !fitAddon || !host) return;
    if (host.clientWidth < 8 || host.clientHeight < 8) return;
    try {
      fitAddon.fit();
    } catch {
      return;
    }
    if (attached) sendSize(term.cols, term.rows);
  }

  function sendSize(cols: number, rows: number): void {
    if (!attached || disposed) return;
    if (cols === sentCols && rows === sentRows) return;
    const previousCols = sentCols;
    const previousRows = sentRows;
    sentCols = cols;
    sentRows = rows;
    const gen = generation;
    void resizeTerminal(worker.id, cols, rows).catch((error) => {
      if (disposed || gen !== generation) return;
      if (sentCols === cols && sentRows === rows) {
        sentCols = previousCols;
        sentRows = previousRows;
      }
      report(`Terminal ${worker.name}: resize was not applied. ${errorMessage(error)}`);
    });
  }

  function noteAck(end: number, gen: number): void {
    if (disposed || gen !== generation) return;
    if (end > ackWanted) {
      ackWanted = end;
      ackFailed = false;
    }
    void flushAck(gen);
  }

  async function flushAck(gen: number): Promise<void> {
    if (disposed || gen !== generation || ackInFlight || ackFailed) return;
    const end = ackWanted;
    if (end <= ackedEnd) return;
    ackInFlight = true;
    try {
      await ackTerminal(worker.id, end);
      if (gen === generation) ackedEnd = Math.max(ackedEnd, end);
    } catch (error) {
      if (disposed || gen !== generation) return;
      ackFailed = true;
      const message = `Terminal ${worker.name}: output acknowledgement failed. The session is stalled until it reconnects. ${errorMessage(error)}`;
      if (failure === null) {
        failure = message;
        report(message);
      }
    } finally {
      ackInFlight = false;
      if (!disposed && !ackFailed && ackWanted > ackedEnd) {
        void flushAck(generation);
      }
    }
  }

  function resetForGap(chunk: TerminalChunk, gen: number): void {
    if (!term) return;
    // Queue the reset through xterm's write buffer so it lands after earlier
    // chunks and before the replay, never as a synchronous buffer wipe.
    term.write('\x1bc');
    term.write(GAP_NOTICE);
    cursor = chunk.start;
    const bytes = toBytes(chunk.data);
    const current = term;
    if (bytes.length > 0) {
      current.write(bytes, () => noteAck(chunk.end, gen));
    } else {
      noteAck(chunk.end, gen);
    }
    cursor = chunk.end;
    if (!gapReported) {
      gapReported = true;
      report(
        `Terminal ${worker.name}: older output was discarded by the output ring. The display was reset so the missing bytes are not hidden.`,
      );
    }
  }

  function deliver(chunk: TerminalChunk, gen: number): void {
    if (!term || disposed || gen !== generation) return;
    if (chunk.end <= cursor) {
      noteAck(chunk.end, gen);
      return;
    }
    if (chunk.start > cursor) {
      resetForGap(chunk, gen);
      return;
    }
    const bytes = toBytes(chunk.data);
    const offset = Math.max(0, cursor - chunk.start);
    if (offset >= bytes.length) {
      cursor = Math.max(cursor, chunk.end);
      noteAck(chunk.end, gen);
      if (!gapReported) {
        gapReported = true;
        report(
          `Terminal ${worker.name}: received an output span with no new bytes (${chunk.start}-${chunk.end}).`,
        );
      }
      return;
    }
    const current = term;
    current.write(bytes.subarray(offset), () => noteAck(chunk.end, gen));
    cursor = chunk.end;
  }

  function receive(chunk: TerminalChunk, gen: number): void {
    if (disposed || gen !== generation) return;
    if (!attached) {
      pending.push(chunk);
      pendingBytes += chunk.data.length;
      if (pendingBytes > MAX_PENDING_BYTES) {
        scheduleOverflowReconnect();
      }
      return;
    }
    deliver(chunk, gen);
  }

  function flushPending(gen: number): void {
    const queued = pending;
    pending = [];
    pendingBytes = 0;
    for (const chunk of queued) {
      if (disposed || gen !== generation) return;
      deliver(chunk, gen);
    }
  }

  function scheduleOverflowReconnect(): void {
    if (overflowResetting) return;
    overflowResetting = true;
    pending = [];
    pendingBytes = 0;
    report(
      `Terminal ${worker.name}: the local output queue exceeded its bound and was dropped. Reattaching to resynchronise from the backend buffer.`,
    );
    queueMicrotask(() => {
      overflowResetting = false;
      if (!disposed && inView && pageVisible) {
        shutdown();
        startConnection();
      }
    });
  }

  async function enableWebgl(gen: number): Promise<void> {
    if (webglUnavailable || !term) return;
    try {
      const { WebglAddon: Addon } = await import('@xterm/addon-webgl');
      if (disposed || gen !== generation || !term) return;
      const addon = new Addon();
      addon.onContextLoss(() => {
        webglUnavailable = true;
        addon.dispose();
        if (webglAddon === addon) webglAddon = null;
      });
      term.loadAddon(addon);
      webglAddon = addon;
    } catch (error) {
      webglUnavailable = true;
      console.info('WebGL terminal renderer unavailable; using the canvas renderer.', error);
    }
  }

  function awaitSnapshotWrite(bytes: Uint8Array, gen: number): Promise<boolean> {
    if (bytes.length === 0) {
      return Promise.resolve(!disposed && gen === generation);
    }
    return new Promise<boolean>((resolve) => {
      if (!term) {
        resolve(false);
        return;
      }
      const current = term;
      const finish = (ok: boolean): void => {
        if (snapshotWriteDone === finish) snapshotWriteDone = null;
        resolve(ok);
      };
      snapshotWriteDone = finish;
      current.write(bytes, () => {
        finish(!disposed && gen === generation);
      });
    });
  }

  async function connect(): Promise<void> {
    if (disposed || term || connected || !surface || !host) return;

    const gen = generation;
    failure = null;
    inputErrorShown = false;
    gapReported = false;
    cursor = 0;
    pending = [];
    pendingBytes = 0;
    ackWanted = 0;
    ackedEnd = 0;
    ackFailed = false;
    sentCols = 0;
    sentRows = 0;

    connecting = true;
    connected = true;

    const instance = new Xterm({
      fontFamily: '"Cascadia Mono", Consolas, "Courier New", monospace',
      fontSize: 13,
      lineHeight: 1.2,
      letterSpacing: 0,
      cursorBlink: true,
      cursorStyle: 'bar',
      scrollback: 500,
      smoothScrollDuration: 0,
      allowTransparency: false,
      minimumContrastRatio: 4.5,
      disableStdin: true,
      convertEol: false,
      theme: XTERM_THEME,
    });
    term = instance;

    const fit = new FitAddon();
    fitAddon = fit;
    instance.loadAddon(fit);
    instance.open(surface);
    instance.textarea?.setAttribute('aria-label', `Terminal for ${worker.name}`);
    instance.options.disableStdin = true;

    dataDisposable = instance.onData((data) => {
      if (!attached || disposed) return;
      const sendGen = generation;
      void writeTerminal(worker.id, data).catch((error) => {
        if (disposed || sendGen !== generation) return;
        if (inputErrorShown) return;
        inputErrorShown = true;
        report(`Terminal ${worker.name}: input was not delivered. ${errorMessage(error)}`);
      });
    });

    resizeDisposable = instance.onResize(({ cols, rows }) => {
      sendSize(cols, rows);
    });

    const textarea = instance.textarea;
    if (textarea) {
      const focusHandler = () => onActivateWorker();
      textareaFocusHandler = focusHandler;
      textarea.addEventListener('focus', focusHandler);
    }
    const surfacePointerHandler = () => instance.focus();
    textareaPointerHandler = surfacePointerHandler;
    surface.addEventListener('pointerdown', surfacePointerHandler);

    void enableWebgl(gen);
    scheduleFit();

    const cols = instance.cols;
    const rows = instance.rows;
    sentCols = cols;
    sentRows = rows;

    const attach = attachTerminal(worker.id, cols, rows, (chunk) => receive(chunk, gen));
    lease = attach.lease;

    try {
      const snapshot: TerminalSnapshot = await attach.snapshot;
      if (disposed || gen !== generation) return;

      cursor = snapshot.end;
      const bytes = toBytes(snapshot.data);
      const written = await awaitSnapshotWrite(bytes, gen);
      if (!written || disposed || gen !== generation) return;

      ackedEnd = Math.max(ackedEnd, snapshot.end);
      ackWanted = Math.max(ackWanted, snapshot.end);
      attached = true;
      connecting = false;
      instance.options.disableStdin = false;
      flushPending(gen);
      fitAndSync();
    } catch (error) {
      if (disposed || gen !== generation) return;
      connecting = false;
      if (isAttachSuperseded(error)) {
        releaseLease();
        return;
      }
      const message = `Terminal ${worker.name}: ${errorMessage(error)}`;
      failure = message;
      report(message);
      instance.options.disableStdin = true;
      pending = [];
      pendingBytes = 0;
      releaseLease();
    }
  }

  function startConnection(): void {
    void connect().catch((error) => {
      if (disposed) return;
      connecting = false;
      attached = false;
      const message = `Terminal ${worker.name}: ${errorMessage(error)}`;
      failure = message;
      report(message);
    });
  }

  function onActivateWorker(): void {
    onActivate?.(worker.id);
  }

  function releaseLease(): void {
    const current = lease;
    lease = null;
    if (!current) return;
    void detachTerminal(current).catch((error) => {
      console.warn(
        `Terminal ${worker.name}: detach after a failed attach was not confirmed. ${errorMessage(error)}`,
      );
    });
  }

  function destroyTerminal(): void {
    if (fitFrame !== 0) {
      cancelAnimationFrame(fitFrame);
      fitFrame = 0;
    }
    dataDisposable?.dispose();
    dataDisposable = null;
    resizeDisposable?.dispose();
    resizeDisposable = null;

    const textarea = term?.textarea;
    if (textarea && textareaFocusHandler) {
      textarea.removeEventListener('focus', textareaFocusHandler);
    }
    textareaFocusHandler = null;
    if (surface && textareaPointerHandler) {
      surface.removeEventListener('pointerdown', textareaPointerHandler);
    }
    textareaPointerHandler = null;

    if (webglAddon) {
      try {
        webglAddon.dispose();
      } catch {
        // The renderer may already be gone after a context loss.
      }
      webglAddon = null;
    }
    if (term) {
      term.dispose();
      term = null;
    }
    fitAddon = null;
    surface?.replaceChildren();
  }

  function shutdown(): void {
    generation += 1;
    snapshotWriteDone?.(false);
    snapshotWriteDone = null;
    const current = lease;
    lease = null;
    connected = false;
    attached = false;
    connecting = false;
    cursor = 0;
    pending = [];
    pendingBytes = 0;
    sentCols = 0;
    sentRows = 0;
    ackWanted = 0;
    ackedEnd = 0;
    ackFailed = false;
    destroyTerminal();
    if (current) {
      void detachTerminal(current).catch((error) => {
        const message = `Terminal ${worker.name}: detach failed. ${errorMessage(error)}`;
        if (disposed) console.warn(message);
        else report(message);
      });
    }
  }

  function retryAttach(): void {
    if (disposed || !inView || !pageVisible) return;
    onActivateWorker();
    shutdown();
    startConnection();
  }

  onMount(() => {
    pageVisible = document.visibilityState === 'visible';
    const onVisibility = () => {
      pageVisible = document.visibilityState === 'visible';
    };
    visibilityHandler = onVisibility;
    document.addEventListener('visibilitychange', onVisibility);

    if (typeof IntersectionObserver === 'undefined') {
      inView = true;
    } else if (host) {
      intersectionObserver = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            if (entry.target === host) inView = entry.isIntersecting;
          }
        },
        { threshold: 0 },
      );
      intersectionObserver.observe(host);
    }

    if (typeof ResizeObserver !== 'undefined' && host) {
      resizeObserver = new ResizeObserver(() => scheduleFit());
      resizeObserver.observe(host);
    }
  });

  $effect(() => {
    const shouldRun = inView && pageVisible;
    untrack(() => {
      if (shouldRun) {
        startConnection();
      } else if (connected) {
        shutdown();
      }
    });
  });

  $effect(() => {
    const pid = worker.pid;
    if (pid === lastPid) return;
    lastPid = pid;
    if (pid === null) return;
    untrack(() => {
      if (!disposed && inView && pageVisible) {
        shutdown();
        startConnection();
      }
    });
  });

  onDestroy(() => {
    disposed = true;
    generation += 1;
    if (visibilityHandler) document.removeEventListener('visibilitychange', visibilityHandler);
    visibilityHandler = null;
    intersectionObserver?.disconnect();
    intersectionObserver = null;
    resizeObserver?.disconnect();
    resizeObserver = null;
    snapshotWriteDone?.(false);
    snapshotWriteDone = null;
    const current = lease;
    lease = null;
    connected = false;
    attached = false;
    connecting = false;
    destroyTerminal();
    if (current) {
      void detachTerminal(current).catch((error) => {
        console.warn(`Terminal ${worker.name}: detach failed. ${errorMessage(error)}`);
      });
    }
  });
</script>

<div class="terminal-shell" bind:this={host}>
  <div class="terminal-surface" bind:this={surface}></div>
  {#if connecting}
    <p class="terminal-notice" role="status">Attaching terminal…</p>
  {/if}
  {#if failure}
    <div class="terminal-failure" role="alert">
      <p class="failure-title">Terminal session problem</p>
      <p class="failure-text">{failure}</p>
      <button
        class="terminal-retry"
        type="button"
        onclick={retryAttach}
        disabled={!inView || !pageVisible}
      >
        Retry attach
      </button>
    </div>
  {/if}
</div>

<style>
  .terminal-shell {
    position: relative;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--terminal-bg);
  }

  .terminal-surface {
    position: absolute;
    inset: 0;
    padding: 6px 4px 4px 10px;
  }

  .terminal-surface :global(.xterm) {
    height: 100%;
  }

  .terminal-surface :global(.xterm-viewport) {
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .terminal-notice {
    position: absolute;
    left: 10px;
    bottom: 8px;
    margin: 0;
    padding: 3px 8px;
    border: 1px solid var(--line);
    border-radius: 3px;
    background: var(--panel-raised);
    color: var(--text-muted);
    font-size: 11px;
    pointer-events: none;
  }

  .terminal-failure {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: 8px;
    padding: 18px;
    overflow: auto;
    background: color-mix(in srgb, var(--terminal-bg) 92%, transparent);
  }

  .failure-title {
    margin: 0;
    color: var(--text);
    font-size: 13px;
    font-weight: 600;
  }

  .failure-text {
    margin: 0;
    max-width: 62ch;
    color: var(--danger);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .terminal-retry {
    min-height: 30px;
    padding: 4px 12px;
    border: 1px solid var(--line-strong);
    border-radius: 3px;
    background: var(--panel-raised);
    color: var(--text);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .terminal-retry:hover:not(:disabled) {
    border-color: var(--accent);
  }

  .terminal-retry:disabled {
    color: var(--text-faint);
    cursor: not-allowed;
  }
</style>
