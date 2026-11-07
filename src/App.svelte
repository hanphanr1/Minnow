<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { Component } from 'svelte';
  import { PanelLeft, PanelRight, X, GitBranch, Bell, Plug, Monitor, TriangleAlert, Check, FolderOpen, Download, FolderPlus, ChevronDown, TerminalSquare, GitCompareArrows, Plus, ArrowLeft } from 'lucide-svelte';
  import Sidebar from './ui/Sidebar.svelte';
  import Workspace from './ui/Workspace.svelte';
  import Landing from './ui/Landing.svelte';
  import CreateWorkspace from './ui/CreateWorkspace.svelte';
  import WorkspacePalette from './ui/WorkspacePalette.svelte';
  import RightSidebar from './ui/RightSidebar.svelte';
  import WindowControls from './ui/WindowControls.svelte';
  import { loadPreferences, preferenceKey, type UiPreferences } from './ui/preferences';
  import type { Project, Settings, Worker } from './types';
  import { addProject, bootstrap, createWorkers, deleteWorker, diffWorker, errorMessage, isDesktop, killWorker, mergeWorker, openEditor, restartWorker, saveSettings, subscribeWorkerEvents } from './api';

  type Action = 'restart' | 'stop' | 'editor' | 'diff' | 'merge' | 'delete';
  const desktop = isDesktop();
  let projects = $state<Project[]>([]);
  let workers = $state<Worker[]>([]);
  let settings = $state<Settings>({ omp_path: 'omp', default_worker_count: 1 });
  let preferences = $state<UiPreferences>(loadPreferences());
  let selectedProjectId = $state<string|null>(null);
  let activeWorkerId = $state<string|null>(null);
  let page = $state('terminal');
  let sidebarVisible = $state(true);
  let rightVisible = $state(true);
  let panel = $state('explorer');
  let booting = $state(true);
  let bootError = $state<string|null>(null);
  let createOpen = $state(false);
  let paletteOpen = $state(false);
  let projectOpen = $state(false);
  let projectPathMode = $state(false);
  let repoPath = $state('');
  let projectBusy = $state(false);
  let projectError = $state<string|null>(null);
  let projectDialog = $state<HTMLDialogElement>();
  let confirmDialog = $state<HTMLDialogElement>();
  let confirm = $state<{action:'merge'|'delete';worker:Worker;busy:boolean;error:string|null}|null>(null);
  let diff = $state<string|null>(null);
  let diffLoading = $state(false);
  let diffError = $state<string|null>(null);
  let diffWorkerId = $state<string|null>(null);
  let diffOpen = $state(false);
  type SettingsViewProps = { settings: Settings; preferences: UiPreferences; onSave: (value: Settings) => Promise<void>; onPreferences: (value: UiPreferences) => void; onBack: () => void };
  type DiffViewProps = { text: string; worker: Worker; onClose: () => void; onRefresh: () => void; loading: boolean; error: string|null };
  let SettingsView = $state<Component<SettingsViewProps>|null>(null);
  let settingsLoading = $state(false);
  let settingsError = $state<string|null>(null);
  let DiffViewComponent = $state<Component<DiffViewProps>|null>(null);
  let diffViewLoading = $state(false);
  let diffViewError = $state<string|null>(null);
  let sidebar = $state<{ focusList(): void; toggleCollapsedProjects(): void }>();
  let workspace = $state<{ handleShortcut(event: KeyboardEvent): boolean; focusActive(): void }>();
  let history = $state<{projectId:string;workerId?:string}[]>([]);
  let historyIndex = $state(-1);
  let toasts = $state<{id:number;message:string;error:boolean}[]>([]);
  let nextToast = 0;
  const timers = new Set<ReturnType<typeof setTimeout>>();
  const busy = new Set<string>();
  let diffGeneration = 0;
  let systemDark = $state(true);
  let disposed = false;
  let hydrating = true;
  let starting = false;
  let unlisten: (() => void) | undefined;
  const queued: ({worker:Worker}|{id:string})[] = [];
  const project = $derived(projects.find(p => p.id === selectedProjectId) ?? null);
  const projectWorkers = $derived(workers.filter(w => w.project_id === selectedProjectId));
  const worker = $derived(workers.find(w => w.id === activeWorkerId) ?? null);
  const diffTarget = $derived(workers.find(w => w.id === diffWorkerId) ?? null);

  function notify(message:string, error = false) {
    const id = ++nextToast;
    toasts = [...toasts.slice(-3), {id,message,error}];
    if (!error) { const timer = setTimeout(() => { toasts = toasts.filter(t => t.id !== id); timers.delete(timer); },5000); timers.add(timer); }
  }
  function applyWorker(value:Worker) {
    const exists = workers.some(w => w.id === value.id);
    workers = exists ? workers.map(w => w.id === value.id ? value : w) : [...workers,value];
  }
  function removeWorker(id:string) {
    workers = workers.filter(w => w.id !== id);
    if (activeWorkerId === id) activeWorkerId = workers.find(w => w.project_id === selectedProjectId)?.id ?? null;
    if (diffWorkerId === id) { diffWorkerId = null; diffOpen = false; diff = null; }
  }
  function select(projectId:string,workerId?:string,record = true) {
    selectedProjectId = projectId;
    activeWorkerId = workerId ?? workers.find(w => w.project_id === projectId)?.id ?? null;
    page = 'terminal'; diffOpen = false;
    if (record) {
      const previous = history[historyIndex];
      if (previous?.projectId !== projectId || previous.workerId !== activeWorkerId) {
        history = [...history.slice(0,historyIndex+1),{projectId,workerId:activeWorkerId ?? undefined}];
        historyIndex = history.length-1;
      }
    }
  }
  function navigate(offset:number) {
    const index = historyIndex+offset;
    const target = history[index];
    if (!target) return;
    historyIndex = index; select(target.projectId,target.workerId,false);
  }
  async function start() {
    if (starting || disposed) return;
    if (!desktop) { booting = false; bootError = 'Open Minnow as a desktop application to access local workspaces.'; return; }
    starting = true; booting = true; bootError = null; hydrating = true;
    queued.length = 0;
    try {
      if (!unlisten) {
        const remove = await subscribeWorkerEvents(
          value => { if (hydrating) queued.push({worker:value}); else applyWorker(value); },
          id => { if (hydrating) queued.push({id}); else removeWorker(id); },
        );
        if (disposed) { remove(); return; }
        unlisten = remove;
      }
      const value = await bootstrap();
      if (disposed) return;
      projects = value.projects; workers = value.workers; settings = value.settings;
      for (const event of queued) { if ('worker' in event) applyWorker(event.worker); else removeWorker(event.id); }
      queued.length = 0;
      try {
        const saved = JSON.parse(localStorage.getItem('minnow.orca.selection') ?? 'null');
        if (saved && projects.some(item => item.id === saved.projectId)) {
          selectedProjectId = saved.projectId;
          activeWorkerId = workers.find(item => item.id === saved.workerId && item.project_id === saved.projectId)?.id ?? null;
        }
      } catch { /* An unavailable or obsolete UI selection does not block the workspace. */ }
    } catch (error) { if (!disposed) bootError = errorMessage(error); }
    finally { hydrating = false; starting = false; if (!disposed) booting = false; }
  }
  async function create(projectId:string,name:string,count:number,keepOpen = false) {
    const values = await createWorkers(projectId,count,name);
    for (const value of values) applyWorker(value);
    select(projectId,values[0]?.id); createOpen = keepOpen;
  }
  async function newTerminal():Promise<Worker|null> {
    if (!project) return null;
    const values = await createWorkers(project.id,1,'');
    for (const value of values) applyWorker(value);
    activeWorkerId = values[0]?.id ?? activeWorkerId;
    return values[0] ?? null;
  }
  async function add(event:SubmitEvent) {
    event.preventDefault(); if (projectBusy) return;
    if (!repoPath.trim()) { projectError = 'Enter the path to a local Git repository.'; return; }
    projectBusy = true; projectError = null;
    try { const value = await addProject(repoPath.trim()); projects = [...projects.filter(p => p.id !== value.id),value]; select(value.id); projectOpen = false; repoPath = ''; }
    catch (error) { projectError = errorMessage(error); }
    finally { projectBusy = false; }
  }
  async function refreshDiff(target = worker) {
    if (!target) return;
    const generation = ++diffGeneration;
    diffWorkerId = target.id; diffLoading = true; diffError = null; diff = null;
    try { const text = await diffWorker(target.id); if (generation === diffGeneration) diff = text; }
    catch (error) { if (generation === diffGeneration) diffError = errorMessage(error); }
    finally { if (generation === diffGeneration) diffLoading = false; }
  }
  async function loadDiffView() {
    if (DiffViewComponent || diffViewLoading) return;
    diffViewLoading = true; diffViewError = null;
    try { DiffViewComponent = (await import('./ui/DiffView.svelte')).default; }
    catch (error) { diffViewError = errorMessage(error); }
    finally { diffViewLoading = false; }
  }
  function openDiff(target:Worker) {
    page = 'terminal';
    diffOpen = true;
    void loadDiffView();
    void refreshDiff(target);
  }
  async function loadSettings() {
    if (SettingsView || settingsLoading) return;
    settingsLoading = true; settingsError = null;
    try { SettingsView = (await import('./ui/Settings.svelte')).default; }
    catch (error) { settingsError = errorMessage(error); }
    finally { settingsLoading = false; }
  }
  function openSettings() {
    page = 'settings'; diffOpen = false;
    void loadSettings();
  }
  function closeSettings() { page = 'terminal'; }
  async function action(kind:Action,target:Worker) {
    if (kind === 'diff') { openDiff(target); return; }
    if (kind === 'merge' || kind === 'delete') { confirm = {action:kind,worker:target,busy:false,error:null}; return; }
    if (busy.has(target.id)) return;
    busy.add(target.id);
    try {
      if (kind === 'restart') applyWorker(await restartWorker(target.id));
      else if (kind === 'stop') applyWorker(await killWorker(target.id));
      else await openEditor(target.id);
    } catch (error) { notify(errorMessage(error),true); }
    finally { busy.delete(target.id); }
  }
  async function confirmAction() {
    if (!confirm || confirm.busy) return;
    const current = confirm; confirm = {...current,busy:true,error:null};
    try {
      if (current.action === 'delete') { await deleteWorker(current.worker.id); removeWorker(current.worker.id); }
      else { const result = await mergeWorker(current.worker.id); notify(result.trim() || `Merged ${current.worker.branch}.`); }
      confirm = null;
    } catch (error) { confirm = {...current,busy:false,error:errorMessage(error)}; }
  }
  function changePreferences(value:UiPreferences) {
    preferences = value;
    try { localStorage.setItem(preferenceKey,JSON.stringify(value)); }
    catch (error) { notify(`Could not save appearance: ${errorMessage(error)}`,true); }
  }
  function setPanel(value:string) { panel = value; rightVisible = true; }
  function feature(name:string) { page = name; diffOpen = false; }
  let viewportWidth = $state(1440);
  const shellViewport = $derived(Math.max(320, Number.isFinite(viewportWidth) ? viewportWidth : 1440));
  const compactShell = $derived(shellViewport < 960);
  const effectiveSidebar = $derived(sidebarVisible);
  // Below 960 one panel owns the window so the center pane never collapses.
  // Full-page routes (Tasks, Mobile, shell-only entries) span to the right edge.
  const effectiveRight = $derived(page === 'terminal' && (compactShell ? rightVisible && !sidebarVisible : rightVisible));
  const shellWidths = $derived.by(() => {
    const leftMin = 220;
    const rightMin = 220;
    if (compactShell) {
      const minSidebar = shellViewport >= 520 ? leftMin : 180;
      const minCenter = shellViewport >= 520 ? 300 : 180;
      const max = Math.max(minSidebar, shellViewport - minCenter);
      const fit = (value:number,fallback:number) => Math.round(Math.min(max, Math.max(minSidebar, Number.isFinite(value) ? value : fallback)));
      return {
        left: effectiveSidebar ? fit(preferences.sidebarWidth,280) : 0,
        right: !effectiveSidebar && effectiveRight ? fit(preferences.rightSidebarWidth,350) : 0,
      };
    }
    const minCenter = shellViewport >= 1180 ? 420 : 400;
    const leftMax = Math.max(leftMin, shellViewport - minCenter - (effectiveRight ? rightMin : 0));
    const left = effectiveSidebar
      ? Math.round(Math.min(leftMax, Math.max(leftMin, Number.isFinite(preferences.sidebarWidth) ? preferences.sidebarWidth : 280)))
      : 0;
    const rightMax = Math.max(rightMin, shellViewport - minCenter - left);
    const right = effectiveRight
      ? Math.round(Math.min(rightMax, Math.max(rightMin, Number.isFinite(preferences.rightSidebarWidth) ? preferences.rightSidebarWidth : 350)))
      : 0;
    return { left, right };
  });
  function toggleSidebar() {
    if (compactShell) {
      if (effectiveSidebar) { sidebarVisible = false; return; }
      sidebarVisible = true; rightVisible = false; return;
    }
    sidebarVisible = !sidebarVisible;
  }
  function toggleRightSidebar() {
    if (page !== 'terminal') return;
    if (compactShell) {
      if (effectiveRight) { rightVisible = false; return; }
      rightVisible = true; sidebarVisible = false; return;
    }
    rightVisible = !rightVisible;
  }
  function resize(event:PointerEvent,side:'left'|'right') {
    const target = event.currentTarget as HTMLElement;
    const startX = event.clientX;
    const initial = side === 'left' ? shellWidths.left : shellWidths.right;
    target.setPointerCapture(event.pointerId);
    const move = (e:PointerEvent) => {
      const width = Math.min(side === 'left' ? 500 : 600,Math.max(220,initial+(e.clientX-startX)*(side === 'left' ? 1 : -1)));
      preferences = {...preferences,[side === 'left' ? 'sidebarWidth' : 'rightSidebarWidth']:width};
    };
    const finish = () => { target.removeEventListener('pointermove',move); target.removeEventListener('pointerup',finish); target.removeEventListener('pointercancel',finish); changePreferences(preferences); };
    target.addEventListener('pointermove',move); target.addEventListener('pointerup',finish); target.addEventListener('pointercancel',finish);
  }
  function globalKeys(event:KeyboardEvent) {
    if (event.defaultPrevented || event.isComposing || event.altKey && event.ctrlKey && !['ArrowLeft','ArrowRight','[',']'].includes(event.key)) return;
    if (document.querySelector('dialog[open]')) return;
    if (page === 'settings') {
      if (event.key === 'Escape' && !document.querySelector('[role="menu"], [role="listbox"]')) { event.preventDefault(); closeSettings(); }
      return;
    }
    const key = event.key.toLowerCase();
    if (event.ctrlKey && !event.altKey) {
      if (key === 'n') { event.preventDefault(); createOpen = true; return; }
      if (key === 'p' && !event.shiftKey) { event.preventDefault(); setPanel('search'); return; }
      if (event.shiftKey && ['b','m','o'].includes(key)) { event.preventDefault(); feature(key === 'b' ? 'Browser' : 'Markdown'); return; }
      if (event.shiftKey && key === 'backspace' && worker) { event.preventDefault(); confirm = {action:'delete',worker,busy:false,error:null}; return; }
      if (key === 'b' && !event.shiftKey) { event.preventDefault(); toggleSidebar(); return; }
      if (key === 'l' && !event.shiftKey) { event.preventDefault(); toggleRightSidebar(); return; }
      if (event.shiftKey && ['e','f','g'].includes(key)) { event.preventDefault(); setPanel(key === 'e' ? 'explorer' : key === 'f' ? 'search' : 'source-control'); return; }
      if (event.shiftKey && key === 'j') { event.preventDefault(); paletteOpen = true; return; }
      if (event.shiftKey && key === 'h') { event.preventDefault(); sidebar?.toggleCollapsedProjects(); return; }
      if (event.shiftKey && key === '0') { event.preventDefault(); sidebar?.focusList(); return; }
      if (event.shiftKey && ['arrowup','arrowdown'].includes(key)) {
        event.preventDefault(); const index = workers.findIndex(w => w.id === activeWorkerId); const next = workers[(index+(key === 'arrowup' ? -1 : 1)+workers.length)%workers.length]; if (next) select(next.project_id,next.id); return;
      }
      if (!event.shiftKey && /^[1-9]$/.test(key)) { const next = workers[Number(key)-1]; if (next) { event.preventDefault(); select(next.project_id,next.id); } return; }
      if (['=','+','-','0'].includes(key) && !(event.target instanceof HTMLElement && event.target.closest('.xterm'))) { event.preventDefault(); changePreferences({...preferences,fontSize:key === '0' ? 14 : Math.max(11,Math.min(20,preferences.fontSize+(key === '-' ? -1 : 1)))}); return; }
      if (event.shiftKey && key === 'y') { event.preventDefault(); feature('Plugins'); return; }
    }
    if (event.ctrlKey && event.altKey && ['ArrowLeft','ArrowRight'].includes(event.key)) { event.preventDefault(); navigate(event.key === 'ArrowLeft' ? -1 : 1); return; }
    if (diffOpen) {
      if (event.ctrlKey && key === 'w') { event.preventDefault(); diffOpen = false; }
      else if (event.ctrlKey && key === 't') { event.preventDefault(); void newTerminal().then(() => { diffOpen = false; }).catch(error => notify(errorMessage(error),true)); }
      return;
    }
    if (page === 'terminal') workspace?.handleShortcut(event);
  }
  $effect(() => {
    if (booting || bootError) return;
    try { localStorage.setItem('minnow.orca.selection',JSON.stringify({projectId:selectedProjectId,workerId:activeWorkerId})); }
    catch { /* The live selection remains usable without browser storage. */ }
  });
  $effect(() => {
    const id = activeWorkerId;
    if (panel !== 'source-control' || !effectiveRight) return;
    untrack(() => {
      if (id) void refreshDiff(workers.find(value => value.id === id) ?? null);
      else { diffGeneration++; diff = null; diffWorkerId = null; diffError = null; diffLoading = false; }
    });
  });
  $effect(() => {
    document.documentElement.classList.toggle('dark',preferences.theme === 'dark' || preferences.theme === 'system' && systemDark);
    document.documentElement.style.fontSize = `${preferences.fontSize/14*16}px`;
    document.documentElement.dataset.reduceMotion = String(preferences.reduceMotion);
  });
  $effect(() => {
    const update = () => { viewportWidth = innerWidth; };
    update();
    window.addEventListener('resize',update);
    return () => window.removeEventListener('resize',update);
  });
  $effect(() => {
    if (projectOpen && projectDialog && !projectDialog.open) {
      projectPathMode = false; projectError = null;
      projectDialog.showModal();
      queueMicrotask(() => projectDialog?.querySelector<HTMLButtonElement>('[data-project-browse]')?.focus());
    } else if (!projectOpen && projectDialog?.open) projectDialog.close();
  });
  $effect(() => { if (confirm && confirmDialog && !confirmDialog.open) confirmDialog.showModal(); else if (!confirm && confirmDialog?.open) confirmDialog.close(); });
  onMount(() => {
    const media = matchMedia('(prefers-color-scheme: dark)'); systemDark = media.matches;
    const themeChanged = () => { systemDark = media.matches; }; media.addEventListener('change',themeChanged);
    void start();
    window.addEventListener('keydown',globalKeys,true);
    return () => { disposed = true; unlisten?.(); media.removeEventListener('change',themeChanged); window.removeEventListener('keydown',globalKeys,true); for (const timer of timers) clearTimeout(timer); };
  });
