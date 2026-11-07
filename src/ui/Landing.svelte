<script lang="ts">
  import { FolderPlus, GitBranchPlus, Star } from 'lucide-svelte';
  import minnowMark from '../../src-tauri/icons/128x128@2x.png';

  let {
    hasProjects,
    onAddProject,
    onCreate
  }: {
    hasProjects: boolean;
    onAddProject: () => void;
    onCreate: () => void;
  } = $props();

  const shortcuts = [
    { id: 'create', action: 'Create workspace', keys: ['Ctrl', 'N'] },
    { id: 'up', action: 'Move up workspace', keys: ['Ctrl', '⇧', '↑'] },
    { id: 'down', action: 'Move down workspace', keys: ['Ctrl', '⇧', '↓'] }
  ];
</script>

<!-- Ported from Orca Landing.tsx (MIT). Minnow artwork and labels. -->
<div class="absolute inset-0 flex items-center justify-center overflow-y-auto bg-background">
  <div class="w-full max-w-lg px-6">
    <div class="flex flex-col items-center gap-4 py-8">
      <div
        class="size-20 overflow-hidden rounded-2xl border border-border/80 shadow-lg shadow-black/40"
      >
        <img src={minnowMark} alt="Minnow logo" class="size-full object-cover" />
      </div>
      <h1 class="text-4xl font-bold tracking-tight text-foreground">MINNOW</h1>

      <p class="text-center text-sm text-muted-foreground">
        {hasProjects
          ? 'Select a workspace from the sidebar to begin.'
          : 'Add a project to get started.'}
      </p>

      <div class="flex flex-wrap items-center justify-center gap-2.5">
        <button
          type="button"
          class="inline-flex cursor-pointer items-center gap-1.5 rounded-md border border-border/80 bg-secondary/70 px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
          onclick={onAddProject}
        >
          <FolderPlus class="size-3.5" aria-hidden="true" />
          Add project
        </button>

        <button
          type="button"
          class="inline-flex cursor-pointer items-center gap-1.5 rounded-md border border-border/80 bg-secondary/70 px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
          onclick={onCreate}
        >
          <GitBranchPlus class="size-3.5" aria-hidden="true" />
          Create workspace
        </button>
      </div>

      <div class="mt-6 w-full max-w-xs space-y-2">
        {#each shortcuts as shortcut (shortcut.id)}
          <div class="grid grid-cols-[1fr_auto] items-center gap-3">
            <span class="text-sm text-muted-foreground">{shortcut.action}</span>
            <span class="inline-flex items-center gap-1">
              {#each shortcut.keys as key, index (key)}
                <span
                  class="inline-flex min-w-6 items-center justify-center rounded border border-border/80 bg-secondary/70 px-1.5 py-0.5 text-xs font-medium text-muted-foreground shadow-sm"
                >
                  {key}
                </span>
                {#if index < shortcut.keys.length - 1}
                  <span class="mx-0.5 text-[10px] text-muted-foreground">+</span>
                {/if}
              {/each}
            </span>
          </div>
        {/each}
      </div>
    </div>
  </div>

  <div class="absolute bottom-6 left-0 right-0 flex justify-center">
    <a
      href="https://github.com/hanphanr1/Minnow"
      target="_blank"
      rel="noreferrer"
      class="inline-flex cursor-pointer items-center gap-2 rounded-full border border-amber-500/60 px-4 py-1.5 text-[13px] font-medium text-amber-700 transition-all duration-300 hover:border-amber-500/80 hover:bg-amber-400/10 dark:border-amber-400/30 dark:text-amber-300/90 dark:hover:border-amber-400/50 dark:hover:bg-amber-400/[0.08]"
    >
      <Star class="size-3.5 text-amber-600 transition-all duration-300 dark:text-amber-400/80" aria-hidden="true" />
      Star on GitHub
    </a>
  </div>
</div>
