// Saved terminal commands, modelled on Orca's shared/terminal/quick-commands types (MIT).
// A command either types text into a terminal or starts an agent terminal with a prompt.

export type QuickCommandScope = { type: 'global' } | { type: 'repo'; repoId: string };
export type QuickCommandAction = 'terminal-command' | 'agent-prompt';

export interface QuickCommand {
  id: string;
  label: string;
  scope: QuickCommandScope;
  action: QuickCommandAction;
  /** terminal-command: the text typed into the focused terminal. */
  command: string;
  /** terminal-command: press Enter after typing. */
  appendEnter: boolean;
  /** agent-prompt: the first message sent to a new agent terminal. */
  prompt: string;
}

const STORAGE_KEY = 'minnow.quickCommands.v1';

function parse(raw: unknown): QuickCommand | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const value = raw as Record<string, unknown>;
  if (typeof value.id !== 'string' || typeof value.label !== 'string') return null;
  const scopeRaw = value.scope as Record<string, unknown> | undefined;
  const scope: QuickCommandScope =
    scopeRaw && scopeRaw.type === 'repo' && typeof scopeRaw.repoId === 'string'
      ? { type: 'repo', repoId: scopeRaw.repoId }
      : { type: 'global' };
  return {
    id: value.id,
    label: value.label,
    scope,
    action: value.action === 'agent-prompt' ? 'agent-prompt' : 'terminal-command',
    command: typeof value.command === 'string' ? value.command : '',
    appendEnter: value.appendEnter !== false,
    prompt: typeof value.prompt === 'string' ? value.prompt : ''
  };
}

function load(): QuickCommand[] {
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
    return Array.isArray(raw) ? raw.map(parse).filter((item): item is QuickCommand => item !== null) : [];
  } catch {
    return [];
  }
}

export const quickCommandStore = $state<{ list: QuickCommand[] }>({ list: load() });

export function setQuickCommands(list: QuickCommand[]): void {
  quickCommandStore.list = list;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch {
    // Commands stay usable for this session.
  }
}

/** Commands that apply to a project: global ones plus that project's own. */
export function quickCommandsFor(projectId: string | null): QuickCommand[] {
  return quickCommandStore.list.filter(
    (command) => command.scope.type === 'global' || command.scope.repoId === projectId
  );
}

export function newQuickCommandId(): string {
  return typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
    ? `qc-${crypto.randomUUID()}`
    : `qc-${Date.now().toString(36)}`;
}
