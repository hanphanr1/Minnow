export type WorkerStatus = 'running' | 'idle' | 'waiting-for-input' | 'exited';
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
export interface Snapshot { projects: Project[]; workers: Worker[]; settings: Settings }
export interface TerminalChunk { worker_id: string; start: number; end: number; data: number[] }
export interface TerminalSnapshot { data: number[]; end: number }
