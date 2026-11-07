<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import {
    ChevronDown,
    CornerDownLeft,
    CaseSensitive,
    Github,
    Sparkles,
    FolderPlus,
    GitBranch,
    LoaderCircle,
    Minus,
    Monitor,
    Plus,
    Terminal as TerminalIcon,
    X
  } from 'lucide-svelte';
  import type { Project } from '../types';
  import { errorMessage } from '../api';

  interface Props {
    projects: Project[];
    selectedProjectId: string | null;
    defaultCount: number;
    onCreate: (projectId: string, name: string, count: number, keepOpen?: boolean) => Promise<void>;
    onAddProject: () => void;
    onClose: () => void;
  }

  let { projects, selectedProjectId, defaultCount, onCreate, onAddProject, onClose }: Props =
    $props();

  function clampCount(value: number): number {
    if (!Number.isFinite(value)) return 1;
    return Math.min(16, Math.max(1, Math.round(value)));
  }

  function parseCount(value: string): number | null {
    const trimmed = value.trim();
    if (!/^\d+$/.test(trimmed)) return null;
    const parsed = Number(trimmed);
    return Number.isSafeInteger(parsed) && parsed >= 1 && parsed <= 16 ? parsed : null;
  }

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let formEl = $state<HTMLFormElement | null>(null);
  let nameInput = $state<HTMLInputElement | null>(null);
  let projectId = $state(
    untrack(
      () =>
        projects.find((project) => project.id === selectedProjectId)?.id ??
        projects[0]?.id ??
        ''
    )
  );
  let name = $state('');
  let count = $state(String(clampCount(untrack(() => defaultCount))));
  let countTouched = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let createMore = $state(false);

  let countNumber = $derived(parseCount(count));
  let selectionReady = $derived(projects.some((project) => project.id === projectId));
  let countInvalid = $derived(countTouched && countNumber === null);
  let canSubmit = $derived(!busy && selectionReady && countNumber !== null);
  let createLabel = $derived(
    countNumber !== null && countNumber > 1 ? `Create ${countNumber} workspaces` : 'Create workspace'
  );
  let selectedProject = $derived(projects.find((project) => project.id === projectId) ?? null);

  let previousFocus: HTMLElement | null = null;
  let unmounted = false;
  let closeNotified = false;
  let lastSelectedProjectId: string | null = untrack(() => selectedProjectId);

  function restoreFocusSoon(): void {
    const target = previousFocus;
    previousFocus = null;
    if (target === null) return;
    requestAnimationFrame(() => {
      if (target.isConnected && document.activeElement === document.body) {
        target.focus({ preventScroll: true });
      }
    });
  }

  function handleNativeClose(): void {
    if (unmounted || closeNotified) return;
    closeNotified = true;
    onClose();
    restoreFocusSoon();
  }

  function requestClose(): void {
    if (busy) return;
    if (dialogEl?.open) dialogEl.close();
    else handleNativeClose();
  }

  function handleCancel(event: Event): void {
    if (busy) event.preventDefault();
  }

  function handleBackdrop(event: MouseEvent): void {
    if (dialogEl === null || event.target !== dialogEl) return;
    const rect = dialogEl.getBoundingClientRect();
    const outside =
      event.clientX < rect.left ||
      event.clientX > rect.right ||
      event.clientY < rect.top ||
      event.clientY > rect.bottom;
    if (outside) requestClose();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (
      event.key === 'Enter' &&
      (event.ctrlKey || event.metaKey) &&
      !event.altKey &&
      !event.shiftKey &&
      !event.isComposing
    ) {
      event.preventDefault();
      (event.currentTarget as HTMLFormElement).requestSubmit();
    }
  }

  function changeCount(delta: number): void {
    countTouched = true;
    count = String(clampCount((countNumber ?? 1) + delta));
  }

  function handleCountInput(event: Event): void {
    count = (event.currentTarget as HTMLInputElement).value;
    countTouched = true;
  }

  function handleCountBlur(): void {
    if (countNumber !== null) return;
    countTouched = true;
    count = String(clampCount(Number(count)));
  }


  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (busy) return;
    const parsedCount = parseCount(count);
    if (parsedCount === null) {
      countTouched = true;
      return;
    }
    if (!selectionReady) return;

    busy = true;
    error = null;
    try {
      await onCreate(projectId, name.trim(), parsedCount, createMore);
      if (!unmounted) {
        if (createMore) { name = ''; await tick(); nameInput?.focus(); }
        else onClose();
      }
    } catch (cause) {
      if (!unmounted) error = errorMessage(cause);
    } finally {
      if (!unmounted) busy = false;
    }
  }

  $effect(() => {
    const ids = projects.map((project) => project.id);
    if (selectedProjectId !== lastSelectedProjectId) {
      lastSelectedProjectId = selectedProjectId;
      if (
        selectedProjectId !== null &&
        selectedProjectId !== projectId &&
        ids.includes(selectedProjectId)
      ) {
        projectId = selectedProjectId;
        return;
      }
    }
    if (ids.includes(projectId)) return;
    const fallback =
      selectedProjectId !== null && ids.includes(selectedProjectId)
        ? selectedProjectId
        : (ids[0] ?? '');
    if (fallback !== projectId) projectId = fallback;
  });

  onMount(() => {
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    formEl?.addEventListener('keydown', handleKeydown);
    if (dialogEl !== null && !dialogEl.open) dialogEl.showModal();
    void tick().then(() => {
      if (!unmounted) nameInput?.focus();
    });
    return () => {
      unmounted = true;
      formEl?.removeEventListener('keydown', handleKeydown);
      if (dialogEl?.open) dialogEl.close();
      restoreFocusSoon();
    };
  });
