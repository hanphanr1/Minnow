<!--
  Worktree row. Ported from Orca's WorktreeCard surface, header and meta rows
  (workspace-card/{surface,identity,metadata}, MIT): rounded-lg 13px title,
  11px muted branch row, status lane and hover action. One backend worker is one
  worktree + PTY, so this row is the only workspace identity Minnow renders.
-->
<script lang="ts">
  import {
    CircleAlert,
    Ellipsis,
    Folder,
    MessageCircleQuestionMark,
    Pin
  } from 'lucide-svelte';
  import type { MenuAnchor } from './presentation';
  import { workerStatusInfo } from './presentation';
  import type { Project, Worker } from '../../types';

  let {
    worker,
    project,
    active,
    pinned,
    showProject,
    tabIndex,
    onActivate,
    onMenu
  }: {
    worker: Worker;
    project: Project | null;
    active: boolean;
    pinned: boolean;
    showProject: boolean;
    tabIndex: number;
    onActivate: (worker: Worker) => void;
    onMenu: (anchor: MenuAnchor, worker: Worker, restoreTo: HTMLElement | null) => void;
  } = $props();

  let row: HTMLDivElement | null = $state(null);
  const status = $derived(workerStatusInfo(worker));

  function openAt(anchor: MenuAnchor): void {
    onMenu(anchor, worker, row);
  }

  function handleContextMenu(event: MouseEvent): void {
    event.preventDefault();
    openAt({ left: event.clientX, top: event.clientY, bottom: event.clientY });
  }

  function handleActionClick(event: MouseEvent): void {
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    openAt({ left: rect.left, top: rect.top, bottom: rect.bottom });
  }

  function handleKeydown(event: KeyboardEvent): void {
    if ((event.key === 'F10' && event.shiftKey) || event.key === 'ContextMenu') {
      if (!row) return;
      event.preventDefault();
      event.stopPropagation();
      const rect = row.getBoundingClientRect();
      openAt({ left: rect.left, top: rect.top + 16, bottom: rect.top + 16 });
    }
  }
</script>

<div
  bind:this={row}
  role="treeitem"
  aria-level={2}
  aria-selected={active}
  aria-label={`${worker.name}, ${pinned ? 'pinned, ' : ''}${status.label}`}
  data-sidebar-item={`worker:${worker.id}`}
  data-kind="worker"
  data-worker-id={worker.id}
  tabindex={tabIndex}
  class="wt-row group/row relative"
  onclick={() => onActivate(worker)}
  onkeydown={handleKeydown}
  oncontextmenu={handleContextMenu}
