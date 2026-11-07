import { invoke } from '@tauri-apps/api/core';

export interface SessionUsage { input: number; output: number; cache_read: number; cache_write: number; cost: number }
export interface SessionSummary {
  path: string;
  worker_id: string;
  terminal_id: string;
  terminal_name: string;
  title: string;
  started_at: string;
  updated_at: string;
  cwd: string;
  messages: number;
  user_messages: number;
  preview: string;
  models: string[];
  usage: SessionUsage;
}
export interface SessionMessage { role: string; text: string; timestamp: string; model: string | null; truncated: boolean }
export interface SessionHit { path: string; role: string; snippet: string }

export function listAgentSessions(): Promise<SessionSummary[]> {
  return invoke('list_agent_sessions');
}
export function readAgentSession(path: string): Promise<SessionMessage[]> {
  return invoke('read_agent_session', { path });
}
export function searchAgentSessions(query: string): Promise<SessionHit[]> {
  return invoke('search_agent_sessions', { query });
}