</script>

<!-- Dialog, fields, and footer ported from Orca NewWorkspaceComposerModal/Card (MIT). -->
<dialog
  bind:this={dialogEl}
  class="m-auto max-h-[calc(100vh-2rem)] w-full max-w-[calc(100%-2rem)] overflow-hidden rounded-lg border border-black/14 bg-dialog-surface p-0 text-foreground shadow-[0_20px_60px_rgba(0,0,0,0.28),inset_0_1px_0_rgba(255,255,255,0.08)] outline-none backdrop:bg-black/55 dark:border-white/14 dark:shadow-[0_24px_72px_rgba(0,0,0,0.55),inset_0_1px_0_rgba(255,255,255,0.06)] sm:max-w-lg"
  aria-labelledby="create-workspace-title"
  aria-describedby="create-workspace-description"
  onclick={handleBackdrop}
  oncancel={handleCancel}
  onclose={handleNativeClose}
>
  <form
    bind:this={formEl}
    class="flex max-h-[calc(100vh-2rem)] min-h-0 flex-col gap-4 p-6"
    novalidate
    onsubmit={submit}
  >
    <header class="flex flex-col gap-1 pr-9 text-center sm:text-left">
      <h2 id="create-workspace-title" class="text-base font-semibold leading-none">
        Create workspace
      </h2>
      <p id="create-workspace-description" class="sr-only">
        Choose a project, an optional workspace name, and how many workspaces to create.
      </p>
    </header>

    <button
      type="button"
      class="absolute top-4 right-4 cursor-pointer rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-ring focus:ring-offset-2 focus:ring-offset-background focus:outline-hidden disabled:pointer-events-none"
      aria-label="Close"
      disabled={busy}
      onclick={requestClose}
    >
      <X class="size-4" aria-hidden="true" />
    </button>

    <div class="min-h-0 flex-1 overflow-y-auto px-2 pb-1 scrollbar-sleek">
      <div class="space-y-4 pt-3">
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <fieldset class="min-w-0 space-y-1">
            <div class="flex h-5 items-center justify-between gap-2">
              <label
                for="create-workspace-project"
                class="text-xs font-medium text-muted-foreground"
              >
                Project
              </label>
              <button
                type="button"
                class="inline-flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-sm text-muted-foreground hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50"
                aria-label="Add project"
                title="Add project"
                disabled={busy}
                onclick={onAddProject}
              >
                <FolderPlus class="size-3" aria-hidden="true" />
              </button>
            </div>

            <div class="relative">
              <select
                id="create-workspace-project"
                class="h-9 w-full min-w-0 appearance-none rounded-md border border-input bg-transparent px-2.5 pr-8 text-sm shadow-xs outline-none transition-[color,box-shadow] focus:border-ring focus:ring-[3px] focus:ring-ring/50 disabled:opacity-50 dark:bg-input/30"
                bind:value={projectId}
                disabled={busy}
                aria-invalid={!selectionReady}
                aria-describedby={projects.length === 0
                  ? 'create-workspace-project-help'
                  : undefined}
              >
                {#if projects.length === 0}
                  <option value="">Choose project</option>
                {/if}
                {#each projects as project (project.id)}
                  <option value={project.id}>{project.name}</option>
                {/each}
              </select>
              <ChevronDown
                class="pointer-events-none absolute top-1/2 right-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
            </div>
          </fieldset>

          <div class="min-w-0 space-y-1">
            <div class="flex h-5 items-center">
              <span class="text-xs font-medium text-muted-foreground">Run on</span>
            </div>
            <div
              class="flex h-9 w-full min-w-0 items-center gap-2 rounded-md border border-input bg-transparent px-2.5 text-sm shadow-xs dark:bg-input/30"
              title="Minnow runs workspaces on this computer."
            >
              <Monitor class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
              <span class="truncate">Local Windows</span>
              <ChevronDown class="ml-auto size-3.5 text-muted-foreground" aria-hidden="true" />
            </div>
          </div>
        </div>

        {#if projects.length === 0}
          <p id="create-workspace-project-help" class="text-[11px] text-muted-foreground">
            Add a project before creating a workspace.
          </p>
        {/if}

        <div class="min-w-0 space-y-1">
          <div class="flex items-center justify-between gap-2">
            <label
              for="create-workspace-name"
              class="min-w-0 truncate text-xs font-medium text-muted-foreground"
            >
              Create From
              <span class="text-muted-foreground/70">[Optional]</span>
            </label>
            {#if selectedProject !== null}
              <span
                aria-label={`Branch from: ${selectedProject.base_ref}`}
                class="inline-flex h-6 max-w-44 items-center gap-1 rounded-md border border-border bg-muted/30 px-1.5 font-mono text-[11px] text-muted-foreground opacity-70"
              >
                <GitBranch class="size-3 shrink-0" aria-hidden="true" />
                <span class="min-w-0 truncate">{selectedProject.base_ref}</span>
              </span>
            {/if}
          </div>
          <div class="mb-2 flex h-8 items-center gap-4 text-xs">
            <button type="button" disabled title="Smart creation is unavailable in this build" class="inline-flex h-full items-center gap-1.5 border-b-2 border-transparent text-muted-foreground"><Sparkles class="size-3.5"/>Smart</button>
            <button type="button" disabled title="GitHub creation is unavailable in this build" class="inline-flex h-full items-center gap-1.5 border-b-2 border-transparent text-muted-foreground"><Github class="size-3.5"/>GitHub</button>
            <button type="button" disabled title="Branch selection is unavailable in this build" class="inline-flex h-full items-center gap-1.5 border-b-2 border-transparent text-muted-foreground"><GitBranch class="size-3.5"/>Branch</button>
            <button type="button" class="inline-flex h-full items-center gap-1.5 border-b-2 border-foreground font-medium text-foreground" aria-pressed="true" onclick={() => nameInput?.focus()}><CaseSensitive class="size-3.5"/>Name</button>
          </div>
          <input
            bind:this={nameInput}
            id="create-workspace-name"
            class="h-9 w-full min-w-0 rounded-md border border-input bg-background px-2.5 text-sm shadow-xs outline-none transition-[color,box-shadow] placeholder:text-muted-foreground focus:border-ring focus:ring-[3px] focus:ring-ring/50 disabled:opacity-50 dark:bg-input/30"
            type="text"
            bind:value={name}
            placeholder="Type a workspace name"
            autocomplete="off"
            spellcheck="false"
            disabled={busy}
            aria-label="Workspace name"
          />
        </div>

        <div class="min-w-0 space-y-1" data-composer-agent>
          <div class="flex h-5 items-center">
            <span class="text-xs font-medium text-muted-foreground">Agent [Optional]</span>
          </div>
          <div
            class="flex h-9 w-full min-w-0 items-center gap-2 rounded-md border border-input bg-transparent px-2.5 text-sm shadow-xs dark:bg-input/30"
            title="Workspaces launch the omp terminal configured in Settings."
          >
            <TerminalIcon class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
            <span class="min-w-0 flex-1 truncate">omp</span>
            <span class="shrink-0 text-[11px] text-muted-foreground">From Settings</span>
          </div>
        </div>

        <details class="group/advanced text-xs">
          <summary class="-ml-2 flex cursor-pointer list-none items-center gap-1.5 rounded-md px-2 py-1.5 text-muted-foreground hover:bg-accent/40 hover:text-foreground"><ChevronDown class="size-3 -rotate-90 group-open/advanced:rotate-0"/>Advanced</summary>
          <div class="space-y-4 pt-3">
        <div class="min-w-0 space-y-1">
          <label for="create-workspace-count" class="text-xs font-medium text-muted-foreground">
            Workspaces
          </label>
          <div class="flex h-9 w-full items-center justify-between rounded-md border border-input bg-transparent px-1 shadow-xs dark:bg-input/30">
            <button
              type="button"
              class="inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-40"
              aria-label="Create fewer workspaces"
              disabled={busy || (countNumber !== null && countNumber <= 1)}
              onclick={() => changeCount(-1)}
            >
              <Minus class="size-3.5" aria-hidden="true" />
            </button>
            <input
              id="create-workspace-count"
              class="h-full w-12 min-w-0 appearance-none bg-transparent text-center text-sm tabular-nums outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
              type="number"
              min="1"
              max="16"
              step="1"
              inputmode="numeric"
              value={count}
              oninput={handleCountInput}
              onblur={handleCountBlur}
              disabled={busy}
              aria-invalid={countInvalid}
              aria-describedby={countInvalid
                ? 'create-workspace-count-help create-workspace-count-error'
                : 'create-workspace-count-help'}
            />
            <button
              type="button"
              class="inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-40"
              aria-label="Create more workspaces"
              disabled={busy || (countNumber !== null && countNumber >= 16)}
              onclick={() => changeCount(1)}
            >
              <Plus class="size-3.5" aria-hidden="true" />
            </button>
          </div>
          <p id="create-workspace-count-help" class="text-[11px] text-muted-foreground">
            1–16 workspaces, each with its own worktree and terminal.
          </p>
          {#if countInvalid}
            <p id="create-workspace-count-error" class="text-[11px] text-destructive" role="alert">
              Enter a whole number from 1 to 16.
            </p>
          {/if}
        </div>
            <div class="space-y-1.5"><span class="text-xs font-medium text-muted-foreground">Branch name</span><div class="rounded-md border border-input px-2.5 py-2 text-sm text-muted-foreground" title="Branch naming is fixed by the backend">Generated from the workspace name</div></div>
            <div class="space-y-1.5"><span class="text-xs font-medium text-muted-foreground">Parent worktree</span><div class="flex items-center justify-between rounded-md border border-input px-2.5 py-2 text-sm text-muted-foreground"><span>No parent</span><ChevronDown class="size-3.5"/></div><p class="text-[11px] text-muted-foreground">Workspace nesting is unavailable in this build.</p></div>
            <div class="space-y-1.5"><span class="text-xs font-medium text-muted-foreground">Sparse checkout</span><div class="rounded-md border border-input px-2.5 py-2 text-sm text-muted-foreground">Full checkout</div></div>
          </div>
        </details>
      </div>
    </div>

    <div class="shrink-0 space-y-1 px-2">
      {#if error !== null}
        <div
          role="alert"
          class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
        >
          {error}
        </div>
      {/if}

      <div class="flex items-center justify-between gap-3">
        <label class="flex cursor-pointer items-center gap-2 text-xs text-muted-foreground">
          <input type="checkbox" role="switch" class="peer sr-only" bind:checked={createMore} disabled={busy}/>
          <span class="relative h-5 w-9 rounded-full bg-input transition-colors peer-checked:bg-primary peer-focus-visible:ring-2 peer-focus-visible:ring-ring after:absolute after:left-0.5 after:top-0.5 after:size-4 after:rounded-full after:bg-background after:transition-transform peer-checked:after:translate-x-4"></span>
          Create more
        </label>
        <button
          type="submit"
          class="inline-flex h-8 shrink-0 cursor-pointer items-center justify-center gap-1.5 whitespace-nowrap rounded-md bg-primary px-3 text-xs font-medium text-primary-foreground transition-all outline-none hover:bg-primary/90 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50"
          disabled={!canSubmit}
          aria-busy={busy}
        >
          {#if busy}
            <LoaderCircle class="size-4 animate-spin" aria-hidden="true" />
          {/if}
          {createLabel}
          <span
            aria-hidden="true"
            class="ml-1 inline-flex items-center gap-0.5 rounded border border-white/20 px-1.5 py-0.5 text-[10px] leading-none font-medium text-current/80"
          >
            Ctrl
            <CornerDownLeft class="size-3" />
          </span>
        </button>
      </div>
    </div>
  </form>
</dialog>
