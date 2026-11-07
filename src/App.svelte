<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import type { Component } from 'svelte';
  import { PanelLeft, PanelRight, X, GitBranch, Bell, Plug, Monitor, TriangleAlert, Check, TerminalSquare, GitCompareArrows, Plus, ArrowLeft } from 'lucide-svelte';
  import Sidebar from './ui/Sidebar.svelte';
  import Workspace from './ui/Workspace.svelte';
  import Landing from './ui/Landing.svelte';
  import CreateWorkspace from './ui/CreateWorkspace.svelte';
  import WorkspacePalette from './ui/WorkspacePalette.svelte';
  import RightSidebar from './ui/RightSidebar.svelte';
  import ProjectAdmission from './ui/ProjectAdmission.svelte';
  import QuickOpen from './ui/QuickOpen.svelte';
  import ActivityPage from './ui/ActivityPage.svelte';
  import { activity, observeTerminal, type ActivityEvent } from './ui/activity.svelte';
  import WindowControls from './ui/WindowControls.svelte';
  import { loadPreferences, preferenceKey, type UiPreferences } from './ui/preferences';
  import type { Project, Settings, TerminalKind, TerminalSession, Worker } from './types';
  import type { TabDocument } from './ui/workspace-layout';
  import { bootstrap, closeTerminal, createTerminal as createTerminalApi, createWorkers, deleteWorker, errorMessage, isDesktop, killTerminal, killWorker, mergeWorker, openEditor, restartTerminal, restartWorker, saveSettings, subscribeTerminalEvents, subscribeWorkerEvents } from './api';
  import { watchWorkspace } from './workspace-watch';

  type Action = 'restart' | 'stop' | 'editor' | 'diff' | 'merge' | 'delete';
  const desktop = isDesktop();
  let projects = $state<Project[]>([]);
  let workers = $state<Worker[]>([]);
  /** Every terminal of every worktree, keyed by terminal id; closed records included. */
  let terminals = $state<TerminalSession[]>([]);
  let settings = $state<Settings>({ omp_path: 'omp', default_worker_count: 1 });
  let preferences = $state<UiPreferences>(loadPreferences());
  let selectedProjectId = $state<string|null>(null);
  let activeWorkerId = $state<string|null>(null);
  /** Focused terminal inside the selected worktree; never a worktree identity. */
  let activeTerminalId = $state<string|null>(null);
  let page = $state('terminal');
  let sidebarVisible = $state(true);
  let rightVisible = $state(true);
  let panel = $state('explorer');
  let booting = $state(true);
  let bootError = $state<string|null>(null);
  let createOpen = $state(false);
  let paletteOpen = $state(false);
  let projectOpen = $state(false);
  let quickOpenOpen = $state(false);
  let confirmDialog = $state<HTMLDialogElement>();
  let confirm = $state<{action:'merge'|'delete';worker:Worker;busy:boolean;error:string|null}|null>(null);
  type SettingsViewProps = { settings: Settings; preferences: UiPreferences; onSave: (value: Settings) => Promise<void>; onPreferences: (value: UiPreferences) => void; onBack: () => void; initialSection?: string; projects?: Project[] };
  let settingsSection = $state<string|undefined>(undefined);
  let SettingsView = $state<Component<SettingsViewProps>|null>(null);
  let settingsLoading = $state(false);
  let settingsError = $state<string|null>(null);
  let sidebar = $state<{ focusList(): void; toggleCollapsedProjects(): void }>();
  let workspace = $state<{ handleShortcut(event: KeyboardEvent): boolean; focusActive(): void; createTerminal(kind?: TerminalKind): Promise<void>; activateTerminal(terminalId: string): void; openDocument(document: TabDocument): void; requestLeave(): Promise<boolean> }>();
  let history = $state<{projectId:string;workerId?:string}[]>([]);
  let historyIndex = $state(-1);
  let toasts = $state<{id:number;message:string;error:boolean}[]>([]);
  let nextToast = 0;
  const timers = new Set<ReturnType<typeof setTimeout>>();
  const busy = new Set<string>();
  let systemDark = $state(true);
  let disposed = false;
  let hydrating = true;
  let starting = false;
  let unlisten: (() => void) | undefined;
  let unlistenTerminals: (() => void) | undefined;
  const queued: ({worker:Worker}|{id:string}|{terminal:TerminalSession}|{terminalId:string})[] = [];
  const project = $derived(projects.find(p => p.id === selectedProjectId) ?? null);
  const projectWorkers = $derived(workers.filter(w => w.project_id === selectedProjectId));
  const worker = $derived(workers.find(w => w.id === activeWorkerId) ?? null);
  const workerTerminals = $derived(terminals.filter(t => t.worker_id === activeWorkerId));

  function notify(message:string, error = false) {
    const id = ++nextToast;
    toasts = [...toasts.slice(-3), {id,message,error}];
    if (!error) { const timer = setTimeout(() => { toasts = toasts.filter(t => t.id !== id); timers.delete(timer); },5000); timers.add(timer); }
  }
  function applyWorker(value:Worker) {
    const exists = workers.some(w => w.id === value.id);
    workers = exists ? workers.map(w => w.id === value.id ? value : w) : [...workers,value];
  }
  function applyTerminal(value:TerminalSession) {
    const previous = terminals.find(t => t.id === value.id);
    observeTerminal(previous, value, workers.find(w => w.id === value.worker_id));
    const exists = previous !== undefined;
    terminals = exists ? terminals.map(t => t.id === value.id ? value : t) : [...terminals,value];
    return value;
  }
  /** Only for a record that was really deleted. A close keeps its record (closed = true). */
  function removeTerminal(id:string) {
    terminals = terminals.filter(t => t.id !== id);
    if (activeTerminalId === id) activeTerminalId = null;
  }
  function removeWorker(id:string) {
    terminals = terminals.filter(t => t.worker_id !== id);
    workers = workers.filter(w => w.id !== id);
    if (activeWorkerId === id) activeWorkerId = workers.find(w => w.project_id === selectedProjectId)?.id ?? null;
  }
  async function select(projectId:string,workerId?:string,record = true) {
    if (!(await leaveFile())) return;
    selectedProjectId = projectId;
    const nextWorkerId = workerId ?? workers.find(w => w.project_id === projectId)?.id ?? null;
    if (nextWorkerId !== activeWorkerId) activeTerminalId = null;
    activeWorkerId = nextWorkerId;
    page = 'terminal';
    if (record) {
      const previous = history[historyIndex];
      if (previous?.projectId !== projectId || previous.workerId !== activeWorkerId) {
        history = [...history.slice(0,historyIndex+1),{projectId,workerId:activeWorkerId ?? undefined}];
        historyIndex = history.length-1;
      }
    }
  }
  async function navigate(offset:number) {
    const index = historyIndex+offset;
    const target = history[index];
    if (!target || !(await leaveFile())) return;
    historyIndex = index; await select(target.projectId,target.workerId,false);
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
      if (!unlistenTerminals) {
        const remove = await subscribeTerminalEvents(
          value => { if (hydrating) queued.push({terminal:value}); else applyTerminal(value); },
          terminalId => { if (hydrating) queued.push({terminalId}); else removeTerminal(terminalId); },
        );
        if (disposed) { remove(); return; }
        unlistenTerminals = remove;
      }
      const value = await bootstrap();
      if (disposed) return;
      projects = value.projects; workers = value.workers; terminals = value.terminals; settings = value.settings;
      for (const event of queued) {
        if ('worker' in event) applyWorker(event.worker);
        else if ('terminal' in event) applyTerminal(event.terminal);
        else if ('terminalId' in event) removeTerminal(event.terminalId);
        else removeWorker(event.id);
      }
      queued.length = 0;
      try {
        const saved = JSON.parse(localStorage.getItem('minnow.orca.selection') ?? 'null');
        if (saved && projects.some(item => item.id === saved.projectId)) {
          selectedProjectId = saved.projectId;
          activeWorkerId = workers.find(item => item.id === saved.workerId && item.project_id === saved.projectId)?.id
            ?? workers.find(item => item.project_id === saved.projectId)?.id ?? null;
        }
      } catch { /* An unavailable or obsolete UI selection does not block the workspace. */ }
    } catch (error) { if (!disposed) bootError = errorMessage(error); }
    finally { hydrating = false; starting = false; if (!disposed) booting = false; }
  }
  async function create(projectId:string,name:string,count:number,keepOpen = false) {
    if (!(await leaveFile())) throw new Error('Workspace creation cancelled; the current file is still open.');
    const values = await createWorkers(projectId,count,name);
    for (const value of values) applyWorker(value);
    await select(projectId,values[0]?.id); createOpen = keepOpen;
  }
  async function createWorkspaceTerminal(kind:TerminalKind,sourceTerminalId:string|null):Promise<TerminalSession> {
    const target = worker;
    if (!target) throw new Error('Select a workspace before creating a terminal.');
    return applyTerminal(await createTerminalApi(target.id,sourceTerminalId,kind));
  }
  /** Every open editor may hold unsaved changes; false means the user cancelled leaving. */
  async function leaveFile():Promise<boolean> {
    return workspace ? await workspace.requestLeave() : true;
  }
  /** Files and diffs open as tabs in the workspace's active group, beside the terminals. */
  async function openDocument(document:TabDocument) {
    if (!worker) return;
    page = 'terminal';
    await tick();
    workspace?.openDocument(document);
  }
  async function openActivity(event:ActivityEvent) {
    if (!workers.some(w => w.id === event.workerId)) return;
    await select(event.projectId,event.workerId);
    await tick();
    workspace?.activateTerminal(event.terminalId);
  }
  const unreadActivity = $derived(activity.events.filter(event => !event.read).length);
  async function openFile(path:string,line?:number) { await openDocument({kind:'file',path,line}); }
  async function openDiff(target:Worker,path?:string,staged = false) {
    if (target.id !== activeWorkerId) { await select(target.project_id,target.id); }
    await openDocument({kind:'diff',path:path ?? null,staged});
  }
  async function loadSettings() {
    if (SettingsView || settingsLoading) return;
    settingsLoading = true; settingsError = null;
    try { SettingsView = (await import('./ui/Settings.svelte')).default; }
    catch (error) { settingsError = errorMessage(error); }
    finally { settingsLoading = false; }
  }
  async function openSettings(section?:string) {
    settingsSection = section;
    if (!(await leaveFile())) return;
    page = 'settings';
    void loadSettings();
  }
  function closeSettings() { page = 'terminal'; }
  async function action(kind:Action,target:Worker) {
    if (kind === 'diff') { await openDiff(target); return; }
    if (kind === 'delete' && target.id === activeWorkerId && !(await leaveFile())) return;
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
  // Orca entries that are real settings sections open there instead of a placeholder page.
  const SETTINGS_FEATURES: Record<string,string> = { 'Quick Commands': 'quick-commands', 'Notifications': 'notifications', 'Session History': 'session-history', 'Stats': 'stats' };
  async function feature(name:string) { if (SETTINGS_FEATURES[name]) { await openSettings(SETTINGS_FEATURES[name]); return; } if (!(await leaveFile())) return; page = name; }
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
      if (key === 'p' && !event.shiftKey) { event.preventDefault(); if (worker) quickOpenOpen = true; return; }
      if (event.shiftKey && ['b','m','o'].includes(key)) { event.preventDefault(); feature(key === 'b' ? 'Browser' : 'Markdown'); return; }
      if (event.shiftKey && key === 'backspace' && worker) { event.preventDefault(); void action('delete',worker); return; }
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
    if (page === 'terminal') workspace?.handleShortcut(event);
  }
  $effect(() => {
    if (booting || bootError) return;
    try { localStorage.setItem('minnow.orca.selection',JSON.stringify({projectId:selectedProjectId,workerId:activeWorkerId})); }
    catch { /* The live selection remains usable without browser storage. */ }
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
  $effect(() => { if (confirm && confirmDialog && !confirmDialog.open) confirmDialog.showModal(); else if (!confirm && confirmDialog?.open) confirmDialog.close(); });
  const watchedWorkerId = $derived(page === 'terminal' ? activeWorkerId : null);
  $effect(() => {
    const id = watchedWorkerId;
    if (!desktop || !id) return;
    return watchWorkspace(id,error => notify(`File updates: ${errorMessage(error)}`,true));
  });
  onMount(() => {
    const media = matchMedia('(prefers-color-scheme: dark)'); systemDark = media.matches;
    const themeChanged = () => { systemDark = media.matches; }; media.addEventListener('change',themeChanged);
    void start();
    window.addEventListener('keydown',globalKeys,true);
    return () => { disposed = true; unlisten?.(); unlistenTerminals?.(); media.removeEventListener('change',themeChanged); window.removeEventListener('keydown',globalKeys,true); for (const timer of timers) clearTimeout(timer); };
  });
</script>

<a href="#workspace" class="skip-link">Skip to workspace</a>
<div class="app-layout">
  {#if page === 'settings'}
    {#if SettingsView}
      <SettingsView {settings} {preferences} initialSection={settingsSection} {projects} onSave={async value => { settings = await saveSettings(value); }} onPreferences={changePreferences} onBack={closeSettings} />
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
          <Sidebar bind:this={sidebar} {projects} {workers} {selectedProjectId} {activeWorkerId} activeView={page} onSelect={select} onCreate={() => { createOpen = true; }} onAddProject={() => { projectOpen = true; }} onSearch={() => { paletteOpen = true; }} onSettings={() => { void openSettings(); }} onToggle={() => { sidebarVisible = false; }} onBack={() => navigate(-1)} onForward={() => navigate(1)} canBack={historyIndex > 0} canForward={historyIndex < history.length-1} onFeature={feature} onWorkerAction={(kind,value) => { void action(kind,value); }} />
          <div role="slider" aria-label="Resize workspace sidebar" aria-orientation="horizontal" aria-valuemin="220" aria-valuemax="500" aria-valuenow={shellWidths.left} tabindex="0" class="sidebar-resizer right-edge" onpointerdown={event => resize(event,'left')} onkeydown={event => { if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); changePreferences({...preferences,sidebarWidth:Math.max(220,Math.min(500,shellWidths.left+(event.key === 'ArrowLeft' ? -10 : 10)))}); } }}></div>
        </aside>
      {/if}
      <main id="workspace" class="shell-main">
        {#if page !== 'terminal' || !project || !worker || booting || bootError}
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
          {:else if page === 'Activity'}<ActivityPage {workers} onOpen={openActivity} onSettings={() => { void openSettings('notifications'); }}/>
          {:else if page !== 'terminal'}<div class="flex min-h-0 flex-1 flex-col"><header class="border-b border-border px-6 py-5"><h1 class="text-xl font-semibold">{page}</h1></header><div class="m-auto max-w-lg space-y-3 p-8 text-center"><h2 class="text-lg font-medium">{page} is unavailable in this build</h2><p class="text-sm text-muted-foreground">This Orca shell entry is reserved. Minnow does not start the associated service.</p><button class="mn-button" onclick={() => { page = 'terminal'; }}>Back to workspace</button></div></div>
          {:else if project && worker}
            <Workspace bind:this={workspace} {project} {worker} terminals={workerTerminals} {preferences} onActivateTerminal={id => { activeTerminalId = id || null; }} onCreateTerminal={createWorkspaceTerminal} onRestartTerminal={async id => applyTerminal(await restartTerminal(id))} onStopTerminal={async id => applyTerminal(await killTerminal(id))} onCloseTerminal={async id => applyTerminal(await closeTerminal(id))} onStopWorker={async target => { applyWorker(await killWorker(target.id)); }} onAction={(kind,value) => { void action(kind,value); }} onFeature={feature} onError={message => notify(message,true)} onToggleSidebar={toggleSidebar} onToggleRightSidebar={toggleRightSidebar} sidebarVisible={effectiveSidebar} rightSidebarVisible={effectiveRight}/>
          {:else}<Landing hasProjects={projects.length > 0} onAddProject={() => { projectOpen = true; }} onCreate={() => { createOpen = true; }}/>{/if}
        </div>
      </main>
      {#if effectiveRight}<aside class="shell-right" style:width={`${shellWidths.right}px`}><RightSidebar {worker} workers={projectWorkers} {panel} onPanel={setPanel} onRefresh={() => {}} onOpenDiff={(path,staged) => { if (worker) void openDiff(worker,path,staged); }} onOpenFile={(path,line) => { void openFile(path,line); }} onOpenEditor={() => { if (worker) void action('editor',worker); }} onSelect={id => { if (project) void select(project.id,id); }} onClose={() => { rightVisible = false; }}/><div role="slider" aria-label="Resize right sidebar" aria-orientation="horizontal" aria-valuemin="220" aria-valuemax="600" aria-valuenow={shellWidths.right} tabindex="0" class="sidebar-resizer left-edge" onpointerdown={event => resize(event,'right')} onkeydown={event => { if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); changePreferences({...preferences,rightSidebarWidth:Math.max(220,Math.min(600,shellWidths.right+(event.key === 'ArrowLeft' ? 10 : -10)))}); } }}></div></aside>{/if}
    </div>
  {/if}
  <footer class="mn-statusbar"><div class="flex min-w-0 items-center"><button aria-label="Activity" title="Activity" onclick={() => feature('Activity')}><Bell size={12}/>{#if unreadActivity > 0}<span class="tabular-nums">{unreadActivity > 99 ? '99+' : unreadActivity}</span>{/if}</button>{#if worker}<span class="truncate">{worker.status}</span><button onclick={() => { void action('diff',worker); }}><GitBranch size={12}/><span class="max-w-56 truncate">{worker.branch}</span></button>{/if}</div><div class="flex items-center"><button onclick={() => feature('Plugins')} title="Plugins"><Plug size={12}/><span>Off</span></button><button onclick={() => { void openSettings(); }}><Monitor size={12}/><span>{workers.filter(w => w.pid !== null).length} workers</span></button><button onclick={() => feature('Remote Hosts')}>0 hosts</button></div></footer>
</div>
<WindowControls onError={message => notify(message,true)} onBeforeClose={leaveFile}/>
{#if createOpen}<CreateWorkspace {projects} {selectedProjectId} defaultCount={settings.default_worker_count} onCreate={create} onAddProject={() => { createOpen = false; projectOpen = true; }} onClose={() => { createOpen = false; }}/>{/if}
{#if paletteOpen}<WorkspacePalette {projects} {workers} onSelect={select} onCreate={() => { paletteOpen = false; createOpen = true; }} onClose={() => { paletteOpen = false; }}/>{/if}
{#if quickOpenOpen && worker}<QuickOpen {worker} onOpen={(path,line) => { void openFile(path,line); }} onClose={() => { quickOpenOpen = false; }}/>{/if}
{#if projectOpen}<ProjectAdmission repoCount={projects.length} onAdded={async value => { projects = [...projects.filter(p => p.id !== value.id),value]; await select(value.id); }} onClose={() => { projectOpen = false; }}/>{/if}
<dialog bind:this={confirmDialog} class="mn-dialog" aria-labelledby="confirm-title" oncancel={event => { if (confirm?.busy) event.preventDefault(); else confirm = null; }} onclose={() => { if (!confirm?.busy) confirm = null; }}>
  {#if confirm}<h2 id="confirm-title">{confirm.action === 'delete' ? 'Delete workspace' : 'Merge workspace'}</h2><p class="mt-3">{confirm.action === 'delete' ? `Delete ${confirm.worker.name} and its worktree? Dirty files and unmerged changes prevent deletion. Session history is retained.` : `Merge ${confirm.worker.branch} into ${confirm.worker.base_ref}? Stop the worker and commit its changes first. Both trees must be clean.`}</p>{#if confirm.error}<p class="mn-error mt-3" role="alert">{confirm.error}</p>{/if}<div class="mt-6 flex justify-end gap-2"><button class="mn-button" disabled={confirm.busy} onclick={() => { confirm = null; }}>Cancel</button><button class="mn-button" class:danger={confirm.action === 'delete'} disabled={confirm.busy} onclick={confirmAction}>{confirm.busy ? 'Working…' : confirm.action === 'delete' ? 'Delete workspace' : 'Merge'}</button></div>{/if}
</dialog>
<div class="mn-notifications" aria-label="Notifications" aria-live="polite">{#each toasts as toast (toast.id)}<div class="mn-toast" role={toast.error ? 'alert' : 'status'}>{#if toast.error}<TriangleAlert size={16}/>{:else}<Check size={16}/>{/if}<p>{toast.message}</p><button aria-label="Dismiss notification" onclick={() => { toasts = toasts.filter(t => t.id !== toast.id); }}><X size={14}/></button></div>{/each}</div>
