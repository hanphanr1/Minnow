export type WorkerStatus = 'running' | 'idle' | 'waiting-for-input' | 'exited';
export type TerminalKind = 'agent' | 'shell';
export interface Settings { omp_path: string; default_worker_count: number }
export interface Project { id: string; name: string; path: string; base_ref: string }
export interface Worker {
  id: string;
  project_id: string;
  name: string;
  path: string;
  branch: string;
  base_ref: string;
  base_commit: string;
  session_path: string;
  status: WorkerStatus;
  pid: number | null;
  exit_code: number | null;
  last_error: string | null;
  desired_running: boolean;
}
/** One independently owned process inside a worktree. */
export interface TerminalSession {
  id: string;
  worker_id: string;
  name: string;
  kind: TerminalKind;
  cwd: string;
  current_cwd: string | null;
  session_path: string;
  status: WorkerStatus;
  pid: number | null;
  exit_code: number | null;
  last_error: string | null;
  desired_running: boolean;
  closed: boolean;
}
export interface Snapshot {
  projects: Project[];
  workers: Worker[];
  settings: Settings;
  terminals: TerminalSession[];
}
export interface TerminalChunk { terminal_id: string; start: number; end: number; data: number[] }
export interface TerminalSnapshot { data: number[]; end: number }