</script>

<a href="#workspace" class="skip-link">Skip to workspace</a>
<div class="app-layout">
  {#if page === 'settings'}
    {#if SettingsView}
      <SettingsView {settings} {preferences} onSave={async value => { settings = await saveSettings(value); }} onPreferences={changePreferences} onBack={closeSettings} />
    {:else}
      <div class="flex h-full min-h-0 flex-1 flex-col bg-background">
        <div class="titlebar shrink-0" data-tauri-drag-region>
          <button class="mn-icon-button ml-2" aria-label="Back to workspace" title="Back to workspace (Escape)" onclick={closeSettings}><ArrowLeft size={16}/></button>
          <span class="flex-1" data-tauri-drag-region></span>
          <div class="window-controls-titlebar-spacer" aria-hidden="true"></div>
        </div>
        {#if settingsError}
          <div class="m-auto max-w-lg space-y-3 p-6" role="alert">
            <h1 class="text-lg font-semibold">Settings failed to load</h1>
            <p class="mn-error">{settingsError}</p>
            <div class="flex gap-2"><button class="mn-button" onclick={() => { void loadSettings(); }}>Retry</button><button class="mn-button primary" onclick={closeSettings}>Back to workspace</button></div>
          </div>
        {:else}
          <div class="m-auto space-y-3 p-6 text-center" role="status">
            <h1 class="text-lg font-semibold">Loading settings…</h1>
            <p class="text-sm text-muted-foreground">Loading the settings workspace.</p>
          </div>
        {/if}
      </div>
    {/if}
  {:else}
    <div class="app-body">
      {#if effectiveSidebar}
        <aside class="shell-sidebar" style:width={`${shellWidths.left}px`}>
          <Sidebar bind:this={sidebar} {projects} {workers} {selectedProjectId} {activeWorkerId} activeView={page} onSelect={select} onCreate={() => { createOpen = true; }} onAddProject={() => { projectOpen = true; }} onSearch={() => { paletteOpen = true; }} onSettings={openSettings} onToggle={() => { sidebarVisible = false; }} onBack={() => navigate(-1)} onForward={() => navigate(1)} canBack={historyIndex > 0} canForward={historyIndex < history.length-1} onFeature={feature} onWorkerAction={(kind,value) => { void action(kind,value); }} />
          <div role="slider" aria-label="Resize workspace sidebar" aria-orientation="horizontal" aria-valuemin="220" aria-valuemax="500" aria-valuenow={shellWidths.left} tabindex="0" class="sidebar-resizer right-edge" onpointerdown={event => resize(event,'left')} onkeydown={event => { if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); changePreferences({...preferences,sidebarWidth:Math.max(220,Math.min(500,shellWidths.left+(event.key === 'ArrowLeft' ? -10 : 10)))}); } }}></div>
        </aside>
      {/if}
      <main id="workspace" class="shell-main">
        {#if page !== 'terminal' || !project || projectWorkers.length === 0 || booting || bootError}
          <div class="titlebar" data-tauri-drag-region>
            {#if !effectiveSidebar}<button class="mn-icon-button ml-2" aria-label="Toggle sidebar" title="Toggle sidebar (Ctrl+B)" onclick={toggleSidebar}><PanelLeft size={16}/></button>{/if}
            <span class="flex-1" data-tauri-drag-region></span>
            {#if page === 'terminal' && !effectiveRight}<button class="mn-icon-button mr-2" aria-label="Toggle right sidebar" title="Toggle right sidebar (Ctrl+L)" onclick={toggleRightSidebar}><PanelRight size={16}/></button>{/if}
            {#if !effectiveRight}<div class="window-controls-titlebar-spacer"></div>{/if}
          </div>
        {/if}
        <div class="relative flex min-h-0 flex-1 flex-col">
          {#if booting}<div class="flex h-full items-center justify-center text-sm text-muted-foreground" role="status">Loading workspace…</div>
          {:else if bootError}<div class="m-auto max-w-lg space-y-3 p-6" role="alert"><h1 class="text-lg font-semibold">Workspace failed to load</h1><p class="mn-error">{bootError}</p><button class="mn-button" onclick={start}>Retry</button></div>
          {:else if page !== 'terminal'}<div class="flex min-h-0 flex-1 flex-col"><header class="border-b border-border px-6 py-5"><h1 class="text-xl font-semibold">{page}</h1></header><div class="m-auto max-w-lg space-y-3 p-8 text-center"><h2 class="text-lg font-medium">{page} is unavailable in this build</h2><p class="text-sm text-muted-foreground">This Orca shell entry is reserved. Minnow does not start the associated service.</p><button class="mn-button" onclick={() => { page = 'terminal'; }}>Back to workspace</button></div></div>
          {:else if diffOpen && diffTarget}
            <div class="titlebar" data-tauri-drag-region>
              {#if !effectiveSidebar}<button class="mn-icon-button ml-2" aria-label="Toggle sidebar" title="Toggle sidebar (Ctrl+B)" onclick={toggleSidebar}><PanelLeft size={16}/></button>{/if}
              <button class="flex h-full min-w-36 max-w-[300px] flex-1 items-center gap-1.5 border-r border-border bg-tab-inactive px-2 text-xs text-muted-foreground hover:text-foreground" onclick={() => { diffOpen = false; }}><TerminalSquare size={12}/><span class="truncate">{diffTarget.name}</span></button>
              <div class="flex h-full min-w-36 max-w-[300px] flex-1 items-center gap-1.5 border-r border-b-2 border-border border-b-foreground bg-tab-active px-2 text-xs"><GitCompareArrows size={12}/><span class="min-w-0 flex-1 truncate">Changes (diff)</span><button class="mn-icon-button" aria-label="Close diff tab" onclick={() => { diffOpen = false; }}><X size={12}/></button></div>
              <button class="mn-icon-button mx-2" aria-label="New terminal" title="New terminal (Ctrl+T)" onclick={() => { void newTerminal().then(() => { diffOpen = false; }).catch(error => notify(errorMessage(error),true)); }}><Plus size={14}/></button>
              <span class="flex-1" data-tauri-drag-region></span>
              {#if !effectiveRight}<button class="mn-icon-button mr-2" aria-label="Toggle right sidebar" title="Toggle right sidebar (Ctrl+L)" onclick={toggleRightSidebar}><PanelRight size={16}/></button><div class="window-controls-titlebar-spacer"></div>{/if}
            </div>
            {#if DiffViewComponent}
              <DiffViewComponent text={diff ?? ''} worker={diffTarget} onClose={() => { diffOpen = false; }} onRefresh={() => { void refreshDiff(diffTarget); }} loading={diffLoading} error={diffError}/>
            {:else if diffViewError}
              <div class="m-auto max-w-lg space-y-3 p-6" role="alert">
                <h2 class="text-lg font-semibold">Diff view failed to load</h2>
                <p class="mn-error">{diffViewError}</p>
                <div class="flex gap-2"><button class="mn-button" onclick={() => { void loadDiffView(); }}>Retry</button><button class="mn-button primary" onclick={() => { diffOpen = false; }}>Back to workspace</button></div>
              </div>
            {:else}
              <div class="m-auto space-y-2 p-6 text-center" role="status">
                <h2 class="text-lg font-semibold">Loading diff view…</h2>
                <p class="text-sm text-muted-foreground">Preparing the changes viewer.</p>
              </div>
            {/if}
          {:else if project && projectWorkers.length > 0}
            <Workspace bind:this={workspace} {project} workers={projectWorkers} {activeWorkerId} {preferences} onActivate={id => { activeWorkerId = id; }} onCreate={newTerminal} onStop={async target => { applyWorker(await killWorker(target.id)); }} onAction={(kind,value) => { void action(kind,value); }} onFeature={feature} onError={message => notify(message,true)} onToggleSidebar={toggleSidebar} onToggleRightSidebar={toggleRightSidebar} sidebarVisible={effectiveSidebar} rightSidebarVisible={effectiveRight}/>
          {:else}<Landing hasProjects={projects.length > 0} onAddProject={() => { projectOpen = true; }} onCreate={() => { createOpen = true; }}/>{/if}
        </div>
      </main>
      {#if effectiveRight}<aside class="shell-right" style:width={`${shellWidths.right}px`}><RightSidebar {project} {worker} workers={projectWorkers} {panel} {diff} loading={diffLoading} error={diffError} onPanel={setPanel} onRefresh={() => { void refreshDiff(); }} onOpenDiff={() => { if (worker) openDiff(worker); }} onAction={(kind,value) => { void action(kind,value); }} onSelect={id => { activeWorkerId = id; }} onFeature={feature} onClose={() => { rightVisible = false; }}/><div role="slider" aria-label="Resize right sidebar" aria-orientation="horizontal" aria-valuemin="220" aria-valuemax="600" aria-valuenow={shellWidths.right} tabindex="0" class="sidebar-resizer left-edge" onpointerdown={event => resize(event,'right')} onkeydown={event => { if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); changePreferences({...preferences,rightSidebarWidth:Math.max(220,Math.min(600,shellWidths.right+(event.key === 'ArrowLeft' ? 10 : -10)))}); } }}></div></aside>{/if}
    </div>
  {/if}
  <footer class="mn-statusbar"><div class="flex min-w-0 items-center"><button aria-label="Notifications" onclick={() => feature('Activity')}><Bell size={12}/></button>{#if worker}<span class="truncate">{worker.status}</span><button onclick={() => { void action('diff',worker); }}><GitBranch size={12}/><span class="max-w-56 truncate">{worker.branch}</span></button>{/if}</div><div class="flex items-center"><button onclick={() => feature('Plugins')} title="Plugins"><Plug size={12}/><span>Off</span></button><button onclick={openSettings}><Monitor size={12}/><span>{workers.filter(w => w.pid !== null).length} workers</span></button><button onclick={() => feature('Remote Hosts')}>0 hosts</button></div></footer>
</div>
<WindowControls onError={message => notify(message,true)}/>
{#if createOpen}<CreateWorkspace {projects} {selectedProjectId} defaultCount={settings.default_worker_count} onCreate={create} onAddProject={() => { createOpen = false; projectOpen = true; }} onClose={() => { createOpen = false; }}/>{/if}
{#if paletteOpen}<WorkspacePalette {projects} {workers} onSelect={select} onCreate={() => { paletteOpen = false; createOpen = true; }} onClose={() => { paletteOpen = false; }}/>{/if}
<dialog bind:this={projectDialog} class="mn-dialog" aria-labelledby="add-project-title" oncancel={event => { if (projectBusy) event.preventDefault(); else projectOpen = false; }} onclose={() => { projectOpen = false; }}>
  <form onsubmit={add} class="space-y-5">
    <div class="flex items-center justify-between"><h2 id="add-project-title">Add a project</h2><button type="button" class="mn-icon-button" aria-label="Close add project" disabled={projectBusy} onclick={() => { projectOpen = false; }}><X size={16}/></button></div>
    {#if projects.length === 0}<p>Add a project to get started with Minnow.</p>{/if}
    <div class="flex items-center gap-2 text-xs text-muted-foreground"><span>Host</span><span class="inline-flex h-7 items-center gap-1 rounded-md border border-border bg-muted/30 px-2"><span>Local Windows</span><ChevronDown size={12}/></span></div>
    {#if projectPathMode}
      <div class="space-y-2"><label class="block text-xs font-medium" for="repo-path">Project folder</label><input id="repo-path" class="mn-input" bind:value={repoPath} placeholder="C:\path\to\repository" disabled={projectBusy}/><p class="text-xs text-muted-foreground">Enter a local Git repository path. Minnow does not include a native folder picker.</p></div>
      {#if projectError}<p class="mn-error" role="alert">{projectError}</p>{/if}
      <div class="flex justify-end"><button class="mn-button primary" disabled={projectBusy}>{projectBusy ? 'Adding project…' : 'Add project'}</button></div>
    {:else}
      <button data-project-browse type="button" class="flex min-h-[60px] w-full items-center gap-3 rounded-md border border-input px-3 py-2.5 text-left hover:bg-accent focus:bg-accent focus-visible:ring-2 focus-visible:ring-ring" onclick={() => { projectPathMode = true; }}><span class="grid size-7 shrink-0 place-items-center rounded-md bg-muted"><FolderOpen size={16}/></span><span class="flex-1"><span class="block text-sm font-medium">Browse folder</span><span class="block text-xs text-muted-foreground">Local project, Git repo, or folder with many repos</span></span><kbd class="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">⏎</kbd></button>
      <div class="space-y-1.5">
        <p class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Other ways to add</p>
        <div class="overflow-hidden rounded-md border border-input bg-background">
          <button type="button" disabled class="flex w-full items-center gap-3 px-3 py-2.5 text-left opacity-60" title="Unavailable in this build"><Download size={16}/><span><span class="block text-sm font-medium">Clone from URL</span><span class="block text-xs text-muted-foreground">Unavailable in this build</span></span></button>
          <button type="button" disabled class="flex w-full items-center gap-3 border-t border-border/70 px-3 py-2.5 text-left opacity-60" title="Unavailable in this build"><Plus size={16}/><span><span class="block text-sm font-medium">Create new project</span><span class="block text-xs text-muted-foreground">Unavailable in this build</span></span></button>
        </div>
      </div>
    {/if}
  </form>
</dialog>
<dialog bind:this={confirmDialog} class="mn-dialog" aria-labelledby="confirm-title" oncancel={event => { if (confirm?.busy) event.preventDefault(); else confirm = null; }} onclose={() => { if (!confirm?.busy) confirm = null; }}>
  {#if confirm}<h2 id="confirm-title">{confirm.action === 'delete' ? 'Delete workspace' : 'Merge workspace'}</h2><p class="mt-3">{confirm.action === 'delete' ? `Delete ${confirm.worker.name} and its worktree? Dirty files and unmerged changes prevent deletion. Session history is retained.` : `Merge ${confirm.worker.branch} into ${confirm.worker.base_ref}? Stop the worker and commit its changes first. Both trees must be clean.`}</p>{#if confirm.error}<p class="mn-error mt-3" role="alert">{confirm.error}</p>{/if}<div class="mt-6 flex justify-end gap-2"><button class="mn-button" disabled={confirm.busy} onclick={() => { confirm = null; }}>Cancel</button><button class="mn-button" class:danger={confirm.action === 'delete'} disabled={confirm.busy} onclick={confirmAction}>{confirm.busy ? 'Working…' : confirm.action === 'delete' ? 'Delete workspace' : 'Merge'}</button></div>{/if}
</dialog>
<div class="mn-notifications" aria-label="Notifications" aria-live="polite">{#each toasts as toast (toast.id)}<div class="mn-toast" role={toast.error ? 'alert' : 'status'}>{#if toast.error}<TriangleAlert size={16}/>{:else}<Check size={16}/>{/if}<p>{toast.message}</p><button aria-label="Dismiss notification" onclick={() => { toasts = toasts.filter(t => t.id !== toast.id); }}><X size={14}/></button></div>{/each}</div>
