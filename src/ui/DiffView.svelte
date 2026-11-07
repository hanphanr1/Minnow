<script lang="ts">
  import { onDestroy } from 'svelte';
  import {
    ArrowDown,
    ArrowUp,
    ChevronDown,
    ChevronRight,
    CircleAlert,
    ClipboardCopy,
    Columns2,
    Loader2,
    RefreshCw,
    Rows2
  } from 'lucide-svelte';
  import type { Worker } from '../types';
  import {
    diffStatusColor,
    diffStatusLetter,
    pairHunkLines,
    parseUnifiedDiff,
    splitDiffPath
  } from './diff';

  let {
    text,
    worker,
    onClose,
    onRefresh,
    loading,
    error
  }: {
    text: string;
    worker: Worker;
    onClose: () => void;
    onRefresh: () => void;
    loading: boolean;
    error: string | null;
  } = $props();

  const parsed = $derived(parseUnifiedDiff(text ?? ''));
  const hunkCount = $derived(
    parsed.files.reduce((total, file) => total + file.hunks.length, 0)
  );
  const headerPath = $derived.by(() => {
    const base = worker.path.replace(/[\\/]+$/, '');
    if (parsed.files.length === 1) {
      const separator = base.includes('\\') ? '\\' : '/';
      const relative =
        separator === '\\' ? parsed.files[0].path.replace(/\//g, '\\') : parsed.files[0].path;
      return `${base}${separator}${relative} (diff)`;
    }
    return `${base} (diff)`;
  });
  const headerCopyText = $derived(headerPath.replace(/ \(diff\)$/, ''));
  const headerDisplay = $derived(splitDiffPath(headerPath));

  let sideBySide = $state(false);
  let collapsed = $state<Set<string>>(new Set());
  let copyStatus = $state<'idle' | 'copied' | 'failed'>('idle');
  let bodyElement = $state<HTMLDivElement | null>(null);
  let hunkCursor = $state(-1);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;
  let copyRun = 0;

  onDestroy(() => {
    if (copyTimer) clearTimeout(copyTimer);
  });

  function fileKey(index: number, path: string, oldPath: string | null): string {
    return `${index}:${path}:${oldPath ?? ''}`;
  }

  function toggle(key: string): void {
    const next = new Set(collapsed);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    collapsed = next;
  }

  async function copyPath(path: string): Promise<void> {
    const run = ++copyRun;
    let copied = false;
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(path);
        copied = true;
      }
    } catch {
      copied = false;
    }
    if (run !== copyRun) return;
    copyStatus = copied ? 'copied' : 'failed';
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copyStatus = 'idle';
      copyTimer = null;
    }, 2200);
  }

  function navigateHunk(direction: -1 | 1): void {
    const nodes = bodyElement?.querySelectorAll<HTMLElement>('[data-diff-hunk]');
    if (!nodes || nodes.length === 0) return;
    let next = hunkCursor + direction;
    next = ((next % nodes.length) + nodes.length) % nodes.length;
    hunkCursor = next;
    nodes[next]?.scrollIntoView({ block: 'start' });
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape' || event.defaultPrevented) return;
    if (document.querySelector('dialog[open]')) return;
    event.preventDefault();
    onClose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="flex min-h-0 w-full flex-1 flex-col bg-editor-surface">
  <div class="editor-diff-header">
    <div class="editor-diff-path-row">
      <button
        type="button"
        class="editor-diff-path"
        onclick={() => void copyPath(headerCopyText)}
        title={headerCopyText}
        aria-label="Copy path"
      >
        <span class="editor-diff-path-prefix">{headerDisplay.prefix}</span>
        <span class="editor-diff-path-file">{headerDisplay.fileName}</span>
      </button>
      {#if copyStatus !== 'idle'}
        <span
          class="editor-diff-copy-toast"
          class:failed={copyStatus === 'failed'}
          role="status"
          aria-live="polite"
        >
          {copyStatus === 'copied' ? 'Copied path' : 'Copy failed'}
        </span>
      {/if}
    </div>
    <div class="flex shrink-0 items-center gap-1" role="toolbar" aria-label="Diff actions">
      <button
        type="button"
        class="inline-flex shrink-0 items-center justify-center rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        onclick={() => void copyPath(headerCopyText)}
        aria-label="Copy path"
        title="Copy path"
      >
        <ClipboardCopy size={14} />
      </button>
      <button
        type="button"
        class="inline-flex shrink-0 items-center justify-center rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-muted-foreground"
        onclick={() => (sideBySide = !sideBySide)}
        disabled={hunkCount === 0}
        aria-label={sideBySide ? 'Switch to inline diff' : 'Switch to side-by-side diff'}
        title={sideBySide ? 'Switch to inline diff' : 'Switch to side-by-side diff'}
      >
        {#if sideBySide}
          <Rows2 size={14} />
        {:else}
          <Columns2 size={14} />
        {/if}
      </button>
      <button
        type="button"
        class="inline-flex shrink-0 items-center justify-center rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-muted-foreground"
        onclick={() => navigateHunk(-1)}
        disabled={hunkCount === 0}
        aria-label="Previous change"
        title="Previous change"
      >
        <ArrowUp size={14} />
      </button>
      <button
        type="button"
        class="inline-flex shrink-0 items-center justify-center rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-muted-foreground"
        onclick={() => navigateHunk(1)}
        disabled={hunkCount === 0}
        aria-label="Next change"
        title="Next change"
      >
        <ArrowDown size={14} />
      </button>
      <button
        type="button"
        class="inline-flex shrink-0 items-center justify-center rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-muted-foreground"
        onclick={onRefresh}
        disabled={loading}
        aria-label="Refresh diff"
        title="Refresh diff"
      >
        <RefreshCw size={14} class={loading ? 'animate-spin' : ''} />
      </button>
    </div>
  </div>

  {#if error}
    <div class="m-3 rounded-md border border-destructive/30 bg-destructive/5 p-3" role="alert">
      <div class="flex items-start gap-2 text-xs text-destructive">
        <CircleAlert size={14} class="mt-0.5 shrink-0" aria-hidden="true" />
        <p class="min-w-0 flex-1 break-words">{error}</p>
      </div>
      <button
        type="button"
        class="mt-2 h-6 rounded-md border border-border px-2 text-[11px] text-foreground transition-colors hover:bg-accent disabled:opacity-50"
        onclick={onRefresh}
        disabled={loading}
      >
        Retry
      </button>
    </div>
  {:else if loading}
    <div class="flex flex-1 items-center justify-center gap-2 text-xs text-muted-foreground" role="status">
      <Loader2 size={14} class="animate-spin" aria-hidden="true" />
      Loading changes…
    </div>
  {:else if parsed.files.length === 0}
    <div class="flex flex-1 items-center justify-center px-6 text-center">
      <div>
        <div class="text-sm font-medium text-foreground">No changes</div>
        <div class="mt-1 text-xs text-muted-foreground">
          {text.trim() === ''
            ? 'This worktree has no changes against its base commit.'
            : 'The patch did not contain recognizable file sections.'}
        </div>
      </div>
    </div>
  {:else}
    <div bind:this={bodyElement} class="min-h-0 flex-1 overflow-auto scrollbar-sleek">
      {#each parsed.files as file, index (fileKey(index, file.path, file.oldPath))}
        {@const key = fileKey(index, file.path, file.oldPath)}
        {@const isCollapsed = collapsed.has(key)}
        {@const displayPath = splitDiffPath(file.path)}
        <section class="border-b border-border/40 last:border-b-0">
          <div
            class="sticky top-0 z-10 flex w-full cursor-pointer items-center bg-editor-surface px-3 py-1.5 text-left text-xs transition-colors hover:bg-accent"
            role="button"
            tabindex="0"
            aria-expanded={!isCollapsed}
            onclick={() => toggle(key)}
            onkeydown={(event) => {
              if (event.key !== 'Enter' && event.key !== ' ') return;
              event.preventDefault();
              toggle(key);
            }}
          >
            <span class="flex min-w-0 flex-1 items-center text-muted-foreground">
              <span
                class="flex min-w-0 flex-1 cursor-copy overflow-hidden hover:underline"
                role="button"
                tabindex="0"
                title="Copy path"
                onmousedown={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                }}
                onclick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  void copyPath(file.path);
                }}
                onkeydown={(event) => {
                  if (event.key !== 'Enter' && event.key !== ' ') return;
                  event.preventDefault();
                  event.stopPropagation();
                  void copyPath(file.path);
                }}
              >
                <span class="editor-diff-path-prefix">{displayPath.prefix}</span>
                <span class="editor-diff-path-file">{displayPath.fileName}</span>
              </span>
              <span class="flex shrink-0 items-center">
                <span
                  class="ml-1 font-medium"
                  style:color={diffStatusColor(file.status)}
                  title={file.status}
                >
                  {diffStatusLetter(file.status)}
                </span>
                {#if file.added > 0 || file.removed > 0}
                  <span class="ml-2 tabular-nums">
                    {#if file.added > 0}
                      <span style:color="var(--git-decoration-added)">+{file.added}</span>
                    {/if}
                    {#if file.added > 0 && file.removed > 0}<span> </span>{/if}
                    {#if file.removed > 0}
                      <span style:color="var(--git-decoration-deleted)">-{file.removed}</span>
                    {/if}
                  </span>
                {/if}
                {#if file.binary}
                  <span class="ml-2 text-[10px] uppercase tracking-wide text-muted-foreground">binary</span>
                {/if}
              </span>
            </span>
            <div class="ml-2 flex shrink-0 items-center gap-1">
              {#if isCollapsed}
                <ChevronRight size={14} class="shrink-0 text-muted-foreground" aria-hidden="true" />
              {:else}
                <ChevronDown size={14} class="shrink-0 text-muted-foreground" aria-hidden="true" />
              {/if}
            </div>
          </div>

          {#if !isCollapsed}
            {#if file.binary}
              <div class="px-3 py-2 text-xs text-muted-foreground">
                Binary file — contents are not displayed.
              </div>
            {:else}
              {#if file.status === 'renamed' || file.status === 'copied'}
                <div class="px-3 pt-2 text-[11px] text-muted-foreground">
                  {file.status === 'renamed' ? 'Renamed' : 'Copied'} from
                  <span class="path-mono">{file.oldPath ?? ''}</span>
                </div>
              {/if}
              {#if file.modeOnly}
                <div class="px-3 py-2 text-xs text-muted-foreground">
                  File mode changed from <span class="path-mono">{file.oldMode}</span> to
                  <span class="path-mono">{file.newMode}</span>.
                </div>
              {:else if file.hunks.length === 0 && file.status !== 'renamed' && file.status !== 'copied'}
                <div class="px-3 py-2 text-xs text-muted-foreground">
                  No content changes in this patch.
                </div>
              {/if}
              {#each file.hunks as hunk}
                <div class="hunk-header" data-diff-hunk="true" title={hunk.header}>{hunk.header}</div>
                {#if sideBySide}
                  {#each pairHunkLines(hunk.lines) as paired, rowIndex (rowIndex)}
                    <div class="side-row">
                      <div
                        class="side-pane"
                        class:removed={paired.oldLine?.kind === 'remove'}
                        class:empty={paired.oldLine === null}
                      >
                        {#if paired.oldLine}
                          <div class="side-line">
                            <span class="side-gutter">{paired.oldLine.oldNumber ?? ''}</span>
                            <span class="side-marker">
                              {paired.oldLine.kind === 'remove' ? '-' : ' '}
                            </span>
                            <span class="side-code" title={paired.oldLine.text}>
                              {paired.oldLine.text}
                            </span>
                          </div>
                          {#if paired.oldLine.noNewline}
                            <div class="side-no-newline">\ No newline at end of file</div>
                          {/if}
                        {/if}
                      </div>
                      <div
                        class="side-pane"
                        class:added={paired.newLine?.kind === 'add'}
                        class:empty={paired.newLine === null}
                      >
                        {#if paired.newLine}
                          <div class="side-line">
                            <span class="side-gutter">{paired.newLine.newNumber ?? ''}</span>
                            <span class="side-marker">
                              {paired.newLine.kind === 'add' ? '+' : ' '}
                            </span>
                            <span class="side-code" title={paired.newLine.text}>
                              {paired.newLine.text}
                            </span>
                          </div>
                          {#if paired.newLine.noNewline}
                            <div class="side-no-newline">\ No newline at end of file</div>
                          {/if}
                        {/if}
                      </div>
                    </div>
                  {/each}
                {:else}
                  {#each hunk.lines as line}
                    <div class="diff-line" class:add={line.kind === 'add'} class:remove={line.kind === 'remove'}>
                      <span class="diff-gutter">{line.oldNumber ?? ''}</span>
                      <span class="diff-gutter">{line.newNumber ?? ''}</span>
                      <span class="diff-marker">
                        {line.kind === 'add' ? '+' : line.kind === 'remove' ? '-' : ' '}
                      </span>
                      <span class="diff-code">{line.text}</span>
                    </div>
                    {#if line.noNewline}
                      <div class="no-newline">\ No newline at end of file</div>
                    {/if}
                  {/each}
                {/if}
              {/each}
            {/if}
          {/if}
        </section>
      {/each}
    </div>
  {/if}
</div>

<style>
  .editor-diff-header {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 32px;
    padding: 3px 14px;
    background: var(--editor-surface);
    border-bottom: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
    flex-shrink: 0;
  }

  .editor-diff-path-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex: 1;
    overflow: hidden;
  }

  .editor-diff-path {
    display: flex;
    align-items: center;
    min-width: 0;
    flex: 1 1 auto;
    padding: 0;
    max-width: 100%;
    overflow: hidden;
    border: none;
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.2;
    white-space: nowrap;
    text-align: left;
  }

  .editor-diff-path:hover {
    color: var(--foreground);
  }

  .editor-diff-path-prefix {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .editor-diff-path-file {
    flex: 0 0 auto;
    white-space: nowrap;
  }

  .editor-diff-copy-toast {
    flex-shrink: 0;
    padding: 2px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 72%, transparent);
    color: var(--foreground);
    font-size: 11px;
    line-height: 1.2;
    white-space: nowrap;
  }

  .editor-diff-copy-toast.failed {
    background: color-mix(in srgb, var(--destructive) 14%, transparent);
    color: var(--destructive);
  }

  .hunk-header {
    padding: 3px 12px;
    border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    background: color-mix(in srgb, var(--muted) 55%, transparent);
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 20px;
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
    scroll-margin-top: 32px;
  }

  .diff-line {
    display: flex;
    width: max-content;
    min-width: 100%;
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 20px;
    tab-size: 4;
  }

  .diff-line.add {
    background: var(--diff-added-background);
  }

  .diff-line.remove {
    background: var(--diff-removed-background);
  }

  .diff-gutter {
    flex: none;
    width: 3.75rem;
    padding: 0 0.5rem;
    border-right: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
    color: var(--muted-foreground);
    text-align: right;
    white-space: nowrap;
    user-select: none;
  }

  .diff-line.add .diff-gutter {
    background: var(--diff-added-gutter);
  }

  .diff-line.remove .diff-gutter {
    background: var(--diff-removed-gutter);
  }

  .diff-marker {
    flex: none;
    width: 1.25rem;
    color: var(--muted-foreground);
    text-align: center;
    user-select: none;
  }

  .diff-line.add .diff-marker {
    color: var(--git-decoration-added);
  }

  .diff-line.remove .diff-marker {
    color: var(--git-decoration-deleted);
  }

  .diff-code {
    padding-right: 1rem;
    color: var(--foreground);
    white-space: pre;
  }

  .no-newline {
    padding: 0 12px 0 calc(7.5rem + 1.25rem);
    background: color-mix(in srgb, var(--muted) 35%, transparent);
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 11px;
    font-style: italic;
    line-height: 20px;
    white-space: pre;
  }

  .side-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }

  .side-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    border-right: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }

  .side-pane:last-child {
    border-right: none;
  }

  .side-pane.added {
    background: var(--diff-added-background);
  }

  .side-pane.removed {
    background: var(--diff-removed-background);
  }

  .side-pane.empty {
    background: color-mix(in srgb, var(--muted) 20%, transparent);
  }

  .side-line {
    display: flex;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 20px;
    tab-size: 4;
  }

  .side-gutter {
    flex: none;
    width: 3.25rem;
    padding: 0 0.5rem;
    border-right: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
    color: var(--muted-foreground);
    text-align: right;
    white-space: nowrap;
    user-select: none;
  }

  .side-pane.added .side-gutter {
    background: var(--diff-added-gutter);
  }

  .side-pane.removed .side-gutter {
    background: var(--diff-removed-gutter);
  }

  .side-marker {
    flex: none;
    width: 1.1rem;
    color: var(--muted-foreground);
    text-align: center;
    user-select: none;
  }

  .side-pane.added .side-marker {
    color: var(--git-decoration-added);
  }

  .side-pane.removed .side-marker {
    color: var(--git-decoration-deleted);
  }

  .side-code {
    flex: 1;
    min-width: 0;
    padding-right: 0.5rem;
    overflow: hidden;
    color: var(--foreground);
    text-overflow: ellipsis;
    white-space: pre;
  }

  .side-no-newline {
    padding-left: calc(3.25rem + 1.1rem);
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 11px;
    font-style: italic;
    line-height: 20px;
    white-space: pre;
  }

  .path-mono {
    font-family: var(--font-mono);
    color: var(--foreground);
  }
</style>
