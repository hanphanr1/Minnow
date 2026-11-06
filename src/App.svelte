<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import Terminal from './Terminal.svelte';
  import type { Project, Settings, Worker, WorkerStatus } from './types';
  import minnowMark from '../src-tauri/icons/32x32.png';
  import {
    addProject,
    bootstrap,
    createWorkers,
    deleteWorker,
    diffWorker as runDiff,
    errorMessage,
    isDesktop,
    killWorker,
    mergeWorker as runMerge,
    openEditor,
    restartWorker,
    saveSettings,
    subscribeWorkerEvents,
    type UnlistenFn,
  } from './api';

  const desktop = isDesktop();
  const VIEW_KEY = 'minnow.view';

  type ViewMode = 'grid' | 'tabs';
  type ToastTone = 'error' | 'success' | 'info';

  interface Toast {
    id: number;
    tone: ToastTone;
    message: string;
  }

  type QueuedEvent =
    | { type: 'updated'; worker: Worker }
    | { type: 'removed'; workerId: string };

  interface ConfirmState {
    kind: 'delete' | 'merge';
    workerId: string;
    busy: boolean;
    error: string | null;
  }

  type DiffState =
    | { kind: 'loading' }
    | { kind: 'ready'; text: string }
    | { kind: 'error'; message: string };

  interface ResultState {
    title: string;
    tone: 'success' | 'error';
    text: string;
  }

  const STATUS_LABELS: Record<WorkerStatus, string> = {
    running: 'Running',
    idle: 'Idle',
    'waiting-for-input': 'Waiting for input',
    exited: 'Exited',
  };

  let projects = $state<Project[]>([]);
  let workers = $state<Worker[]>([]);
  let settings = $state<Settings>({ omp_path: 'omp', default_worker_count: 1 });

  let selectedProjectId = $state<string | null>(null);
  let focusedWorkerId = $state<string | null>(null);
  let view = $state<ViewMode>('tabs');

  let booting = $state(true);
  let bootError = $state<string | null>(null);

  let repoPath = $state('');
  let addProjectBusy = $state(false);
  let addProjectError = $state<string | null>(null);

  let workerName = $state('');
  let workerCount = $state('1');
  let creating = $state(false);
  let createError = $state<string | null>(null);

  let busyByWorker = $state<Partial<Record<string, string>>>({});
  let toasts = $state<Toast[]>([]);

  let diffOpen = $state(false);
  let diffTarget = $state<Worker | null>(null);
  let diffState = $state<DiffState | null>(null);
  let diffDialog = $state<HTMLDialogElement | null>(null);

  let confirmState = $state<ConfirmState | null>(null);
  let confirmDialog = $state<HTMLDialogElement | null>(null);

  let resultState = $state<ResultState | null>(null);
  let resultDialog = $state<HTMLDialogElement | null>(null);

  let settingsOpen = $state(false);
  let settingsPath = $state('');
  let settingsCount = $state('1');
  let settingsBusy = $state(false);
  let settingsError = $state<string | null>(null);
  let settingsDialog = $state<HTMLDialogElement | null>(null);

  let listeners: UnlistenFn[] = [];
  let subscribed = false;
  let starting = false;
  let hydrating = true;
  let queuedEvents: QueuedEvent[] = [];
  let workerCountTouched = false;
  let toastSeq = 0;
  const toastTimers = new Map<number, ReturnType<typeof setTimeout>>();

  const projectWorkers = $derived(
    selectedProjectId === null ? [] : workers.filter((worker) => worker.project_id === selectedProjectId),
  );

  const selectedProject = $derived.by(() => {
    const id = selectedProjectId;
    if (id === null) return null;
    return projects.find((project) => project.id === id) ?? null;
  });

  const focusedWorker = $derived.by(() => {
    const id = focusedWorkerId;
    if (id === null) return null;
    return projectWorkers.find((worker) => worker.id === id) ?? null;
  });

  const confirmWorker = $derived.by(() => {
    const state = confirmState;
    if (state === null) return null;
    return workers.find((worker) => worker.id === state.workerId) ?? null;
  });

  function statusLabel(status: WorkerStatus): string {
    return STATUS_LABELS[status] ?? status;
  }

  function projectWorkerCount(projectId: string): number {
    return workers.filter((worker) => worker.project_id === projectId).length;
  }

  function clampCount(value: number): number {
    if (!Number.isFinite(value)) return 1;
    return Math.max(1, Math.round(value));
  }

  function parseCount(value: string): number | null {
    const trimmed = value.trim();
    if (!/^\d+$/.test(trimmed)) return null;
    const parsed = Number(trimmed);
    if (!Number.isSafeInteger(parsed) || parsed < 1) return null;
    return parsed;
  }

  function reconcileFocus(): void {
    if (selectedProjectId === null) {
      focusedWorkerId = null;
      return;
    }
    const list = workers.filter((worker) => worker.project_id === selectedProjectId);
    if (
      focusedWorkerId === null ||
      !list.some((worker) => worker.id === focusedWorkerId)
    ) {
      focusedWorkerId = list[0]?.id ?? null;
    }
  }

  function reconcileSelection(): void {
    if (
      selectedProjectId !== null &&
      !projects.some((project) => project.id === selectedProjectId)
    ) {
      selectedProjectId = null;
    }
    if (selectedProjectId === null && projects.length > 0) {
      selectedProjectId = projects[0].id;
    }
    reconcileFocus();
  }

  function selectProject(projectId: string): void {
    if (selectedProjectId === projectId) return;
    selectedProjectId = projectId;
    focusedWorkerId = null;
    reconcileFocus();
  }

  function focusWorker(workerId: string): void {
    focusedWorkerId = workerId;
  }

  function setView(next: ViewMode): void {
    view = next;
    try {
      localStorage.setItem(VIEW_KEY, next);
    } catch {
      // The window works without a persisted layout preference.
    }
    if (next === 'tabs') reconcileFocus();
  }

  function applyWorker(worker: Worker): void {
    const exists = workers.some((candidate) => candidate.id === worker.id);
    workers = exists
      ? workers.map((candidate) => (candidate.id === worker.id ? worker : candidate))
      : [...workers, worker];
    if (
      selectedProjectId === null &&
      projects.some((project) => project.id === worker.project_id)
    ) {
      selectedProjectId = worker.project_id;
    }
    if (selectedProjectId === worker.project_id) reconcileFocus();
  }

  function removeWorker(workerId: string): void {
    workers = workers.filter((worker) => worker.id !== workerId);
    if (focusedWorkerId === workerId) {
      focusedWorkerId = null;
      reconcileFocus();
    }
    if (busyByWorker[workerId] !== undefined) {
      const next = { ...busyByWorker };
      delete next[workerId];
      busyByWorker = next;
    }
  }

  function handleWorkerUpdated(worker: Worker): void {
    if (hydrating) {
      queuedEvents.push({ type: 'updated', worker });
      return;
    }
    applyWorker(worker);
  }

  function handleWorkerRemoved(workerId: string): void {
    if (hydrating) {
      queuedEvents.push({ type: 'removed', workerId });
      return;
    }
    removeWorker(workerId);
  }

  async function start(): Promise<void> {
    if (!desktop || starting) return;
    starting = true;
    try {
      booting = true;
      bootError = null;
      hydrating = true;
      queuedEvents = [];

      if (!subscribed) {
        try {
          const unlisten = await subscribeWorkerEvents(handleWorkerUpdated, handleWorkerRemoved);
          listeners.push(unlisten);
          subscribed = true;
        } catch (error) {
          bootError = `Could not listen for worker updates: ${errorMessage(error)}`;
          booting = false;
          return;
        }
      }

      try {
        const snapshot = await bootstrap();
        projects = snapshot.projects;
        workers = snapshot.workers;
        settings = snapshot.settings;
        if (!workerCountTouched) {
          workerCount = String(clampCount(snapshot.settings.default_worker_count));
        }

        const queued = queuedEvents;
        queuedEvents = [];
        hydrating = false;
        for (const event of queued) {
          if (event.type === 'updated') applyWorker(event.worker);
          else removeWorker(event.workerId);
        }
        reconcileSelection();
        booting = false;
      } catch (error) {
        booting = false;
        bootError = errorMessage(error);
      }
    } finally {
      starting = false;
    }
  }

  async function submitProject(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (addProjectBusy) return;
    const path = repoPath.trim();
    if (path === '') {
      addProjectError = 'Enter the path to a local git repository.';
      return;
    }
    addProjectBusy = true;
    addProjectError = null;
    try {
      const project = await addProject(path);
      projects = [...projects.filter((candidate) => candidate.id !== project.id), project];
      selectedProjectId = project.id;
      focusedWorkerId = null;
      repoPath = '';
      reconcileFocus();
      pushToast('success', `Added ${project.name}.`);
    } catch (error) {
      addProjectError = errorMessage(error);
    } finally {
      addProjectBusy = false;
    }
  }

  async function submitWorkers(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (creating || selectedProjectId === null) return;
    const count = parseCount(workerCount);
    if (count === null) {
      createError = 'Worker count must be a whole number of at least 1.';
      return;
    }
    const projectId = selectedProjectId;
    creating = true;
    createError = null;
    try {
      const created = await createWorkers(projectId, count, workerName.trim());
      for (const worker of created) applyWorker(worker);
      workerName = '';
      if (selectedProjectId === projectId && created.length > 0) {
        focusedWorkerId = created[0].id;
      }
      reconcileFocus();
      pushToast(
        'success',
        created.length === 1
          ? `Created ${created[0].name}.`
          : `Created ${created.length} workers.`,
      );
    } catch (error) {
      createError = errorMessage(error);
    } finally {
      creating = false;
    }
  }

  async function withWorkerBusy(
    workerId: string,
    busyText: string,
    actionName: string,
    task: () => Promise<void>,
  ): Promise<void> {
    if (busyByWorker[workerId] !== undefined) return;
    busyByWorker = { ...busyByWorker, [workerId]: busyText };
    try {
      await task();
    } catch (error) {
      pushToast('error', `${actionName} failed: ${errorMessage(error)}`);
    } finally {
      const next = { ...busyByWorker };
      delete next[workerId];
      busyByWorker = next;
    }
  }

  function restart(worker: Worker): void {
    void withWorkerBusy(worker.id, 'Restarting', 'Restart', async () => {
      applyWorker(await restartWorker(worker.id));
    });
  }

  function stop(worker: Worker): void {
    void withWorkerBusy(worker.id, 'Stopping', 'Stop', async () => {
      applyWorker(await killWorker(worker.id));
    });
  }

  function launchEditor(worker: Worker): void {
    void withWorkerBusy(worker.id, 'Opening editor', 'Open editor', async () => {
      await openEditor(worker.id);
      pushToast('success', `Opened editor for ${worker.name}.`);
    });
  }

  function openDiff(worker: Worker): void {
    diffTarget = worker;
    diffState = { kind: 'loading' };
    diffOpen = true;
    void loadDiff(worker);
  }

  async function loadDiff(worker: Worker): Promise<void> {
    try {
      const text = await runDiff(worker.id);
      if (!diffOpen || diffTarget?.id !== worker.id) return;
      diffState = { kind: 'ready', text };
    } catch (error) {
      if (!diffOpen || diffTarget?.id !== worker.id) return;
      diffState = { kind: 'error', message: errorMessage(error) };
    }
  }

  function retryDiff(): void {
    if (diffTarget === null) return;
    diffState = { kind: 'loading' };
    void loadDiff(diffTarget);
  }

  function closeDiff(): void {
    if (diffDialog?.open) diffDialog.close();
    else resetDiff();
  }

  function resetDiff(): void {
    diffOpen = false;
    diffTarget = null;
    diffState = null;
  }

  function askDelete(worker: Worker): void {
    confirmState = { kind: 'delete', workerId: worker.id, busy: false, error: null };
  }

  function askMerge(worker: Worker): void {
    confirmState = { kind: 'merge', workerId: worker.id, busy: false, error: null };
  }

  function closeConfirm(): void {
    if (confirmState?.busy) return;
    if (confirmDialog?.open) confirmDialog.close();
    else confirmState = null;
  }

  function resetConfirm(): void {
    confirmState = null;
  }

  async function runConfirm(): Promise<void> {
    const state = confirmState;
    if (state === null || state.busy) return;
    const worker = workers.find((candidate) => candidate.id === state.workerId);
    if (worker === undefined) return;

    confirmState = { ...state, busy: true, error: null };
    try {
      if (state.kind === 'delete') {
        await deleteWorker(worker.id);
        removeWorker(worker.id);
        confirmState = null;
        pushToast('success', `Deleted ${worker.name}. Session history was kept.`);
      } else {
        const output = await runMerge(worker.id);
        confirmState = null;
        openResult({
          title: `Merged ${worker.name}`,
          tone: 'success',
          text:
            output.trim() === ''
              ? `Merged ${worker.branch} into ${worker.base_ref}.`
              : output,
        });
      }
    } catch (error) {
      const message = errorMessage(error);
      if (state.kind === 'delete') {
        if (confirmState !== null) {
          confirmState = { ...confirmState, busy: false, error: message };
        }
      } else {
        confirmState = null;
        openResult({ title: `Merge failed for ${worker.name}`, tone: 'error', text: message });
      }
    }
  }

  function openResult(state: ResultState): void {
    resultState = state;
  }

  function closeResult(): void {
    if (resultDialog?.open) resultDialog.close();
    else resultState = null;
  }

  function resetResult(): void {
    resultState = null;
  }

  function openSettings(): void {
    settingsPath = settings.omp_path;
    settingsCount = String(settings.default_worker_count);
    settingsError = null;
    settingsOpen = true;
  }

  async function submitSettings(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (settingsBusy) return;
    const path = settingsPath.trim();
    const count = parseCount(settingsCount);
    if (path === '') {
      settingsError = 'The omp executable cannot be empty.';
      return;
    }
    if (count === null) {
      settingsError = 'Default worker count must be a whole number of at least 1.';
      return;
    }
    settingsBusy = true;
    settingsError = null;
    try {
      const saved = await saveSettings({ omp_path: path, default_worker_count: count });
      settings = saved;
      if (!workerCountTouched) workerCount = String(clampCount(saved.default_worker_count));
      settingsOpen = false;
      pushToast('success', 'Settings saved.');
    } catch (error) {
      settingsError = errorMessage(error);
    } finally {
      settingsBusy = false;
    }
  }

  function closeSettings(): void {
    if (settingsBusy) return;
    if (settingsDialog?.open) settingsDialog.close();
    else settingsOpen = false;
  }

  function resetSettings(): void {
    settingsOpen = false;
  }

  function pushError(message: string): void {
    pushToast('error', message);
  }

  function pushToast(tone: ToastTone, message: string): void {
    const duplicate = toasts.find((toast) => toast.tone === tone && toast.message === message);
    if (duplicate !== undefined) {
      const timer = toastTimers.get(duplicate.id);
      if (timer !== undefined) {
        clearTimeout(timer);
        toastTimers.delete(duplicate.id);
        scheduleToastDismiss(duplicate.id, tone);
      }
      return;
    }
    const id = ++toastSeq;
    while (toasts.length >= 4) {
      dismissToast(toasts[0].id);
    }
    toasts = [...toasts, { id, tone, message }];
    scheduleToastDismiss(id, tone);
  }

  function scheduleToastDismiss(id: number, tone: ToastTone): void {
    if (tone === 'error') return;
    const timer = setTimeout(() => dismissToast(id), 5000);
    toastTimers.set(id, timer);
  }

  function dismissToast(id: number): void {
    const timer = toastTimers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      toastTimers.delete(id);
    }
    toasts = toasts.filter((toast) => toast.id !== id);
  }

  function handleTabKeydown(event: KeyboardEvent): void {
    if (!['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(event.key)) return;
    const list = projectWorkers;
    if (list.length === 0) return;
    event.preventDefault();
    const currentIndex = list.findIndex((worker) => worker.id === focusedWorkerId);
    let index = currentIndex < 0 ? 0 : currentIndex;
    if (event.key === 'ArrowRight') index = (index + 1) % list.length;
    else if (event.key === 'ArrowLeft') index = (index - 1 + list.length) % list.length;
    else if (event.key === 'Home') index = 0;
    else if (event.key === 'End') index = list.length - 1;
    const target = list[index];
    focusWorker(target.id);
    void tick().then(() => {
      document.getElementById(`tab-${target.id}`)?.focus();
    });
  }

  $effect(() => {
    if (diffOpen) {
      if (diffDialog !== null && !diffDialog.open) diffDialog.showModal();
    } else if (diffDialog?.open) {
      diffDialog.close();
    }
  });

  $effect(() => {
    const state = confirmState;
    if (state !== null && confirmWorker !== null) {
      if (confirmDialog !== null && !confirmDialog.open) confirmDialog.showModal();
    } else if (confirmDialog?.open) {
      confirmDialog.close();
    }
  });

  $effect(() => {
    if (resultState !== null) {
      if (resultDialog !== null && !resultDialog.open) resultDialog.showModal();
    } else if (resultDialog?.open) {
      resultDialog.close();
    }
  });

  $effect(() => {
    if (settingsOpen) {
      if (settingsDialog !== null && !settingsDialog.open) settingsDialog.showModal();
    } else if (settingsDialog?.open) {
      settingsDialog.close();
    }
  });

  onMount(() => {
    if (!desktop) return;
    try {
      const stored = localStorage.getItem(VIEW_KEY);
      if (stored === 'grid' || stored === 'tabs') view = stored;
    } catch {
      // The layout preference is optional.
    }
    void start();
  });

  onDestroy(() => {
    for (const unlisten of listeners) {
      try {
        unlisten();
      } catch (error) {
        console.warn('Failed to remove a worker event listener.', error);
      }
    }
    listeners = [];
    for (const timer of toastTimers.values()) clearTimeout(timer);
    toastTimers.clear();
  });
</script>

{#if !desktop}
  <div class="gate">
    <div class="gate-panel">
      <p class="gate-brand">Minnow</p>
      <h1>Run this workspace as a desktop app</h1>
      <p>
        Minnow uses the Tauri host for real terminals, git worktrees, and the local SQLite
        workspace. Start it with <code>npm run desktop</code>, or run the installed build.
      </p>
      <p class="gate-note">The browser preview has no workers and no terminal surfaces.</p>
    </div>
  </div>
{:else}
  {#snippet statusMark(status: WorkerStatus)}
    <span class="status-dot" data-status={status} aria-hidden="true"></span>
  {/snippet}

  {#snippet workerPanel(worker: Worker)}
    <section
      class="worker-panel"
      class:focused={worker.id === focusedWorkerId}
      aria-label={`${worker.name} terminal`}
    >
      <header class="panel-head">
        <button class="panel-name" type="button" onclick={() => focusWorker(worker.id)}>
          {worker.name}
        </button>
        <span class="status" data-status={worker.status}>
          {@render statusMark(worker.status)}
          {statusLabel(worker.status)}
        </span>
        <span class="panel-branch" title={worker.branch}>{worker.branch}</span>
      </header>
      <div class="panel-body">
        <Terminal {worker} onActivate={focusWorker} onError={pushError} />
      </div>
    </section>
  {/snippet}

  <a class="skip-link" href="#stage">Skip to worker workspace</a>

  <div class="app">
    <aside class="rail" aria-label="Repositories">
      <p class="brand"><img src={minnowMark} width="28" height="28" alt="" />minnow<span class="brand-dim">/</span></p>

      <nav class="project-list" aria-label="Project list">
        {#if projects.length === 0 && !booting}
          <p class="rail-empty">No repositories yet. Add a local git repository below.</p>
        {/if}
        {#each projects as project (project.id)}
          {@const workerTotal = projectWorkerCount(project.id)}
          <button
            type="button"
            class="project"
            class:selected={project.id === selectedProjectId}
            aria-pressed={project.id === selectedProjectId}
            title={project.path}
            onclick={() => selectProject(project.id)}
          >
            <span class="project-name">{project.name}</span>
            <span class="project-meta">
              {workerTotal} {workerTotal === 1 ? 'worker' : 'workers'} on {project.base_ref}
            </span>
          </button>
        {/each}
      </nav>

      <details class="rail-add">
        <summary>Add repository</summary>
        <form class="form-stack" onsubmit={submitProject}>
          <label for="repo-path">Repository path</label>
          <input
            id="repo-path"
            bind:value={repoPath}
            placeholder="C:\src\project"
            autocomplete="off"
            spellcheck="false"
            aria-describedby="repo-help"
          />
          <p class="hint" id="repo-help">A local git repository with at least one commit.</p>
          {#if addProjectError}
            <p class="form-error" role="alert">{addProjectError}</p>
          {/if}
          <button class="btn btn-primary" type="submit" disabled={addProjectBusy}>
            {addProjectBusy ? 'Adding…' : 'Add repository'}
          </button>
        </form>
      </details>
    </aside>

    <main class="workspace">
      <header class="bar">
        <div class="bar-title">
          <h1>{selectedProject?.name ?? 'Minnow'}</h1>
          <p class="bar-path" title={selectedProject?.path ?? ''}>
            {selectedProject?.path ?? 'Local worker workspace'}
          </p>
        </div>
        <div class="bar-tools">
          <div class="segmented" role="group" aria-label="Worker layout">
            <button
              type="button"
              class:active={view === 'grid'}
              aria-pressed={view === 'grid'}
              onclick={() => setView('grid')}
            >
              Grid
            </button>
            <button
              type="button"
              class:active={view === 'tabs'}
              aria-pressed={view === 'tabs'}
              onclick={() => setView('tabs')}
            >
              Tabs
            </button>
          </div>
          <button class="btn" type="button" onclick={openSettings}>Settings</button>
        </div>
      </header>

      {#if selectedProject !== null}
        <form class="create-bar" onsubmit={submitWorkers}>
          <div class="field field-name">
            <label for="worker-name">Worker name</label>
            <input
              id="worker-name"
              bind:value={workerName}
              placeholder="auto"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
          <div class="field field-count">
            <label for="worker-count">Count</label>
            <input
              id="worker-count"
              type="number"
              min="1"
              step="1"
              inputmode="numeric"
              value={workerCount}
              oninput={(event) => {
                workerCount = event.currentTarget.value;
                workerCountTouched = true;
              }}
            />
          </div>
          <button class="btn btn-primary" type="submit" disabled={creating}>
            {creating ? 'Creating…' : 'Create workers'}
          </button>
          {#if createError}
            <p class="form-error" role="alert">{createError}</p>
          {/if}
        </form>
      {/if}

      {#if focusedWorker !== null}
        {@const focusedBusy = busyByWorker[focusedWorker.id]}
        <div class="worker-toolbar" aria-label={`Actions for ${focusedWorker.name}`}>
          <div class="worker-identity">
            <span class="worker-title">{focusedWorker.name}</span>
            <span class="status" data-status={focusedWorker.status}>
              {@render statusMark(focusedWorker.status)}
              {statusLabel(focusedWorker.status)}
            </span>
            <span class="branch" title={focusedWorker.branch}>{focusedWorker.branch}</span>
            {#if focusedBusy}
              <span class="busy" role="status">{focusedBusy}…</span>
            {/if}
          </div>
          <div class="worker-actions">
            <button
              class="btn"
              type="button"
              disabled={focusedBusy !== undefined}
              onclick={() => restart(focusedWorker)}
            >
              Restart
            </button>
            <button
              class="btn"
              type="button"
              disabled={focusedBusy !== undefined || focusedWorker.status === 'exited'}
              title={focusedWorker.status === 'exited' ? 'Worker is already stopped.' : undefined}
              onclick={() => stop(focusedWorker)}
            >
              Stop
            </button>
            <button
              class="btn"
              type="button"
              disabled={focusedBusy !== undefined}
              onclick={() => launchEditor(focusedWorker)}
            >
              Open editor
            </button>
            <button
              class="btn"
              type="button"
              disabled={focusedBusy !== undefined}
              onclick={() => openDiff(focusedWorker)}
            >
              Diff
            </button>
            <button
              class="btn"
              type="button"
              disabled={focusedBusy !== undefined || focusedWorker.status !== 'exited'}
              title={
                focusedWorker.status !== 'exited'
                  ? 'Stop the worker before merging its branch.'
                  : 'Merge this branch into the project base ref.'
              }
              onclick={() => askMerge(focusedWorker)}
            >
              Merge…
            </button>
            <button
              class="btn btn-danger"
              type="button"
              disabled={focusedBusy !== undefined}
              onclick={() => askDelete(focusedWorker)}
            >
              Delete…
            </button>
          </div>
        </div>
      {/if}

      {#if view === 'tabs' && projectWorkers.length > 0}
        <div class="tab-strip" role="tablist" aria-label="Workers">
          {#each projectWorkers as worker (worker.id)}
            <button
              type="button"
              class="worker-tab"
              class:active={worker.id === focusedWorkerId}
              role="tab"
              id={`tab-${worker.id}`}
              aria-selected={worker.id === focusedWorkerId}
              aria-controls={`panel-${worker.id}`}
              aria-label={`${worker.name}, ${statusLabel(worker.status)}`}
              tabindex={worker.id === focusedWorkerId ? 0 : -1}
              onclick={() => focusWorker(worker.id)}
              onkeydown={handleTabKeydown}
            >
              {@render statusMark(worker.status)}
              <span class="tab-name">{worker.name}</span>
            </button>
          {/each}
        </div>
      {/if}

      <div class="stage" id="stage" tabindex="-1">
        {#if booting}
          <div class="stage-state" role="status">
            <p class="state-title">Loading workspace</p>
            <p>Reading projects, workers, and settings from the local database.</p>
          </div>
        {:else if bootError !== null}
          <div class="stage-state error" role="alert">
            <p class="state-title">Workspace failed to load</p>
            <p>{bootError}</p>
            <button class="btn" type="button" onclick={() => void start()}>Retry</button>
          </div>
        {:else if selectedProject === null}
          <div class="stage-state">
            <p class="state-title">No repository selected</p>
            <p>Add a local git repository in the sidebar to create workers.</p>
          </div>
        {:else if projectWorkers.length === 0}
          <div class="stage-state">
            <p class="state-title">No workers in {selectedProject.name}</p>
            <p>
              Create workers above. Each one gets a git worktree, a session directory, and a
              terminal.
            </p>
          </div>
        {:else if view === 'tabs'}
          {#if focusedWorker !== null}
            {#key focusedWorker.id}
              <div
                class="panel-single"
                role="tabpanel"
                id={`panel-${focusedWorker.id}`}
                aria-labelledby={`tab-${focusedWorker.id}`}
              >
                {@render workerPanel(focusedWorker)}
              </div>
            {/key}
          {/if}
        {:else}
          <div class="panel-grid">
            {#each projectWorkers as worker (worker.id)}
              {@render workerPanel(worker)}
            {/each}
          </div>
        {/if}
      </div>
    </main>
  </div>

  <dialog class="modal" bind:this={diffDialog} aria-labelledby="diff-title" onclose={resetDiff}>
    <div class="modal-head">
      <h2 id="diff-title">Diff for {diffTarget?.name ?? 'worker'}</h2>
      <button class="btn btn-quiet" type="button" onclick={closeDiff}>Close</button>
    </div>
    <div class="modal-body">
      {#if diffState === null || diffState.kind === 'loading'}
        <p class="dialog-state" role="status">Running git diff for this worktree.</p>
      {:else if diffState.kind === 'error'}
        <div class="dialog-state error" role="alert">
          <p>{diffState.message}</p>
          <button class="btn" type="button" onclick={retryDiff}>Retry diff</button>
        </div>
      {:else if diffState.text.trim() === ''}
        <p class="dialog-state">No tracked or untracked changes in this worker.</p>
      {:else}
        <textarea class="diff-text" readonly aria-label="Diff output" rows="18" wrap="off" value={diffState.text}></textarea>
      {/if}
    </div>
  </dialog>

  <dialog
    class="modal modal-small"
    bind:this={confirmDialog}
    aria-labelledby="confirm-title"
    onclose={resetConfirm}
    oncancel={(event) => {
      if (confirmState?.busy) event.preventDefault();
    }}
  >
    {#if confirmState !== null && confirmWorker !== null}
      <div class="modal-head">
        <h2 id="confirm-title">
          {confirmState.kind === 'delete' ? `Delete ${confirmWorker.name}?` : `Merge ${confirmWorker.name}?`}
        </h2>
        <button
          class="btn btn-quiet"
          type="button"
          onclick={closeConfirm}
          disabled={confirmState.busy}
        >
          Cancel
        </button>
      </div>
      <div class="modal-body">
        {#if confirmState.kind === 'delete'}
          <p>
            Delete the worktree at <code>{confirmWorker.path}</code> and its branch
            <code>{confirmWorker.branch}</code>? The session history stays on disk.
          </p>
          <p class="hint">
            Refused when the worktree has changes or the branch is not merged into
            <code>{confirmWorker.base_ref}</code>.
          </p>
        {:else}
          <p>
            Merge <code>{confirmWorker.branch}</code> into <code>{confirmWorker.base_ref}</code>?
          </p>
          <p class="hint">
            Requires the worker stopped, both worktrees clean, and the project repository checked
            out on <code>{confirmWorker.base_ref}</code>. Uncommitted changes are not staged or
            committed.
          </p>
        {/if}
        {#if confirmState.error !== null}
          <textarea class="dialog-error" readonly aria-label="Action error" rows="4" value={confirmState.error}></textarea>
        {/if}
        <div class="modal-actions">
          <button
            class="btn"
            class:btn-danger={confirmState.kind === 'delete'}
            class:btn-primary={confirmState.kind === 'merge'}
            type="button"
            onclick={runConfirm}
            disabled={confirmState.busy}
          >
            {#if confirmState.busy}
              Working…
            {:else if confirmState.kind === 'delete'}
              Delete worker
            {:else}
              Merge branch
            {/if}
          </button>
        </div>
      </div>
    {/if}
  </dialog>

  <dialog class="modal" bind:this={resultDialog} aria-labelledby="result-title" onclose={resetResult}>
    {#if resultState !== null}
      <div class="modal-head">
        <h2 id="result-title" class:error-title={resultState.tone === 'error'}>
          {resultState.title}
        </h2>
        <button class="btn btn-quiet" type="button" onclick={closeResult}>Close</button>
      </div>
      <div class="modal-body">
        <textarea class="result-text" readonly aria-label="Action result" rows="12" value={resultState.text}></textarea>
      </div>
    {/if}
  </dialog>

  <dialog
    class="modal modal-small"
    bind:this={settingsDialog}
    aria-labelledby="settings-title"
    onclose={resetSettings}
    oncancel={(event) => {
      if (settingsBusy) event.preventDefault();
    }}
  >
    <div class="modal-head">
      <h2 id="settings-title">Settings</h2>
      <button class="btn btn-quiet" type="button" onclick={closeSettings} disabled={settingsBusy}>
        Close
      </button>
    </div>
    <div class="modal-body">
      <form class="form-grid" onsubmit={submitSettings}>
        <div class="field">
          <label for="settings-path">omp executable</label>
          <input
            id="settings-path"
            bind:value={settingsPath}
            autocomplete="off"
            spellcheck="false"
            aria-describedby="settings-path-help"
          />
          <p class="hint" id="settings-path-help">
            A command name or absolute path used when starting a worker.
          </p>
        </div>
        <div class="field">
          <label for="settings-count">Default workers per project</label>
          <input
            id="settings-count"
            type="number"
            min="1"
            step="1"
            inputmode="numeric"
            value={settingsCount}
            oninput={(event) => {
              settingsCount = event.currentTarget.value;
            }}
          />
        </div>
        {#if settingsError !== null}
          <p class="form-error" role="alert">{settingsError}</p>
        {/if}
        <div class="modal-actions">
          <button class="btn" type="button" onclick={closeSettings} disabled={settingsBusy}>
            Cancel
          </button>
          <button class="btn btn-primary" type="submit" disabled={settingsBusy}>
            {settingsBusy ? 'Saving…' : 'Save settings'}
          </button>
        </div>
      </form>
    </div>
  </dialog>

  <div class="toasts" aria-label="Notifications">
    {#each toasts as toast (toast.id)}
      <div class="toast" data-tone={toast.tone} role={toast.tone === 'error' ? 'alert' : 'status'}>
        <p>{toast.message}</p>
        <button
          class="toast-dismiss"
          type="button"
          onclick={() => dismissToast(toast.id)}
          aria-label="Dismiss notification"
        >
          Dismiss
        </button>
      </div>
    {/each}
  </div>
{/if}