>
  <div
    class="wt-card relative ml-1 flex w-[calc(100%-0.25rem)] cursor-pointer flex-col overflow-hidden rounded-lg border border-transparent pr-1.5 pt-1.25 pb-1.5 transition-[background-color,border-color,opacity,box-shadow] duration-200 outline-none select-none"
    class:wt-card-active={active}
    class:wt-card-hover={!active}
  >
    <div class="flex min-w-0 items-center justify-between gap-2">
      <div class="flex min-w-0 flex-1 items-center gap-1.5">
        {#if pinned}
          <Pin class="size-3 shrink-0 text-muted-foreground/70" aria-hidden="true" />
        {/if}
        <span class="block min-w-0 truncate text-[13px] leading-5 font-normal text-foreground">{worker.name}</span>
      </div>
      <div class="flex shrink-0 items-center justify-center gap-1 pr-1.5">
        <span
          class="inline-flex h-3 w-3 shrink-0 items-center justify-center"
          role="img"
          aria-label={status.label}
          title={status.label}
        >
          {#if status.tone === 'working'}
            <span class="wt-spinner block size-2 rounded-full border-2 border-agent-working border-t-transparent"></span>
          {:else if status.tone === 'waiting'}
            <MessageCircleQuestionMark class="size-3 text-agent-question" aria-hidden="true" />
          {:else if status.tone === 'failed'}
            <span class="block size-1.5 rounded-full bg-red-500"></span>
          {:else if status.tone === 'idle'}
            <span class="block size-2 rounded-full bg-emerald-500"></span>
          {:else}
            <span class="block size-1.5 rounded-full bg-muted-foreground"></span>
          {/if}
        </span>
        <button
          type="button"
          tabindex="-1"
          class="wt-row-action opacity-0 group-hover/row:opacity-100 group-focus-within/row:opacity-100"
          aria-label={`Workspace actions for ${worker.name}`}
          onclick={handleActionClick}
        >
          <Ellipsis class="size-3.5" />
        </button>
      </div>
    </div>

    <div class="flex min-w-0 items-center gap-1.5 pl-0.5">
      {#if showProject && project}
        <span
          class="inline-flex h-4 max-w-24 shrink-0 items-center gap-0.5 rounded-[4px] border border-worktree-sidebar-border bg-worktree-sidebar-accent/55 px-0.5"
          aria-label={`Project ${project.name}`}
          title={project.name}
        >
          <span class="inline-flex size-3 shrink-0 items-center justify-center text-2xs">
            <Folder class="size-3" aria-hidden="true" />
          </span>
          <span class="min-w-0 truncate text-3xs leading-none text-muted-foreground lowercase">{project.name}</span>
        </span>
      {/if}
      <span class="min-w-0 flex-1 truncate text-[11px] leading-none text-muted-foreground" title={worker.branch}>
        {worker.branch}
      </span>
      {#if worker.last_error && status.tone === 'failed'}
        <span
          class="inline-flex shrink-0"
          role="img"
          aria-label={`Last error: ${worker.last_error}`}
          title={worker.last_error}
        >
          <CircleAlert class="size-3 text-destructive" aria-hidden="true" />
        </span>
      {/if}
    </div>
  </div>
</div>

<style>
  /* Same contrast as Orca's .worktree-sidebar-card-hover (assets/main.css:1484). */
  .wt-card-hover:hover {
    background: color-mix(in srgb, var(--sidebar-foreground) 4%, transparent);
  }

  :global(.dark) .wt-card-hover:hover {
    background: color-mix(in srgb, var(--sidebar-accent) 40%, transparent);
  }

  /* Orca's [data-worktree-card-active='primary'] surface (assets/main.css:1502). */
  .wt-card-active {
    border-color: var(
      --worktree-sidebar-active-border,
      color-mix(in srgb, var(--worktree-sidebar-border) 40%, transparent)
    );
    background: var(
      --worktree-sidebar-active,
      color-mix(in srgb, var(--worktree-sidebar-foreground) 8%, transparent)
    );
    box-shadow: 0 1px 2px color-mix(in srgb, var(--worktree-sidebar-foreground) 4%, transparent);
  }

  :global(.dark) .wt-card-active {
    border-color: var(
      --worktree-sidebar-active-border,
      color-mix(in srgb, var(--worktree-sidebar-foreground) 18%, var(--worktree-sidebar-border))
    );
    background: var(
      --worktree-sidebar-active,
      color-mix(in srgb, var(--worktree-sidebar-foreground) 10%, transparent)
    );
    box-shadow: 0 1px 2px color-mix(in srgb, var(--worktree-sidebar-foreground) 3%, transparent);
  }

  .wt-row:focus-visible {
    outline: none;
  }

  .wt-row:focus-visible .wt-card {
    border-color: var(--ring);
    box-shadow: 0 0 0 1px var(--ring);
  }

  .wt-row-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted-foreground);
    transition:
      background-color 150ms,
      color 150ms,
      opacity 150ms;
  }

  .wt-row-action:hover,
  .wt-row:focus-within .wt-row-action:focus-visible {
    background: color-mix(in srgb, var(--accent) 70%, transparent);
    color: var(--foreground);
  }

  /* Orca's compositor spinner (assets/main.css:1830) so PTY output cannot stall the ring. */
  .wt-spinner {
    animation: wt-spinner-rotate 86400s steps(1036800, end) infinite;
  }

  @keyframes wt-spinner-rotate {
    to {
      transform: rotate(86400turn);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .wt-spinner {
      animation: none;
    }
  }
</style>
