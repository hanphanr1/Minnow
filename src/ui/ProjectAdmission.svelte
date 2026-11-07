<script lang="ts">
  // Layout and flows ported from Orca's AddRepoStartSteps/CloneStep/CreateStep (MIT).
  import { onMount, tick, untrack } from 'svelte';
  import { ArrowLeft, ChevronDown, Download, FolderOpen, GitBranch, Loader2, Plus, X } from 'lucide-svelte';
  import { addProject, errorMessage } from '../api';
  import { cloneProject, createProject, pickProjectFolder } from '../workspace-api';
  import type { Project } from '../types';

  let { repoCount, onAdded, onClose }: {
    repoCount: number;
    onAdded: (project: Project) => void | Promise<void>;
    onClose: () => void;
  } = $props();

  let dialog: HTMLDialogElement;
  let step = $state<'start' | 'path' | 'clone' | 'create'>('start');
  let busy = $state(false);
  let error = $state<string | null>(null);
  let repoPath = $state('');
  let parentPath = $state('');
  let name = $state('');
  let url = $state('');
  let locationOpen = $state(true);
  const cloneName = $derived.by(() => {
    let value = url.trim().replace(/[\\/]+$/, '');
    if (/^[a-z]+:\/\//i.test(value)) {
      try { value = decodeURIComponent(new URL(value).pathname).replace(/\/+$/, ''); }
      catch { return ''; }
    }
    return (value.split(/[\\/:]/).at(-1) ?? '').replace(/\.git$/i, '');
  });
  const title = $derived(step === 'clone' ? 'Clone from URL' : step === 'create' ? 'Create a new project' : 'Add a project');
  const destination = $derived(parentPath.trim() ? `${parentPath.replace(/[\\/]+$/, '')}\\${step === 'clone' ? cloneName : name.trim() || 'project-name'}` : '');

  function changeStep(next: typeof step) { error = null; step = next; }
  function close() { if (!busy) onClose(); }

  async function finish(project: Project) { await onAdded(project); onClose(); }

  async function browseProject() {
    if (busy) return;
    busy = true; error = null;
    try {
      const selected = await pickProjectFolder();
      if (selected) {
        repoPath = selected;
        try { await finish(await addProject(selected)); }
        catch (cause) { step = 'path'; error = errorMessage(cause); }
      }
    } catch (cause) { step = 'path'; error = errorMessage(cause); }
    finally { busy = false; }
  }
  async function browseParent() {
    if (busy) return;
    busy = true; error = null;
    try { const selected = await pickProjectFolder(); if (selected) parentPath = selected; }
    catch (cause) { error = errorMessage(cause); }
    finally { busy = false; }
  }
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    if (step === 'path' && !repoPath.trim()) { error = 'Enter a local repository path.'; return; }
    if ((step === 'create' || step === 'clone') && !parentPath.trim()) { error = 'Choose a parent folder.'; return; }
    if (step === 'create' && !name.trim()) { error = 'Enter a project name.'; return; }
    if (step === 'clone' && !cloneName) { error = 'Enter a repository URL with a project name.'; return; }
    busy = true; error = null;
    try {
      const project = step === 'path' ? await addProject(repoPath.trim())
        : step === 'create' ? await createProject(parentPath.trim(), name.trim())
        : await cloneProject(parentPath.trim(), cloneName, url.trim());
      await finish(project);
    } catch (cause) { error = errorMessage(cause); }
    finally { busy = false; }
  }
  function actionKeys(event: KeyboardEvent) {
    if (!['ArrowUp', 'ArrowDown'].includes(event.key)) return;
    const buttons = Array.from(dialog.querySelectorAll<HTMLButtonElement>('[data-project-action]:not(:disabled)'));
    if (!buttons.length) return;
    event.preventDefault();
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    buttons[(index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length]?.focus();
  }
  $effect(() => {
    const current = step;
    untrack(() => { void tick().then(() => dialog?.querySelector<HTMLElement>(current === 'start' ? '[data-project-browse]' : 'input')?.focus()); });
  });
  onMount(() => { dialog.showModal(); return () => { if (dialog.open) dialog.close(); }; });
</script>

<dialog bind:this={dialog} class="mn-dialog" aria-labelledby="add-project-title" oncancel={event => { event.preventDefault(); close(); }}>
  <form onsubmit={submit} class="min-w-0 space-y-4">
    <div class="flex items-center gap-2">
      {#if step !== 'start'}<button type="button" class="mn-icon-button -ml-1" aria-label="Back to add project" disabled={busy} onclick={() => changeStep('start')}><ArrowLeft size={15}/></button>{/if}
      <h2 id="add-project-title" class="min-w-0 flex-1">{title}</h2>
      <button type="button" class="mn-icon-button" aria-label="Close add project" disabled={busy} onclick={close}><X size={16}/></button>
    </div>
    {#if step === 'start'}
      {#if repoCount === 0}<p>Add a project to get started with Minnow.</p>{/if}
      <div class="space-y-3 pt-2" role="group" aria-label="Ways to add a project">
        <div class="flex items-center gap-2 text-xs text-muted-foreground"><span>Host</span><span class="inline-flex h-7 items-center rounded-md border border-border bg-muted/30 px-2">Local Windows</span></div>
        <button data-project-browse data-project-action type="button" disabled={busy} onkeydown={actionKeys} class="flex min-h-[60px] w-full items-center gap-3 rounded-md border border-input px-3 py-2.5 text-left hover:bg-accent focus:bg-accent focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50" onclick={browseProject}>
          <span class="grid size-7 shrink-0 place-items-center rounded-md bg-muted">{#if busy}<Loader2 size={16} class="animate-spin"/>{:else}<FolderOpen size={16}/>{/if}</span>
          <span class="min-w-0 flex-1"><span class="block text-sm font-medium">Browse folder</span><span class="block text-xs text-muted-foreground">Choose a local Git repository</span></span><kbd class="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">⏎</kbd>
        </button>
        <div class="space-y-1.5">
          <p class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Other ways to add</p>
          <div class="overflow-hidden rounded-md border border-input bg-background">
            <button data-project-action type="button" disabled={busy} onkeydown={actionKeys} class="flex w-full items-center gap-3 px-3 py-2.5 text-left hover:bg-accent focus:bg-accent disabled:opacity-50" onclick={() => changeStep('clone')}><Download size={16}/><span><span class="block text-sm font-medium">Clone from URL</span><span class="block text-xs text-muted-foreground">Clone a repository into a local folder</span></span></button>
            <button data-project-action type="button" disabled={busy} onkeydown={actionKeys} class="flex w-full items-center gap-3 border-t border-border/70 px-3 py-2.5 text-left hover:bg-accent focus:bg-accent disabled:opacity-50" onclick={() => changeStep('create')}><Plus size={16}/><span><span class="block text-sm font-medium">Create new project</span><span class="block text-xs text-muted-foreground">Create a Git repository with an initial commit</span></span></button>
          </div>
        </div>
        <button type="button" class="text-xs text-muted-foreground underline underline-offset-4 hover:text-foreground" disabled={busy} onclick={() => changeStep('path')}>Enter a folder path manually</button>
      </div>
    {:else if step === 'path'}
      <div class="space-y-1.5"><label class="text-xs font-medium" for="repo-path">Project folder</label><div class="flex gap-2"><input id="repo-path" class="mn-input min-w-0 flex-1" bind:value={repoPath} placeholder="C:\path\to\repository" disabled={busy}/><button type="button" class="mn-button px-2" aria-label="Choose project folder" disabled={busy} onclick={browseProject}><FolderOpen size={15}/></button></div></div>
      <button class="mn-button primary w-full" disabled={busy || !repoPath.trim()}>{busy ? 'Adding project…' : 'Add project'}</button>
    {:else if step === 'clone'}
      <p class="text-sm text-muted-foreground">Enter the Git URL and choose where to clone it.</p>
      <div class="space-y-1"><label for="clone-url" class="text-[11px] font-medium text-muted-foreground">Git URL</label><input id="clone-url" class="mn-input h-8 text-xs" bind:value={url} placeholder="https://github.com/user/repo.git" disabled={busy} autocomplete="off" spellcheck="false"/></div>
      <div class="space-y-1"><label for="clone-parent" class="text-[11px] font-medium text-muted-foreground">Parent folder</label><div class="flex gap-2"><input id="clone-parent" class="mn-input h-8 min-w-0 flex-1 text-xs" bind:value={parentPath} placeholder="C:\path\to\projects" disabled={busy}/><button type="button" class="mn-button px-2" aria-label="Choose parent folder" disabled={busy} onclick={browseParent}><FolderOpen size={14}/></button></div></div>
      {#if destination}<p class="break-all font-mono text-[11px] text-muted-foreground">{destination}</p>{/if}
      <button class="mn-button primary w-full" disabled={busy || !cloneName || !parentPath.trim()}>{#if busy}<Loader2 size={14} class="animate-spin"/>{/if}{busy ? 'Cloning…' : 'Clone'}</button>
    {:else}
      <p class="text-sm text-muted-foreground">Name it and Minnow will create a real Git project.</p>
      <div class="space-y-1"><label for="create-project-name" class="text-[11px] font-medium text-muted-foreground">Name</label><input id="create-project-name" class="mn-input h-11 font-mono text-sm" bind:value={name} placeholder="my-project" disabled={busy} autocomplete="off" spellcheck="false"/></div>
      <div class="min-w-0 rounded-md border border-border bg-muted/30">
        <button type="button" class="flex w-full min-w-0 items-start gap-2.5 rounded-md px-3 py-2.5 text-left hover:bg-accent/50" aria-expanded={locationOpen} disabled={busy} onclick={() => { locationOpen = !locationOpen; }}><span class="mt-0.5 grid size-6 shrink-0 place-items-center rounded-md border border-border bg-background/60 text-muted-foreground"><GitBranch size={14}/></span><span class="min-w-0 flex-1"><span class="block truncate text-sm font-medium">Git repository in {parentPath || 'location not selected'}</span><span class="mt-0.5 block truncate font-mono text-[11px] text-muted-foreground">{destination || 'Choose a parent folder before creating.'}</span></span><ChevronDown size={16} class={locationOpen ? 'rotate-180' : ''}/></button>
        {#if locationOpen}<div class="space-y-1.5 border-t border-border px-3 py-3"><label for="create-parent" class="text-[11px] font-medium text-muted-foreground">Location</label><div class="flex gap-2"><input id="create-parent" class="mn-input min-w-0 flex-1 text-xs" bind:value={parentPath} placeholder="C:\path\to\projects" disabled={busy}/><button type="button" class="mn-button px-2" aria-label="Choose parent folder" disabled={busy} onclick={browseParent}><FolderOpen size={14}/></button></div></div>{/if}
      </div>
      <button class="mn-button primary w-full" disabled={busy || !name.trim() || !parentPath.trim()}>{#if busy}<Loader2 size={14} class="animate-spin"/>{/if}{busy ? 'Creating…' : 'Create project'}</button>
    {/if}
    {#if error}<p class="mn-error break-words text-xs" role="alert">{error}</p>{/if}
    {#if busy}<p class="text-xs text-muted-foreground" role="status">{step === 'clone' ? 'Git is cloning the repository. The operation may require your configured Git credentials.' : 'Working with the local project…'}</p>{/if}
  </form>
</dialog>
