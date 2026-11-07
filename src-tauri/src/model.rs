//! Shared data model for Minnow.
//!
//! Field names and casing mirror the TypeScript types in `src/types.ts` so the
//! same values can cross the Tauri IPC boundary without adapters.

use serde::{Deserialize, Serialize};

/// Worker process is running and attached to a terminal.
pub const STATUS_RUNNING: &str = "running";
/// Worker process is alive but has produced no recent output.
pub const STATUS_IDLE: &str = "idle";
/// Worker process is blocked waiting for user input.
pub const STATUS_WAITING_FOR_INPUT: &str = "waiting-for-input";
/// Worker process is not running.
pub const STATUS_EXITED: &str = "exited";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub omp_path: String,
    pub default_worker_count: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            omp_path: "omp".to_string(),
            default_worker_count: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub base_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worker {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub path: String,
    pub branch: String,
    pub base_ref: String,
    pub base_commit: String,
    pub session_path: String,
    pub status: String,
    pub pid: Option<u32>,
    pub exit_code: Option<u32>,
    pub last_error: Option<String>,
    pub desired_running: bool,
}

pub const KIND_AGENT: &str = "agent";
pub const KIND_SHELL: &str = "shell";

/// One independently owned process inside a worktree (`Worker`).
///
/// Field names mirror `TerminalSession` in `src/types.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSession {
    pub id: String,
    pub worker_id: String,
    pub name: String,
    /// `agent` (omp) or `shell` (a real local shell).
    pub kind: String,
    /// Initial launch directory.
    pub cwd: String,
    /// Directory last reported by live OSC 7 output; reset on every launch.
    pub current_cwd: Option<String>,
    pub session_path: String,
    pub status: String,
    pub pid: Option<u32>,
    pub exit_code: Option<u32>,
    pub last_error: Option<String>,
    pub desired_running: bool,
    pub closed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub projects: Vec<Project>,
    pub workers: Vec<Worker>,
    pub settings: Settings,
    pub terminals: Vec<TerminalSession>,
}
