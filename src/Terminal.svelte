<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from 'svelte';
  import type { Terminal as Xterm, IDisposable, ITheme } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { SearchAddon, type ISearchOptions } from '@xterm/addon-search';
  import type { WebglAddon } from '@xterm/addon-webgl';
  import { CaseSensitive, ChevronDown, ChevronUp, Regex, X } from 'lucide-svelte';
  import '@xterm/xterm/css/xterm.css';
  import type { TerminalChunk, TerminalSession, TerminalSnapshot } from './types';
  import type { UiPreferences } from './ui/preferences';
  import type { TerminalHandle } from './ui/workspace-layout';
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
    terminal: TerminalSession;
    preferences: UiPreferences;
    active?: boolean;
    onActivate?: (terminalId: string) => void;
    onError?: (message: string) => void;
    onReady?: (handle: TerminalHandle | null, identity: TerminalHandle | undefined, terminalId: string) => void;
  }

  let { terminal, preferences, active = false, onActivate, onError, onReady }: Props = $props();
  const terminalId = untrack(() => terminal.id);
  const terminalName = untrack(() => terminal.name);

  // Orca defaults: Ghostty Default Style Dark / Builtin Tango Light.
  const DARK_TERMINAL_THEME: ITheme = {
    background: '#282c34',
    foreground: '#ffffff',
    cursor: '#ffffff',
    cursorAccent: '#282c34',
    selectionBackground: '#5a7898',
    selectionForeground: '#ffffff',
    black: '#1d1f21',
    red: '#cc6666',
    green: '#b5bd68',
    yellow: '#f0c674',
    blue: '#81a2be',
    magenta: '#b294bb',
    cyan: '#8abeb7',
    white: '#c5c8c6',
    brightBlack: '#666666',
    brightRed: '#d54e53',
    brightGreen: '#b9ca4a',
    brightYellow: '#e7c547',
    brightBlue: '#7aa6da',
    brightMagenta: '#c397d8',
    brightCyan: '#70c0b1',
    brightWhite: '#eaeaea',
  };

  const LIGHT_TERMINAL_THEME: ITheme = {
    background: '#ffffff',
    foreground: '#2e3434',
    cursor: '#2e3434',
    cursorAccent: '#ffffff',
    selectionBackground: '#accef7',
    selectionForeground: '#2e3434',
    black: '#2e3436',
    red: '#cc0000',
    green: '#4e9a06',
    yellow: '#8e7700',
    blue: '#3465a4',
    magenta: '#75507b',
    cyan: '#05727e',
    white: '#6a6a6a',
    brightBlack: '#555753',
    brightRed: '#ef2929',
    brightGreen: '#1b7a1b',
    brightYellow: '#6d5a00',
    brightBlue: '#204a87',
    brightMagenta: '#ad7fa8',
    brightCyan: '#034b50',
    brightWhite: '#3d3d3d',
  };

  const SEARCH_DECORATIONS = {
    matchBackground: '#5c4a00',
    matchBorder: '#5c4a00',
    matchOverviewRuler: '#ffcc00',
    activeMatchBackground: '#c4580e',
    activeMatchBorder: '#ffcf6b',
    activeMatchColorOverviewRuler: '#ff9900',
  };

  function prefersDark(): boolean {
    if (typeof document === 'undefined') return true;
    const root = document.documentElement;
    if (root.classList.contains('dark')) return true;
    if (root.classList.contains('light')) return false;
    return typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)').matches : true;
  }

  function resolveTerminalTheme(prefs: UiPreferences): ITheme {
    const dark = prefs.theme === 'dark' || (prefs.theme === 'system' && prefersDark());
    return dark ? DARK_TERMINAL_THEME : LIGHT_TERMINAL_THEME;
  }

  function searchOptions(incremental: boolean): ISearchOptions {
    return {
      caseSensitive: searchCase,
      regex: searchRegex,
      incremental,
      decorations: SEARCH_DECORATIONS,
    };
  }

  const GAP_NOTICE = '\r\n\x1b[33m[older output discarded]\x1b[0m\r\n';
  const MAX_PENDING_BYTES = 1024 * 1024;

  let host: HTMLDivElement | null = null;
  let surface: HTMLDivElement | null = null;

  let inView = $state(false);
  let pageVisible = $state(
    typeof document === 'undefined' ? true : document.visibilityState === 'visible',
  );
  let lastPid = untrack(() => terminal.pid);
  let connecting = $state(false);
  let failure = $state<string | null>(null);
  let shellBackground = $state<string>('#282c34');

  let searchOpen = $state(false);
  let searchQuery = $state('');
  let searchCase = $state(false);
  let searchRegex = $state(false);
  let searchIndex = $state(-1);
  let searchCount = $state(0);
  let searchInput = $state<HTMLInputElement | null>(null);
  let searchError = $state<string | null>(null);

  let term: Xterm | null = null;
  let fitAddon: FitAddon | null = null;
  let searchAddon: SearchAddon | null = null;
  let searchResultsDisposable: IDisposable | null = null;
  let webglAddon: WebglAddon | null = null;
  let webglUnavailable = false;
  let dataDisposable: IDisposable | null = null;
  let resizeDisposable: IDisposable | null = null;
  let textareaFocusHandler: (() => void) | null = null;
  let textareaPointerHandler: (() => void) | null = null;

  let intersectionObserver: IntersectionObserver | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let themeObserver: MutationObserver | null = null;
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

  function clearSearchDecorations(): void {
    const addon = searchAddon;
    if (!addon) return;
    try {
      addon.clearDecorations();
      addon.findNext('');
    } catch {
      // The addon can be disposed between a state update and this frame.
    }
    searchIndex = -1;
    searchCount = 0;
  }

  function findSearch(direction: 1 | -1): void {
    const addon = searchAddon;
    if (!addon || searchQuery.length === 0) return;
    searchError = null;
    try {
      if (direction === 1) addon.findNext(searchQuery, searchOptions(false));
      else addon.findPrevious(searchQuery, searchOptions(false));
    } catch (error) {
      searchError = errorMessage(error);
    }
  }

  async function openSearch(): Promise<void> {
    searchOpen = true;
    onActivateWorker();
    await tick();
    searchInput?.focus();
    searchInput?.select();
  }

  function closeSearch(): void {
    searchOpen = false;
    searchError = null;
    clearSearchDecorations();
    term?.focus();
  }

  function onSearchKeydown(event: KeyboardEvent): void {
    event.stopPropagation();
    if (event.key === 'Escape') {
      event.preventDefault();
      closeSearch();
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      findSearch(event.shiftKey ? -1 : 1);
    }
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

  async function readClipboardText(): Promise<string | null> {
    try {
      return await navigator.clipboard.readText();
    } catch (error) {
      report(`Terminal ${terminalName}: clipboard read was denied. ${errorMessage(error)}`);
      return null;
    }
  }

  function copySelection(): Promise<boolean> {
    const text = term?.getSelection() ?? '';
    if (!text) return Promise.resolve(false);
    return writeClipboardText(text);
  }

  async function pasteClipboard(): Promise<boolean> {
    const text = await readClipboardText();
    if (text === null || !term) return false;
    term.paste(text);
    term.focus();
    return true;
  }

  function selectAll(): void {
    term?.selectAll();
    term?.focus();
  }

  function clearScreen(): void {
    term?.clear();
    term?.focus();
  }

  function resetTerminal(): void {
    term?.reset();
    term?.focus();
  }

  function focusTerminal(): void {
    term?.focus();
  }

  function sendToPty(data: string): void {
    if (disposed || !attached) return;
    const gen = generation;
    void writeTerminal(terminalId, data).catch((error) => {
      if (disposed || gen !== generation) return;
      report(`Terminal ${terminalName}: input was not delivered. ${errorMessage(error)}`);
    });
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
    void resizeTerminal(terminalId, cols, rows).catch((error) => {
      if (disposed || gen !== generation) return;
      if (sentCols === cols && sentRows === rows) {
        sentCols = previousCols;
        sentRows = previousRows;
      }
      report(`Terminal ${terminalName}: resize was not applied. ${errorMessage(error)}`);
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
      await ackTerminal(terminalId, end);
      if (gen === generation) ackedEnd = Math.max(ackedEnd, end);
    } catch (error) {
      if (disposed || gen !== generation) return;
      ackFailed = true;
      const message = `Terminal ${terminalName}: output acknowledgement failed. The session is stalled until it reconnects. ${errorMessage(error)}`;
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
        `Terminal ${terminalName}: older output was discarded by the output ring. The display was reset so the missing bytes are not hidden.`,
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
          `Terminal ${terminalName}: received an output span with no new bytes (${chunk.start}-${chunk.end}).`,
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
      `Terminal ${terminalName}: the local output queue exceeded its bound and was dropped. Reattaching to resynchronise from the backend buffer.`,
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
      console.info('WebGL terminal renderer unavailable; using the DOM renderer.', error);
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

    const { Terminal: TerminalEngine } = await import('@xterm/xterm');
    if (disposed || gen !== generation || !connected) return;
    const activeTheme = resolveTerminalTheme(preferences);
    shellBackground = activeTheme.background ?? '#282c34';
    const instance = new TerminalEngine({
      fontFamily: preferences.terminalFontFamily,
      fontSize: preferences.terminalFontSize,
      lineHeight: preferences.terminalLineHeight,
      letterSpacing: 0,
      cursorBlink: preferences.cursorBlink,
      cursorStyle: preferences.cursorStyle,
      scrollback: 500,
      smoothScrollDuration: 0,
      allowTransparency: false,
      allowProposedApi: true,
      minimumContrastRatio: 4.5,
      disableStdin: true,
      convertEol: false,
      theme: activeTheme,
    });
    term = instance;
    // Shell shortcuts are handled during window capture, before xterm sees the event.
    instance.attachCustomKeyEventHandler(event => !event.defaultPrevented);

    const fit = new FitAddon();
    fitAddon = fit;
    instance.loadAddon(fit);

    const search = new SearchAddon();
    searchAddon = search;
    instance.loadAddon(search);
    searchResultsDisposable = search.onDidChangeResults(({ resultIndex, resultCount }) => {
      searchIndex = resultIndex;
      searchCount = resultCount;
    });

    instance.open(surface);
    instance.textarea?.setAttribute('aria-label', `Terminal for ${terminalName}`);
    instance.options.disableStdin = true;

    dataDisposable = instance.onData((data) => {
      if (!attached || disposed) return;
      const sendGen = generation;
      void writeTerminal(terminalId, data).catch((error) => {
        if (disposed || sendGen !== generation) return;
        if (inputErrorShown) return;
        inputErrorShown = true;
        report(`Terminal ${terminalName}: input was not delivered. ${errorMessage(error)}`);
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

    const attach = attachTerminal(terminalId, cols, rows, (chunk) => receive(chunk, gen));
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
      if (searchOpen && searchQuery.length > 0) {
        try {
          searchAddon?.findNext(searchQuery, searchOptions(true));
        } catch {
          // A query the terminal parser rejects is reported by the search effect.
        }
      }
      if (active) instance.focus();
    } catch (error) {
      if (disposed || gen !== generation) return;
      connecting = false;
      if (isAttachSuperseded(error)) {
        releaseLease();
        return;
      }
      const message = `Terminal ${terminalName}: ${errorMessage(error)}`;
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
      const message = `Terminal ${terminalName}: ${errorMessage(error)}`;
      failure = message;
      report(message);
    });
  }

  function onActivateWorker(): void {
    onActivate?.(terminalId);
  }

  function releaseLease(): void {
    const current = lease;
    lease = null;
    if (!current) return;
    void detachTerminal(current).catch((error) => {
      console.warn(
        `Terminal ${terminalName}: detach after a failed attach was not confirmed. ${errorMessage(error)}`,
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
    searchResultsDisposable?.dispose();
    searchResultsDisposable = null;
    searchAddon = null;

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
        const message = `Terminal ${terminalName}: detach failed. ${errorMessage(error)}`;
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

  const terminalHandle: TerminalHandle = {
    focus: focusTerminal,
    focusSearch: () => {
      void openSearch();
    },
    closeSearch,
    isSearchOpen: () => searchOpen,
    hasSelection: () => term?.hasSelection() ?? false,
    copySelection,
    pasteClipboard,
    selectAll,
    clear: clearScreen,
    reset: resetTerminal,
    send: sendToPty,
  };

  $effect(() => {
    const isActive = active;
    if (!isActive) return;
    untrack(() => {
      if (attached && term) term.focus();
    });
  });

  $effect(() => {
    const prefs = preferences;
    const theme = resolveTerminalTheme(prefs);
    shellBackground = theme.background ?? '#282c34';
    const instance = term;
    if (!instance) return;
    instance.options.fontFamily = prefs.terminalFontFamily;
    instance.options.fontSize = prefs.terminalFontSize;
    instance.options.lineHeight = prefs.terminalLineHeight;
    instance.options.cursorStyle = prefs.cursorStyle;
    instance.options.cursorBlink = prefs.cursorBlink;
    instance.options.theme = theme;
    scheduleFit();
  });

  $effect(() => {
    // Track the controls; the addon itself is recreated with the terminal.
    const open = searchOpen;
    const query = searchQuery;
    const caseSensitive = searchCase;
    const regex = searchRegex;
    const addon = searchAddon;
    searchError = null;
    if (!open || query.length === 0 || !addon) {
      if (addon) clearSearchDecorations();
      return;
    }
    try {
      addon.findNext(query, {
        caseSensitive,
        regex,
        incremental: true,
        decorations: SEARCH_DECORATIONS,
      });
    } catch (error) {
      searchError = errorMessage(error);
    }
  });

  onMount(() => {
    onReady?.(terminalHandle, undefined, terminalId);
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

    themeObserver = new MutationObserver(() => {
      const theme = resolveTerminalTheme(preferences);
      shellBackground = theme.background ?? '#282c34';
      if (term) term.options.theme = theme;
      scheduleFit();
    });
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    });
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
    const pid = terminal.pid;
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
    onReady?.(null, terminalHandle, terminalId);
    if (visibilityHandler) document.removeEventListener('visibilitychange', visibilityHandler);
    visibilityHandler = null;
    intersectionObserver?.disconnect();
    intersectionObserver = null;
    resizeObserver?.disconnect();
    resizeObserver = null;
    themeObserver?.disconnect();
    themeObserver = null;
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
        console.warn(`Terminal ${terminalName}: detach failed. ${errorMessage(error)}`);
      });
    }
  });
</script>

<div class="terminal-shell" bind:this={host} style:background={shellBackground}>
  {#if searchOpen}
    <div
      data-terminal-search-root
      class="absolute right-2 top-2 z-50 flex items-center gap-1 rounded-lg border border-border bg-popover/95 px-2 py-1 text-popover-foreground shadow-floating backdrop-blur-sm"
      style="width: 340px; max-width: calc(100% - 16px)"
      role="search"
    >
      <input
        bind:this={searchInput}
        type="text"
        bind:value={searchQuery}
        placeholder="Search..."
        aria-label="Search terminal"
        spellcheck="false"
        class="min-w-0 flex-1 border-none bg-transparent text-sm text-popover-foreground outline-none placeholder:text-muted-foreground"
        onkeydown={onSearchKeydown}
      />
      <button
        type="button"
        class="flex size-6 shrink-0 items-center justify-center rounded-sm {searchCase
          ? 'bg-secondary text-foreground'
          : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
        aria-pressed={searchCase}
        aria-label="Case sensitive"
        title="Case sensitive"
        onclick={() => {
          searchCase = !searchCase;
        }}
      >
        <CaseSensitive size={14} />
      </button>
      <button
        type="button"
        class="flex size-6 shrink-0 items-center justify-center rounded-sm {searchRegex
          ? 'bg-secondary text-foreground'
          : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
        aria-pressed={searchRegex}
        aria-label="Regex"
        title="Regex"
        onclick={() => {
          searchRegex = !searchRegex;
        }}
      >
        <Regex size={14} />
      </button>
      <span class="shrink-0 whitespace-nowrap px-1 text-xs tabular-nums text-muted-foreground">
        {searchError ?? (searchQuery.length === 0
          ? '0/0'
          : searchCount === 0
            ? 'No results'
            : searchIndex === -1
              ? `${searchCount}+`
              : `${searchIndex + 1}/${searchCount}`)}
      </span>
      <div class="mx-0.5 h-4 w-px bg-border"></div>
      <button
        type="button"
        class="flex size-6 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        aria-label="Previous match"
        title="Previous match"
        onclick={() => {
          findSearch(-1);
        }}
      >
        <ChevronUp size={14} />
      </button>
      <button
        type="button"
        class="flex size-6 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        aria-label="Next match"
        title="Next match"
        onclick={() => {
          findSearch(1);
        }}
      >
        <ChevronDown size={14} />
      </button>
      <div class="mx-0.5 h-4 w-px bg-border"></div>
      <button
        type="button"
        class="flex size-6 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        aria-label="Close search"
        title="Close"
        onclick={closeSearch}
      >
        <X size={14} />
      </button>
    </div>
  {/if}
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
    display: flex;
    flex: 1 1 0%;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .terminal-surface {
    position: absolute;
    inset: 0;
    box-sizing: content-box;
    padding: 4px;
    overflow: hidden;
  }

  .terminal-surface :global(.xterm) {
    width: 100%;
    height: 100%;
  }

  .terminal-surface :global(.xterm-viewport) {
    /* xterm 6 scrolls in .xterm-scrollable-element; this hosts its overview ruler. */
    background: transparent;
    overflow: hidden;
  }

  .terminal-notice {
    position: absolute;
    left: 10px;
    bottom: 8px;
    margin: 0;
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--popover);
    color: var(--muted-foreground);
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
    background: var(--popover);
  }

  .failure-title {
    margin: 0;
    color: var(--foreground);
    font-size: 13px;
    font-weight: 600;
  }

  .failure-text {
    margin: 0;
    max-width: 62ch;
    color: var(--destructive);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .terminal-retry {
    min-height: 30px;
    padding: 4px 12px;
    border: 1px solid var(--input);
    border-radius: var(--radius-md);
    background: var(--popover);
    color: var(--foreground);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .terminal-retry:hover:not(:disabled) {
    border-color: var(--accent);
  }

  .terminal-retry:disabled {
    color: var(--muted-foreground);
    cursor: not-allowed;
  }
</style>
