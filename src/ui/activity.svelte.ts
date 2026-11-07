// Agent attention routing modelled on Orca's NotificationSettings
// (shared/notification-settings-types.ts) and its Activity feed (components/activity), MIT.
// Events come from real terminal status transitions; nothing here is synthesized.
import { invoke } from '@tauri-apps/api/core';
import type { TerminalSession, Worker } from '../types';

export type NotificationSound = 'none' | 'two-tone' | 'ding' | 'blip';

export interface NotificationPrefs {
  enabled: boolean;
  /** An agent went quiet after working: its turn is probably complete. */
  agentTaskComplete: boolean;
  /** An agent is asking for permission or input. */
  agentNeedsInput: boolean;
  /** An agent or shell process ended. */
  terminalExited: boolean;
  /** Skip the system notification while Minnow is the focused window. */
  suppressWhenFocused: boolean;
  sound: NotificationSound;
  volume: number;
}

export type ActivityKind = 'needs-input' | 'task-complete' | 'exited';

export interface ActivityEvent {
  id: string;
  at: number;
  kind: ActivityKind;
  projectId: string;
  workerId: string;
  workerName: string;
  terminalId: string;
  terminalName: string;
  detail: string | null;
  read: boolean;
}

const PREFS_KEY = 'minnow.notifications.v1';
const EVENTS_KEY = 'minnow.activity.v1';
const MAX_EVENTS = 200;

export const DEFAULT_PREFS: NotificationPrefs = {
  enabled: true,
  agentTaskComplete: true,
  agentNeedsInput: true,
  terminalExited: true,
  suppressWhenFocused: true,
  sound: 'two-tone',
  volume: 0.5
};

function loadPrefs(): NotificationPrefs {
  try {
    const raw = JSON.parse(localStorage.getItem(PREFS_KEY) ?? 'null');
    return raw && typeof raw === 'object' ? { ...DEFAULT_PREFS, ...raw } : { ...DEFAULT_PREFS };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

function loadEvents(): ActivityEvent[] {
  try {
    const raw = JSON.parse(localStorage.getItem(EVENTS_KEY) ?? '[]');
    return Array.isArray(raw) ? raw.filter((item) => item && typeof item.id === 'string').slice(0, MAX_EVENTS) : [];
  } catch {
    return [];
  }
}

export const activity = $state<{ prefs: NotificationPrefs; events: ActivityEvent[] }>({
  prefs: loadPrefs(),
  events: loadEvents()
});

function persistEvents(): void {
  try {
    localStorage.setItem(EVENTS_KEY, JSON.stringify(activity.events));
  } catch {
    // The feed still works for this session.
  }
}

export function setNotificationPrefs(prefs: NotificationPrefs): void {
  activity.prefs = prefs;
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // Preferences still apply for this session.
  }
}

export function unreadCount(): number {
  return activity.events.filter((event) => !event.read).length;
}

export function markAllRead(): void {
  activity.events = activity.events.map((event) => (event.read ? event : { ...event, read: true }));
  persistEvents();
}

export function markRead(id: string): void {
  activity.events = activity.events.map((event) => (event.id === id ? { ...event, read: true } : event));
  persistEvents();
}

export function clearActivity(): void {
  activity.events = [];
  persistEvents();
}

let audio: AudioContext | null = null;

/** Short synthesized cues, so no sound files ship with the app. */
export function playNotificationSound(sound: NotificationSound, volume: number): void {
  if (sound === 'none' || volume <= 0) return;
  try {
    audio ??= new AudioContext();
    const context = audio;
    const tones: [number, number][] =
      sound === 'two-tone' ? [[660, 0], [880, 0.12]] : sound === 'ding' ? [[1046, 0]] : [[520, 0]];
    for (const [frequency, offset] of tones) {
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.type = 'sine';
      oscillator.frequency.value = frequency;
      const start = context.currentTime + offset;
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.exponentialRampToValueAtTime(Math.max(0.0002, volume * 0.3), start + 0.01);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + (sound === 'blip' ? 0.08 : 0.35));
      oscillator.connect(gain).connect(context.destination);
      oscillator.start(start);
      oscillator.stop(start + 0.4);
    }
  } catch {
    // Audio is optional.
  }
}

export function showSystemNotification(title: string, body: string): Promise<void> {
  return invoke<void>('show_notification', { title, body });
}

const KIND_TEXT: Record<ActivityKind, string> = {
  'needs-input': 'needs your input',
  'task-complete': 'finished its turn',
  exited: 'exited'
};

export function describeActivity(kind: ActivityKind): string {
  return KIND_TEXT[kind];
}

/**
 * Records a real status transition and, when the preferences allow it, raises a
 * system notification. Called with the previous and new terminal records.
 */
export function observeTerminal(previous: TerminalSession | undefined, next: TerminalSession, worker: Worker | undefined): void {
  if (!previous || !worker || next.closed) return;
  let kind: ActivityKind | null = null;
  let detail: string | null = null;
  if (next.status === 'waiting-for-input' && previous.status !== 'waiting-for-input') kind = 'needs-input';
  else if (next.kind === 'agent' && next.status === 'idle' && previous.status === 'running') kind = 'task-complete';
  else if (next.status === 'exited' && previous.status !== 'exited' && next.desired_running === false && previous.pid !== null) {
    // An explicit stop records desired_running=false before the exit; only report natural exits.
    if (previous.desired_running) {
      kind = 'exited';
      detail = next.exit_code === null ? null : `exit code ${next.exit_code}`;
    }
  }
  if (!kind) return;
  const event: ActivityEvent = {
    id: `${next.id}-${Date.now()}`,
    at: Date.now(),
    kind,
    projectId: worker.project_id,
    workerId: worker.id,
    workerName: worker.name,
    terminalId: next.id,
    terminalName: next.name,
    detail,
    read: false
  };
  activity.events = [event, ...activity.events].slice(0, MAX_EVENTS);
  persistEvents();

  const prefs = activity.prefs;
  const wanted =
    prefs.enabled &&
    ((kind === 'needs-input' && prefs.agentNeedsInput) ||
      (kind === 'task-complete' && prefs.agentTaskComplete) ||
      (kind === 'exited' && prefs.terminalExited));
  if (!wanted) return;
  if (prefs.suppressWhenFocused && document.hasFocus()) return;
  playNotificationSound(prefs.sound, prefs.volume);
  void showSystemNotification(`${worker.name} · ${next.name}`, `${next.name} ${KIND_TEXT[kind]}${detail ? ` (${detail})` : ''}.`).catch(() => {});
}
