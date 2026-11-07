import { invoke } from '@tauri-apps/api/core';
import type { Project } from './types';

export interface DirectoryEntry {
  name: string;
  path: string;
  is_dir: boolean;
  is_symlink: boolean;
  size: number;
}
export interface DirectoryListing { entries: DirectoryEntry[]; truncated: boolean }
export interface FileDocument { path: string; content: string; revision: string }
export interface SearchMatch { path: string; line: number; column: number; text: string }
export interface SearchResults { matches: SearchMatch[]; truncated: boolean }
export interface FileList { paths: string[]; truncated: boolean }
export interface GitFileChange { path: string; old_path: string | null; status: string }
export interface GitStatus {
  branch: string;
  staged: GitFileChange[];
  unstaged: GitFileChange[];
  untracked: GitFileChange[];
}
export interface GitCommit { id: string; short_id: string; subject: string; author: string; timestamp: number }

export function listWorkspaceDirectory(workerId: string, path = ''): Promise<DirectoryListing> {
  return invoke('list_workspace_directory', { workerId, path });
}
export function readWorkspaceFile(workerId: string, path: string): Promise<FileDocument> {
  return invoke('read_workspace_file', { workerId, path });
}
export function writeWorkspaceFile(workerId: string, path: string, content: string, revision: string): Promise<FileDocument> {
  return invoke('write_workspace_file', { workerId, path, content, revision });
}
export function searchWorkspace(workerId: string, query: string, caseSensitive = false): Promise<SearchResults> {
  return invoke('search_workspace', { workerId, query, caseSensitive });
}
/** Workspace-relative files whose path contains every whitespace-separated token of `query`. */
export function findWorkspaceFiles(workerId: string, query: string, includeDotfiles: boolean): Promise<FileList> {
  return invoke('find_workspace_files', { workerId, query, includeDotfiles });
}
/** The subset of `paths` that Git ignores. */
export function getWorkspaceGitIgnored(workerId: string, paths: string[]): Promise<string[]> {
  return invoke('workspace_git_ignored', { workerId, paths });
}
/** Creates an empty file or folder; never overwrites. Returns the workspace-relative path. */
export function createWorkspaceEntry(workerId: string, path: string, isDir: boolean): Promise<string> {
  return invoke('create_workspace_entry', { workerId, path, isDir });
}
export function renameWorkspaceEntry(workerId: string, from: string, to: string): Promise<string> {
  return invoke('rename_workspace_entry', { workerId, from, to });
}
/** Moves the entry to the system trash. */
export function deleteWorkspaceEntry(workerId: string, path: string): Promise<void> {
  return invoke('delete_workspace_entry', { workerId, path });
}
export function revealWorkspaceEntry(workerId: string, path: string): Promise<void> {
  return invoke('reveal_workspace_entry', { workerId, path });
}
export function getWorkspaceGitStatus(workerId: string): Promise<GitStatus> {
  return invoke('workspace_git_status', { workerId });
}
export function getWorkspaceHistory(workerId: string, limit = 30): Promise<GitCommit[]> {
  return invoke('workspace_git_history', { workerId, limit });
}
export function stageWorkspacePaths(workerId: string, paths: string[] | null): Promise<void> {
  return invoke('stage_workspace_paths', { workerId, paths });
}
export function unstageWorkspacePaths(workerId: string, paths: string[] | null): Promise<void> {
  return invoke('unstage_workspace_paths', { workerId, paths });
}
/** Restores `tracked` from the index and trashes `untracked`; staged work is kept. */
export function discardWorkspacePaths(workerId: string, tracked: string[], untracked: string[]): Promise<void> {
  return invoke('discard_workspace_paths', { workerId, tracked, untracked });
}
export function commitWorkspace(workerId: string, message: string): Promise<string> {
  return invoke('commit_workspace', { workerId, message });
}
export function getWorkspaceFileDiff(workerId: string, path: string, staged = false): Promise<string> {
  return invoke('workspace_file_diff', { workerId, path, staged });
}

// A shared invalidation event follows an actual write or Git action, never a timer.
export const workspaceChangedEvent = 'minnow-workspace-changed';
export function announceWorkspaceChange(workerId: string): void {
  window.dispatchEvent(new CustomEvent(workspaceChangedEvent, { detail: workerId }));
}

// A rename or delete announces which path moved so open editors can follow or close.
export const workspacePathMovedEvent = 'minnow-workspace-path-moved';
export interface PathMoved { workerId: string; from: string; to: string | null }
export function announcePathMoved(detail: PathMoved): void {
  window.dispatchEvent(new CustomEvent(workspacePathMovedEvent, { detail }));
}

export function pickProjectFolder(): Promise<string | null> {
  return invoke('pick_project_folder');
}
export function createProject(parentPath: string, name: string): Promise<Project> {
  return invoke('create_project', { parentPath, name });
}
export function cloneProject(parentPath: string, name: string, url: string): Promise<Project> {
  return invoke('clone_project', { parentPath, name, url });
}
