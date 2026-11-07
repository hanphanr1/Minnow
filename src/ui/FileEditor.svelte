<script lang="ts">
  import { tick, untrack } from 'svelte';
  import {
    CircleAlert,
    FileWarning,
    Loader2,
    RotateCw,
    Save,
    X
  } from 'lucide-svelte';
  import { EditorView, keymap, lineNumbers, drawSelection, dropCursor, highlightActiveLine, highlightActiveLineGutter, highlightSpecialChars } from '@codemirror/view';
  import { Compartment, EditorState, type Extension } from '@codemirror/state';
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
  import { highlightSelectionMatches, search, searchKeymap } from '@codemirror/search';
  import { bracketMatching, foldGutter, foldKeymap, HighlightStyle, indentOnInput, indentUnit, syntaxHighlighting } from '@codemirror/language';
  import { tags } from '@lezer/highlight';
  import type { Worker } from '../types';
  import {
    announceWorkspaceChange,
    readWorkspaceFile,
    writeWorkspaceFile
  } from '../workspace-api';
  import { baseName, dirName } from './file-icons';
  import { errorMessage } from '../api';

  let {
    worker,
    path,
    line,
    rightSidebarVisible = true,
    workerAvailable = true,
    onClose
  }: {
    worker: Worker;
    path: string;
    line?: number;
    rightSidebarVisible?: boolean;
    workerAvailable?: boolean;
    onClose: () => void;
  } = $props();

  const workerId = $derived(worker.id);

  let container = $state<HTMLDivElement | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let saveError = $state<string | null>(null);
  let languageError = $state<string|null>(null);
  let saving = $state(false);
  let reloading = $state(false);
  let dirty = $state(false);
  let ready = $state(false);
  let currentLine = $state(1);
  let currentColumn = $state(1);

  let closeDialogEl = $state<HTMLDialogElement | null>(null);
  let reloadDialogEl = $state<HTMLDialogElement | null>(null);
  let confirmSaving = $state(false);
  let confirmError = $state<string | null>(null);

  let view: EditorView | null = null;
  let savedContent = '';
  let revision = '';
  let loadSeq = 0;
  let saveSeq = 0;
  let pendingClose: Promise<boolean> | null = null;
  let resolveClosePending: ((value: boolean) => void) | null = null;

  const languageCompartment = new Compartment();

  const languageLabel = $derived(languageLabelFor(path));

  const editorTheme = EditorView.theme({
    '&': {
      height: '100%',
      backgroundColor: 'var(--editor-surface)',
      color: 'var(--foreground)',
      fontSize: '13px'
    },
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': {
      fontFamily:
        'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
      lineHeight: '1.55',
      overflow: 'auto'
    },
    '.cm-content': {
      padding: '6px 0',
      caretColor: 'var(--foreground)'
    },
    '.cm-gutters': {
      backgroundColor: 'var(--editor-surface)',
      color: 'var(--muted-foreground)',
      border: 'none',
      borderRight:
        '1px solid color-mix(in srgb, var(--border) 72%, transparent)'
    },
    '.cm-lineNumbers .cm-gutterElement': { padding: '0 8px 0 10px' },
    '.cm-activeLineGutter': {
      backgroundColor: 'color-mix(in srgb, var(--accent) 60%, transparent)',
      color: 'var(--foreground)'
    },
    '.cm-activeLine': {
      backgroundColor: 'color-mix(in srgb, var(--accent) 34%, transparent)'
    },
    '.cm-selectionBackground, ::selection': {
      backgroundColor: 'color-mix(in srgb, var(--primary) 28%, transparent)'
    },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--foreground)' },
    '.cm-matchingBracket': {
      backgroundColor: 'color-mix(in srgb, var(--annotation-highlight) 26%, transparent)',
      outline: '1px solid var(--border)'
    },
    '.cm-nonmatchingBracket': { color: 'var(--git-decoration-deleted)' },
    '.cm-searchMatch': {
      backgroundColor: 'color-mix(in srgb, var(--annotation-highlight) 32%, transparent)'
    },
    '.cm-searchMatch.cm-searchMatch-selected': {
      backgroundColor: 'color-mix(in srgb, var(--annotation-highlight) 55%, transparent)'
    },
    '.cm-selectionMatch': {
      backgroundColor: 'color-mix(in srgb, var(--annotation-highlight) 18%, transparent)'
    },
    '.cm-panels': {
      backgroundColor: 'var(--editor-surface)',
      color: 'var(--foreground)'
    },
    '.cm-panels.cm-panels-top': {
      borderBottom: '1px solid var(--border)'
    },
    '.cm-panels.cm-panels-bottom': {
      borderTop: '1px solid var(--border)'
    },
    '.cm-panel input, .cm-panel button': {
      backgroundColor: 'var(--background)',
      border: '1px solid var(--border)',
      color: 'var(--foreground)',
      borderRadius: '4px',
      padding: '1px 4px'
    },
    '.cm-foldPlaceholder': {
      backgroundColor: 'var(--accent)',
      border: '1px solid var(--border)',
      color: 'var(--muted-foreground)'
    },
    '.cm-tooltip': {
      backgroundColor: 'var(--popover)',
      border: '1px solid var(--border)',
      color: 'var(--foreground)'
    }
  });

  // Colours come from the shared Orca theme tokens so light and dark both stay legible.
  const editorHighlightStyle = HighlightStyle.define([
    { tag: tags.comment, color: 'var(--muted-foreground)', fontStyle: 'italic' },
    {
      tag: [tags.keyword, tags.modifier, tags.operatorKeyword, tags.self],
      color: 'var(--git-decoration-renamed)'
    },
    {
      tag: [tags.string, tags.special(tags.string), tags.regexp],
      color: 'var(--git-decoration-added)'
    },
    {
      tag: [tags.number, tags.bool, tags.null, tags.atom],
      color: 'var(--git-decoration-untracked)'
    },
    {
      tag: [
        tags.function(tags.variableName),
        tags.function(tags.propertyName),
        tags.labelName
      ],
      color: 'var(--git-decoration-modified)'
    },
    {
      tag: [tags.typeName, tags.className, tags.namespace, tags.tagName],
      color: 'var(--git-decoration-renamed)'
    },
    {
      tag: [tags.propertyName, tags.attributeName, tags.definition(tags.variableName)],
      color: 'var(--foreground)'
    },
    {
      tag: [tags.heading, tags.strong],
      color: 'var(--foreground)',
      fontWeight: '600'
    },
    { tag: tags.emphasis, fontStyle: 'italic' },
    {
      tag: [tags.link, tags.url],
      color: 'var(--git-decoration-copied)',
      textDecoration: 'underline'
    },
    {
      tag: [tags.meta, tags.processingInstruction, tags.punctuation],
      color: 'var(--muted-foreground)'
    },
    { tag: tags.invalid, color: 'var(--git-decoration-deleted)' }
  ]);

  function extensionOf(filePath: string): string {
    const name = baseName(filePath);
    const dot = name.lastIndexOf('.');
    return dot < 0 ? '' : name.slice(dot + 1).toLowerCase();
  }

  function languageLabelFor(filePath: string): string {
    switch (extensionOf(filePath)) {
      case 'ts':
        return 'TypeScript';
      case 'tsx':
        return 'TypeScript JSX';
      case 'js':
      case 'mjs':
      case 'cjs':
      case 'mts':
      case 'cts':
        return 'JavaScript';
      case 'jsx':
        return 'JavaScript JSX';
      case 'json':
      case 'jsonc':
      case 'json5':
        return 'JSON';
      case 'rs':
        return 'Rust';
      case 'css':
        return 'CSS';
      case 'html':
        return 'HTML';
      case 'md':
      case 'mdx':
        return 'Markdown';
      default:
        return 'Plain Text';
    }
  }

  function createState(content: string): EditorState {
    return EditorState.create({
      doc: content,
      extensions: [
        EditorState.lineSeparator.of(content.match(/\r\n|\r|\n/)?.[0] ?? '\n'),
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightSpecialChars(),
        history(),
        drawSelection(),
        dropCursor(),
        EditorState.allowMultipleSelections.of(true),
        EditorState.tabSize.of(2),
        indentUnit.of('  '),
        indentOnInput(),
        bracketMatching(),
        foldGutter(),
        highlightActiveLine(),
        highlightSelectionMatches(),
        search({ top: true }),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({
          'aria-label': `Editing ${baseName(path)}`
        }),
        keymap.of([
          {
            key: 'Mod-s',
            preventDefault: true,
            run: () => {
              void saveContent();
              return true;
            }
          },
          ...defaultKeymap,
          ...searchKeymap,
          ...historyKeymap,
          ...foldKeymap,
          indentWithTab
        ]),
        languageCompartment.of([]),
        editorTheme,
        syntaxHighlighting(editorHighlightStyle),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            dirty = update.state.sliceDoc() !== savedContent;
          }
          if (update.docChanged || update.selectionSet) {
            syncCursor(update.state);
          }
        })
      ]
    });
  }

  function syncCursor(state: EditorState): void {
    const head = state.selection.main.head;
    const lineInfo = state.doc.lineAt(head);
    currentLine = lineInfo.number;
    currentColumn = head - lineInfo.from + 1;
  }

  function scrollToLine(target?: number): void {
    if (!view || !target || target < 1) return;
    const lineNumber = Math.min(
      Math.max(1, Math.floor(target)),
      view.state.doc.lines
    );
    const lineInfo = view.state.doc.line(lineNumber);
    view.dispatch({
      selection: { anchor: lineInfo.from },
      effects: EditorView.scrollIntoView(lineInfo.from, { y: 'center' })
    });
    view.focus();
  }

  function disposeView(): void {
    view?.destroy();
    view = null;
  }

  function destroyView(): void {
    ready = false;
    disposeView();
    currentLine = 1;
    currentColumn = 1;
  }

  async function loadLanguage(filePath: string, sequence: number): Promise<void> {
    const extension = extensionOf(filePath);
    let support: Extension | null = null;
    try {
      if (['ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'mts', 'cts'].includes(extension)) {
        const module = await import('@codemirror/lang-javascript');
        support = module.javascript({
          typescript: ['ts', 'tsx', 'mts', 'cts'].includes(extension),
          jsx: ['jsx', 'tsx'].includes(extension)
        });
      } else if (['json', 'jsonc', 'json5'].includes(extension)) {
        const module = await import('@codemirror/lang-json');
        support = module.json();
      } else if (extension === 'rs') {
        const module = await import('@codemirror/lang-rust');
        support = module.rust();
      }
    } catch (error) {
      if (sequence === loadSeq) languageError = `Syntax highlighting could not load: ${errorMessage(error)}. File editing remains available.`;
      return;
    }
    if (!support || sequence !== loadSeq || !view) return;
    view.dispatch({ effects: languageCompartment.reconfigure(support) });
  }

  async function loadFile(
    requestedWorkerId: string,
    filePath: string,
    sequence: number
  ): Promise<void> {
    loading = true;
    loadError = null;
    saveError = null;
    saving = false;
    reloading = false;
    dirty = false;
    destroyView();
    try {
      const document = await readWorkspaceFile(requestedWorkerId, filePath);
      if (sequence !== loadSeq || requestedWorkerId !== workerId) return;
      savedContent = document.content;
      revision = document.revision;
      loading = false;
      await tick();
      if (sequence !== loadSeq || requestedWorkerId !== workerId) return;
      if (!container) {
        loadError = 'The editor surface is unavailable.';
        return;
      }
      view = new EditorView({
        state: createState(document.content),
        parent: container
      });
      savedContent = view.state.sliceDoc();
      ready = true;
      currentLine = 1;
      currentColumn = 1;
      void loadLanguage(filePath, sequence);
      scrollToLine(line);
      view.focus();
    } catch (error) {
      if (sequence !== loadSeq || requestedWorkerId !== workerId) return;
      loadError = errorMessage(error);
      loading = false;
    }
  }

  async function saveContent(): Promise<boolean> {
    if (!view || saving || reloading) return false;
    if (!workerAvailable) { saveError = 'The workspace was removed. Copy this document before closing it.'; return false; }
    const content = view.state.sliceDoc();
    if (content === savedContent) {
      dirty = false;
      return true;
    }
    const requestWorkerId = workerId;
    const filePath = path;
    const expectedRevision = revision;
    const sequence = ++saveSeq;
    saving = true;
    saveError = null;
    try {
      const document = await writeWorkspaceFile(
        requestWorkerId,
        filePath,
        content,
        expectedRevision
      );
      if (sequence !== saveSeq || requestWorkerId !== workerId) return false;
      revision = document.revision;
      savedContent = content;
      dirty = view ? view.state.sliceDoc() !== content : false;
      saveError = null;
      announceWorkspaceChange(requestWorkerId);
      if (pendingClose && !dirty) settleClose(true);
      return true;
    } catch (error) {
      if (sequence === saveSeq && requestWorkerId === workerId) {
        // Keep the dirty document authoritative; the backend refused to overwrite
        // a file whose revision moved on disk.
        const message = `Save failed: ${errorMessage(error)}`;
        saveError = message;
        if (pendingClose) confirmError = message;
      }
      return false;
    } finally {
      if (sequence === saveSeq) saving = false;
    }
  }

  async function reloadNow(): Promise<void> {
    if (loading || reloading || saving) return;
    const requestWorkerId = workerId;
    const filePath = path;
    const sequence = ++loadSeq;
    const contentAtStart = view ? view.state.sliceDoc() : null;
    reloading = true;
    saveError = null;
    try {
      const document = await readWorkspaceFile(requestWorkerId, filePath);
      if (sequence !== loadSeq || requestWorkerId !== workerId) return;
      if (
        view &&
        contentAtStart !== null &&
        view.state.sliceDoc() !== contentAtStart
      ) {
        saveError =
          'Reload skipped: the document changed while reading. Reload again to discard those edits.';
        return;
      }
      savedContent = document.content;
      revision = document.revision;
      dirty = false;
      loadError = null;
      loading = false;
      if (view) {
        const previousAnchor = view.state.selection.main.head;
        view.setState(createState(document.content));
        savedContent = view.state.sliceDoc();
        view.dispatch({ selection: { anchor: Math.min(previousAnchor, view.state.doc.length) } });
        view.focus();
      } else if (container) {
        await tick();
        if (sequence !== loadSeq) return;
        if (!container) {
          loadError = 'The editor surface is unavailable.';
          return;
        }
        view = new EditorView({
          state: createState(document.content),
          parent: container
        });
        ready = true;
        scrollToLine(line);
      }
      if (view) void loadLanguage(filePath, sequence);
      saveError = null;
      if (pendingClose && !dirty) settleClose(true);
    } catch (error) {
      if (sequence === loadSeq) {
        saveError = `Reload failed: ${errorMessage(error)}`;
      }
    } finally {
      if (sequence === loadSeq) reloading = false;
    }
  }

  function requestReload(): void {
    if (saving || reloading) return;
    if (!dirty) {
      void reloadNow();
      return;
    }
    void tick().then(() => {
      if (reloadDialogEl && !reloadDialogEl.open) reloadDialogEl.showModal();
    });
  }

  function cancelReload(): void {
    if (reloadDialogEl?.open) reloadDialogEl.close();
  }

  function discardAndReload(): void {
    if (reloadDialogEl?.open) reloadDialogEl.close();
    void reloadNow();
  }

  function settleClose(value: boolean): void {
    const resolve = resolveClosePending;
    resolveClosePending = null;
    pendingClose = null;
    confirmSaving = false;
    confirmError = null;
    if (closeDialogEl?.open) closeDialogEl.close();
    resolve?.(value);
  }

  /**
   * Parent navigation guard. Resolves true when there is nothing to lose or the
   * user explicitly chose to discard; false when they cancel. Concurrent calls
   * share one dialog and one promise.
   */
  export function requestClose(): Promise<boolean> {
    if (!dirty) return Promise.resolve(true);
    if (pendingClose) return pendingClose;
    if (reloadDialogEl?.open) reloadDialogEl.close();
    const promise = new Promise<boolean>((resolve) => {
      resolveClosePending = resolve;
    });
    pendingClose = promise;
    confirmError = null;
    void tick().then(() => {
      if (!closeDialogEl) {
        settleClose(false);
        return;
      }
      if (!closeDialogEl.open) closeDialogEl.showModal();
    });
    return promise;
  }

  async function handleCloseClick(): Promise<void> {
    if (await requestClose()) onClose();
  }

  async function saveAndClose(): Promise<void> {
    if (confirmSaving || saving || reloading) return;
    if (!dirty) {
      settleClose(true);
      return;
    }
    confirmSaving = true;
    confirmError = null;
    const saved = await saveContent();
    confirmSaving = false;
    if (saved && !dirty) settleClose(true);
    else if (saved) confirmError = 'There are still unsaved changes.';
    else confirmError = saveError ?? 'The file could not be saved.';
  }

  function handleCloseBackdrop(event: MouseEvent): void {
    if (event.target !== closeDialogEl) return;
    settleClose(false);
  }

  function handleReloadBackdrop(event: MouseEvent): void {
    if (event.target !== reloadDialogEl) return;
    cancelReload();
  }

  const documentKey = $derived(`${workerId}:${path}`);
  $effect(() => {
    documentKey;
    const sequence = ++loadSeq;
    untrack(() => { void loadFile(workerId, path, sequence); });
    return () => {
      loadSeq += 1;
      saveSeq += 1;
      disposeView();
    };
  });

  $effect(() => {
    if (!ready) return;
    const target = line;
    if (!target) return;
    scrollToLine(target);
  });

  $effect(() => {
    if (!dirty) return;
    const handleBeforeUnload = (event: BeforeUnloadEvent): void => {
      event.preventDefault();
      event.returnValue = '';
    };
    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => window.removeEventListener('beforeunload', handleBeforeUnload);
  });

  $effect(() => {
    return () => {
      if (resolveClosePending) {
        resolveClosePending(false);
        resolveClosePending = null;
        pendingClose = null;
      }
    };
  });
</script>

<div class="flex h-full min-h-0 w-full flex-col overflow-hidden bg-editor-surface">
  <div
    class="flex h-9 min-h-9 shrink-0 items-center gap-1 border-b border-border/70 bg-editor-surface pl-2"
    style:padding-right={rightSidebarVisible
      ? '0px'
      : 'var(--window-controls-width,138px)'}
    data-tauri-drag-region={rightSidebarVisible ? undefined : 'true'}
  >
    <div
      class="flex min-w-0 flex-1 items-center gap-1.5 overflow-hidden"
      title={path}
    >
      {#if dirty}
        <span
          class="size-1.5 shrink-0 rounded-full bg-foreground"
          role="img"
          aria-label="Unsaved changes"
          title="Unsaved changes"
        ></span>
      {/if}
      <span class="min-w-0 truncate font-mono text-xs text-muted-foreground">
        {dirName(path)}
      </span>
      <span class="shrink-0 font-mono text-xs text-foreground">{baseName(path)}</span>
    </div>
    <button
      type="button"
      class="inline-flex size-7 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-40"
      aria-label="Save file"
      title="Save (Ctrl+S)"
      disabled={!ready || !dirty || saving || reloading}
      onclick={() => void saveContent()}
    >
      {#if saving}
        <Loader2 size={14} class="animate-spin" />
      {:else}
        <Save size={14} />
      {/if}
    </button>
    <button
      type="button"
      class="inline-flex size-7 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-40"
      aria-label="Reload file"
      title="Reload from disk"
      disabled={loading || saving || reloading}
      onclick={requestReload}
    >
      {#if reloading}
        <Loader2 size={14} class="animate-spin" />
      {:else}
        <RotateCw size={14} />
      {/if}
    </button>
    <button
      type="button"
      class="inline-flex size-7 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      aria-label="Close file"
      title="Close file"
      onclick={() => void handleCloseClick()}
    >
      <X size={14} />
    </button>
  </div>
  {#if !workerAvailable}<p class="shrink-0 border-b border-border px-2 py-2 text-xs text-foreground" role="alert">This workspace was removed. This document is retained in memory; copy unsaved text before closing.</p>{/if}

  {#if saveError}
    <div
      class="flex shrink-0 items-start gap-1.5 border-b border-destructive/30 bg-destructive/5 px-2 py-1.5 text-[11px] text-destructive"
      role="alert"
    >
      <CircleAlert size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
      <p class="min-w-0 flex-1 break-words">{saveError}</p>
      <button
        type="button"
        class="shrink-0 rounded-sm border border-border px-1.5 py-px text-[10px] text-foreground transition-colors hover:bg-accent"
        onclick={requestReload}
      >
        Reload
      </button>
      <button
        type="button"
        class="shrink-0 text-muted-foreground transition-colors hover:text-foreground"
        aria-label="Dismiss error"
        onclick={() => (saveError = null)}
      >
        <X size={12} />
      </button>
    </div>
  {/if}
  {#if languageError}<p class="shrink-0 border-b border-border px-2 py-1.5 text-xs text-muted-foreground" role="status">{languageError}</p>{/if}

  <div class="relative min-h-0 flex-1 overflow-hidden">
    <div
      bind:this={container}
      class="h-full min-h-0 w-full overflow-hidden"
      class:hidden={loading || loadError !== null}
    ></div>

    {#if loading}
      <div class="absolute inset-0 flex items-center justify-center gap-2 bg-editor-surface text-xs text-muted-foreground" role="status">
        <Loader2 size={14} class="animate-spin" aria-hidden="true" />
        Loading {baseName(path)}…
      </div>
    {:else if loadError}
      <div class="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-editor-surface px-6 text-center">
        <FileWarning size={18} class="text-muted-foreground/70" aria-hidden="true" />
        <div class="text-sm font-medium text-foreground">Could not open file</div>
        <p class="max-w-md break-words text-xs text-muted-foreground">{loadError}</p>
        <div class="mt-1 flex items-center gap-2">
          <button
            type="button"
            class="h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
            onclick={() => void loadFile(workerId, path, ++loadSeq)}
          >
            Retry
          </button>
          <button
            type="button"
            class="h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent"
            onclick={() => void handleCloseClick()}
          >
            Close
          </button>
        </div>
      </div>
    {/if}
  </div>

  <div class="flex h-6 min-h-6 shrink-0 items-center gap-3 border-t border-border bg-editor-surface px-2 text-[10px] text-muted-foreground">
    <span class="tabular-nums">Ln {currentLine}, Col {currentColumn}</span>
    <span>{languageLabel}</span>
    <span class="ml-auto" aria-live="polite">
      {#if saving}
        Saving…
      {:else if dirty}
        Unsaved
      {:else if loadError}
        Unavailable
      {:else}
        Saved
      {/if}
    </span>
  </div>
</div>

<dialog
  bind:this={closeDialogEl}
  class="m-auto w-full max-w-[calc(100%-2rem)] rounded-lg border border-black/14 bg-dialog-surface p-0 text-foreground shadow-[0_20px_60px_rgba(0,0,0,0.28),inset_0_1px_0_rgba(255,255,255,0.08)] outline-none backdrop:bg-black/55 dark:border-white/14 dark:shadow-[0_24px_72px_rgba(0,0,0,0.55),inset_0_1px_0_rgba(255,255,255,0.06)] sm:max-w-md"
  aria-labelledby="editor-close-title"
  aria-describedby="editor-close-description"
  onclick={handleCloseBackdrop}
  oncancel={(event) => {
    event.preventDefault();
    settleClose(false);
  }}
  onclose={() => {
    if (resolveClosePending) settleClose(false);
  }}
>
  <div class="flex flex-col gap-4 p-6">
    <div>
      <h2 id="editor-close-title" class="text-base font-semibold leading-none">
        Save changes before closing?
      </h2>
      <p id="editor-close-description" class="mt-2 text-sm text-muted-foreground">
        <span class="break-all font-medium text-foreground">{baseName(path)}</span> has unsaved changes.
        Closing now will discard them.
      </p>
    </div>
    {#if confirmError}
      <div class="flex items-start gap-1.5 rounded-md border border-destructive/30 bg-destructive/5 p-2 text-xs text-destructive" role="alert">
        <CircleAlert size={12} class="mt-0.5 shrink-0" aria-hidden="true" />
        <p class="min-w-0 flex-1 break-words">{confirmError}</p>
      </div>
    {/if}
    <div class="flex items-center justify-end gap-2">
      <button
        type="button"
        class="h-8 rounded-md px-3 text-xs text-foreground transition-colors hover:bg-accent"
        onclick={() => settleClose(false)}
      >
        Cancel
      </button>
      <button
        type="button"
        class="h-8 rounded-md px-3 text-xs text-destructive transition-colors hover:bg-destructive/10 disabled:opacity-50"
        disabled={confirmSaving || saving}
        onclick={() => settleClose(true)}
      >
        Discard
      </button>
      <button
        type="button"
        class="inline-flex h-8 items-center justify-center gap-1.5 rounded-md bg-primary px-3 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
        disabled={confirmSaving || saving || reloading}
        onclick={() => void saveAndClose()}
      >
        {#if confirmSaving || saving}
          <Loader2 size={12} class="animate-spin" aria-hidden="true" />
        {/if}
        Save & close
      </button>
    </div>
  </div>
</dialog>

<dialog
  bind:this={reloadDialogEl}
  class="m-auto w-full max-w-[calc(100%-2rem)] rounded-lg border border-black/14 bg-dialog-surface p-0 text-foreground shadow-[0_20px_60px_rgba(0,0,0,0.28),inset_0_1px_0_rgba(255,255,255,0.08)] outline-none backdrop:bg-black/55 dark:border-white/14 dark:shadow-[0_24px_72px_rgba(0,0,0,0.55),inset_0_1px_0_rgba(255,255,255,0.06)] sm:max-w-md"
  aria-labelledby="editor-reload-title"
  aria-describedby="editor-reload-description"
  onclick={handleReloadBackdrop}
  oncancel={(event) => {
    event.preventDefault();
    cancelReload();
  }}
>
  <div class="flex flex-col gap-4 p-6">
    <div>
      <h2 id="editor-reload-title" class="text-base font-semibold leading-none">
        Discard unsaved changes?
      </h2>
      <p id="editor-reload-description" class="mt-2 text-sm text-muted-foreground">
        Reloading <span class="break-all font-medium text-foreground">{baseName(path)}</span> replaces
        your unsaved edits with the file on disk.
      </p>
    </div>
    <div class="flex items-center justify-end gap-2">
      <button
        type="button"
        class="h-8 rounded-md px-3 text-xs text-foreground transition-colors hover:bg-accent"
        onclick={cancelReload}
      >
        Cancel
      </button>
      <button
        type="button"
        class="h-8 rounded-md px-3 text-xs text-destructive transition-colors hover:bg-destructive/10 disabled:opacity-50"
        disabled={saving}
        onclick={discardAndReload}
      >
        Discard & reload
      </button>
    </div>
  </div>
</dialog>
