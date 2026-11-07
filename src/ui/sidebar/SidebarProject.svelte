<!--
  Project / pinned section header. Ported from Orca's SectionHeader + header visuals
  (worktree-list/rows/SectionHeader.tsx, MIT): 28px row, 10px inset, 16px icon box,
  13px semibold label, and hover-overlay actions on the sidebar surface.
-->
<script lang="ts">
  import { ChevronDown, Folder, Pin, Plus } from 'lucide-svelte';
  import type { Project } from '../../types';

  let {
    kind,
    project = null,
    label,
    collapsed,
    workerCount,
    itemKey,
    tabIndex,
    selected = false,
    onCreate = null,
    onSelect = null,
    onToggle
  }: {
    kind: 'project' | 'pinned';
    project?: Project | null;
    label: string;
    collapsed: boolean;
    workerCount: number;
    itemKey: string;
    tabIndex: number;
    selected?: boolean;
    onCreate?: (() => void) | null;
    onSelect?: (() => void) | null;
    onToggle: () => void;
  } = $props();

  const collapsible = $derived(workerCount > 0);
  const collapseLabel = $derived(
    `${collapsed ? 'Expand' : 'Collapse'} ${kind === 'pinned' ? 'pinned workspaces' : label}`
  );

  function handleClick(): void {
    if (kind === 'project') onSelect?.();
    else onToggle();
  }

  function handleToggle(event: MouseEvent): void {
    event.stopPropagation();
    onToggle();
  }

  function handleCreate(event: MouseEvent): void {
    event.stopPropagation();
    onCreate?.();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.target !== event.currentTarget) return;
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      event.stopPropagation();
      handleClick();
    }
  }
</script>

<div
  role="treeitem"
  aria-level={1}
  aria-expanded={collapsible ? !collapsed : undefined}
  aria-selected="false"
  aria-current={selected ? 'true' : undefined}
  data-sidebar-item={itemKey}
  data-kind="header"
  data-project-id={project?.id}
  data-pinned={kind === 'pinned' ? 'true' : undefined}
  tabindex={tabIndex}
  class={`group relative flex h-7 w-full cursor-pointer items-center gap-1.5 pr-2 pl-2.5 text-left transition-all ${
    selected && workerCount === 0
      ? 'bg-worktree-sidebar-accent/60 text-worktree-sidebar-accent-foreground'
      : ''
  }`}
  onclick={handleClick}
  onkeydown={handleKeydown}
>
  <div class="flex min-w-0 flex-1 items-center gap-1.5 self-stretch">
    <div class="flex size-4 shrink-0 items-center justify-center rounded-[4px] text-foreground">
      {#if kind === 'pinned'}
        <Pin class="size-3" aria-hidden="true" />
      {:else}
        <Folder class="size-3" aria-hidden="true" />
      {/if}
    </div>
    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 items-center gap-1.5">
        <div class="min-w-0 truncate text-[13px] leading-4 font-semibold">{label}</div>
      </div>
    </div>
  </div>

  <!-- Orca PROJECT_HEADER_ACTIONS_CLASS_NAME: hover/focus overlay; touch keeps actions in flow. -->
  <div
    class="flex shrink-0 cursor-pointer items-center gap-0.5 self-stretch can-hover:absolute can-hover:right-1 can-hover:top-1/2 can-hover:z-10 can-hover:-translate-y-1/2 can-hover:rounded-md can-hover:bg-worktree-sidebar can-hover:pl-1 can-hover:pointer-events-none can-hover:opacity-0 can-hover:transition-opacity group-hover:pointer-events-auto group-hover:opacity-100 has-[:focus-visible]:pointer-events-auto has-[:focus-visible]:opacity-100"
  >
    {#if onCreate}
      <button
        type="button"
        class="flex size-5 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
        aria-label={`Create workspace for ${label}`}
        title={`Create workspace for ${label}`}
        onclick={handleCreate}
      >
        <Plus class="size-3" />
      </button>
    {/if}
    {#if collapsible}
      <button
        type="button"
        class="flex size-5 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
        aria-label={collapseLabel}
        aria-expanded={!collapsed}
        onclick={handleToggle}
      >
        <ChevronDown class={`size-3.5 transition-transform ${collapsed ? '-rotate-90' : ''}`} />
      </button>
    {/if}
  </div>
</div>
