//! SQLite persistence for projects, workers, and settings.
//!
//! The database stores durable state only. Runtime facts such as the terminal
//! status, pid, and exit code intentionally live in memory: after a restart no
//! child process is running, so a loaded terminal always begins as `exited` with
//! no pid or exit code. `desired_running` and `last_error` are persisted per
//! terminal so the manager can decide what to resume and why one stopped.
//!
//! A worker is the Git worktree identity. Its legacy `desired_running` and
//! `last_error` columns are only migration input for its primary terminal; they
//! are never written again after the migration.

use crate::model::{Project, Settings, TerminalSession, Worker, STATUS_EXITED};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    omp_path TEXT NOT NULL,
    default_worker_count INTEGER NOT NULL
);

INSERT OR IGNORE INTO settings (id, omp_path, default_worker_count)
VALUES (1, 'omp', 1);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    base_ref TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workers (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    branch TEXT NOT NULL,
    base_ref TEXT NOT NULL,
    base_commit TEXT NOT NULL,
    session_path TEXT NOT NULL,
    desired_running INTEGER NOT NULL DEFAULT 1,
    last_error TEXT,
    UNIQUE (project_id, name)
);

CREATE TABLE IF NOT EXISTS terminal_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    worker_id TEXT NOT NULL REFERENCES workers(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('agent', 'shell')),
    cwd TEXT NOT NULL,
    session_path TEXT NOT NULL,
    desired_running INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    closed INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS terminal_sessions_worker ON terminal_sessions(worker_id);
"#;

/// Gives every worker that has no terminal record yet one primary agent
/// terminal whose id equals the worker id and which keeps the worker's old
/// session directory, resume flag and last error.
///
/// A worker with any terminal record, including a closed one, is already
/// migrated, so rerunning this never invents a second primary.
const MIGRATE_PRIMARY_TERMINALS: &str = r#"
INSERT INTO terminal_sessions (id, worker_id, name, kind, cwd, session_path, desired_running, last_error, closed)
SELECT w.id, w.id, w.name, 'agent', w.path, w.session_path, w.desired_running, w.last_error, 0
FROM workers w
WHERE NOT EXISTS (SELECT 1 FROM terminal_sessions t WHERE t.worker_id = w.id);
"#;

/// Thread-safe handle to the application database.
pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    /// Opens (and, when needed, creates) the database at `path`.
    ///
    /// Missing parent directories are created. Foreign keys, WAL mode, and a
    /// short busy timeout are enabled for every connection this store owns.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "cannot create database directory {}: {error}",
                        parent.display()
                    )
                })?;
            }
        }

        let connection = Connection::open(path)
            .map_err(|error| format!("cannot open database {}: {error}", path.display()))?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA synchronous = NORMAL;",
            )
            .map_err(|error| format!("cannot configure database {}: {error}", path.display()))?;
        connection
            .execute_batch(SCHEMA)
            .map_err(|error| format!("cannot initialize database schema: {error}"))?;
        // One transaction: a crash can never leave only some workers migrated.
        connection
            .execute_batch(&format!("BEGIN IMMEDIATE;
{MIGRATE_PRIMARY_TERMINALS}
COMMIT;"))
            .map_err(|error| format!("cannot migrate terminal sessions: {error}"))?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.connection
            .lock()
            .map_err(|_| "database lock is poisoned".to_string())
    }

    pub fn settings(&self) -> Result<Settings, String> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare("SELECT omp_path, default_worker_count FROM settings WHERE id = 1")
            .map_err(|error| database_error("cannot read settings", error))?;
        let row = statement
            .query_row([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                ))
            })
            .optional()
            .map_err(|error| database_error("cannot read settings", error))?;

        Ok(match row {
            Some((omp_path, count)) => Settings {
                omp_path,
                default_worker_count: u32::try_from(count.max(0)).unwrap_or(u32::MAX),
            },
            None => Settings::default(),
        })
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO settings (id, omp_path, default_worker_count)
                 VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET
                     omp_path = excluded.omp_path,
                     default_worker_count = excluded.default_worker_count",
                params![settings.omp_path, i64::from(settings.default_worker_count)],
            )
            .map_err(|error| database_error("cannot save settings", error))?;
        Ok(())
    }

    pub fn projects(&self) -> Result<Vec<Project>, String> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare("SELECT id, name, path, base_ref FROM projects ORDER BY rowid")
            .map_err(|error| database_error("cannot read projects", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    base_ref: row.get(3)?,
                })
            })
            .map_err(|error| database_error("cannot read projects", error))?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(row.map_err(|error| database_error("cannot read project", error))?);
        }
        Ok(projects)
    }

    pub fn workers(&self) -> Result<Vec<Worker>, String> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, name, path, branch, base_ref, base_commit,
                        session_path, desired_running, last_error
                 FROM workers ORDER BY rowid",
            )
            .map_err(|error| database_error("cannot read workers", error))?;
        let rows = statement
            .query_map([], worker_from_row)
            .map_err(|error| database_error("cannot read workers", error))?;

        let mut workers = Vec::new();
        for row in rows {
            workers.push(row.map_err(|error| database_error("cannot read worker", error))?);
        }
        Ok(workers)
    }

    pub fn project(&self, id: &str) -> Result<Project, String> {
        let connection = self.lock()?;
        let project = connection
            .query_row(
                "SELECT id, name, path, base_ref FROM projects WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                        base_ref: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|error| database_error("cannot read project", error))?;
        project.ok_or_else(|| format!("project {id} was not found"))
    }


    pub fn save_project(&self, project: &Project) -> Result<(), String> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO projects (id, name, path, base_ref)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                     name = excluded.name,
                     path = excluded.path,
                     base_ref = excluded.base_ref",
                params![project.id, project.name, project.path, project.base_ref],
            )
            .map_err(|error| database_error("cannot save project", error))?;
        Ok(())
    }

    /// Saves the worktree identity. Runtime and resume state live on its
    /// terminals; the legacy worker columns are deliberately not rewritten.
    pub fn save_worker(&self, worker: &Worker) -> Result<(), String> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO workers (id, project_id, name, path, branch, base_ref,
                                      base_commit, session_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                     project_id = excluded.project_id,
                     name = excluded.name,
                     path = excluded.path,
                     branch = excluded.branch,
                     base_ref = excluded.base_ref,
                     base_commit = excluded.base_commit,
                     session_path = excluded.session_path",
                params![
                    worker.id,
                    worker.project_id,
                    worker.name,
                    worker.path,
                    worker.branch,
                    worker.base_ref,
                    worker.base_commit,
                    worker.session_path,
                ],
            )
            .map_err(|error| database_error("cannot save worker", error))?;
        Ok(())
    }

    /// Terminal records in creation order. Runtime fields start as `exited`.
    pub fn terminals(&self) -> Result<Vec<TerminalSession>, String> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, worker_id, name, kind, cwd, session_path,
                        desired_running, last_error, closed
                 FROM terminal_sessions ORDER BY rowid",
            )
            .map_err(|error| database_error("cannot read terminals", error))?;
        let rows = statement
            .query_map([], terminal_from_row)
            .map_err(|error| database_error("cannot read terminals", error))?;
        let mut terminals = Vec::new();
        for row in rows {
            terminals.push(row.map_err(|error| database_error("cannot read terminal", error))?);
        }
        Ok(terminals)
    }

    pub fn save_terminal(&self, terminal: &TerminalSession) -> Result<(), String> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO terminal_sessions (id, worker_id, name, kind, cwd, session_path,
                                                desired_running, last_error, closed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     name = excluded.name,
                     cwd = excluded.cwd,
                     session_path = excluded.session_path,
                     desired_running = excluded.desired_running,
                     last_error = excluded.last_error,
                     closed = excluded.closed",
                params![
                    terminal.id,
                    terminal.worker_id,
                    terminal.name,
                    terminal.kind,
                    terminal.cwd,
                    terminal.session_path,
                    i64::from(terminal.desired_running),
                    terminal.last_error,
                    i64::from(terminal.closed),
                ],
            )
            .map_err(|error| database_error("cannot save terminal", error))?;
        Ok(())
    }

    pub fn delete_worker(&self, id: &str) -> Result<(), String> {
        let connection = self.lock()?;
        connection
            .execute("DELETE FROM workers WHERE id = ?1", params![id])
            .map_err(|error| database_error("cannot delete worker", error))?;
        Ok(())
    }
}

fn worker_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Worker> {
    Ok(Worker {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        path: row.get(3)?,
        branch: row.get(4)?,
        base_ref: row.get(5)?,
        base_commit: row.get(6)?,
        session_path: row.get(7)?,
        status: STATUS_EXITED.to_string(),
        pid: None,
        exit_code: None,
        last_error: row.get(9)?,
        desired_running: row.get::<_, i64>(8)? != 0,
    })
}

fn terminal_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TerminalSession> {
    Ok(TerminalSession {
        id: row.get(0)?,
        worker_id: row.get(1)?,
        name: row.get(2)?,
        kind: row.get(3)?,
        cwd: row.get(4)?,
        current_cwd: None,
        session_path: row.get(5)?,
        status: STATUS_EXITED.to_string(),
        pid: None,
        exit_code: None,
        last_error: row.get(7)?,
        desired_running: row.get::<_, i64>(6)? != 0,
        closed: row.get::<_, i64>(8)? != 0,
    })
}

fn database_error(context: &str, error: rusqlite::Error) -> String {
    if let rusqlite::Error::SqliteFailure(_, Some(message)) = &error {
        if message.contains("FOREIGN KEY") {
            return format!("{context}: the referenced project does not exist");
        }
        if message.contains("UNIQUE") {
            return format!("{context}: a worker with this name already exists in this project");
        }
    }
    format!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::STATUS_RUNNING;
    use tempfile::TempDir;

    fn temporary_store() -> (TempDir, Store, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("nested").join("workspace.sqlite");
        let store = Store::open(&path).expect("open store");
        (directory, store, path)
    }

    fn project(id: &str, name: &str) -> Project {
        Project {
            id: id.to_string(),
            name: name.to_string(),
            path: format!(r"C:\repos\{name}"),
            base_ref: "main".to_string(),
        }
    }

    fn worker(id: &str, project_id: &str, name: &str) -> Worker {
        Worker {
            id: id.to_string(),
            project_id: project_id.to_string(),
            name: name.to_string(),
            path: format!(r"C:\repos\demo\.minnow\worktrees\{name}"),
            branch: format!("minnow/{name}-{id}"),
            base_ref: "main".to_string(),
            base_commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            session_path: format!(r"C:\repos\demo\.minnow\sessions\{id}"),
            status: STATUS_RUNNING.to_string(),
            pid: Some(4242),
            exit_code: Some(3),
            last_error: Some("previous failure".to_string()),
            desired_running: true,
        }
    }

    #[test]
    fn settings_persist_across_reopen() {
        let (_directory, store, path) = temporary_store();
        let settings = Settings {
            omp_path: r"C:\tools\omp.exe".to_string(),
            default_worker_count: 4,
        };
        store.save_settings(&settings).unwrap();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.settings().unwrap(), settings);
    }

    #[test]
    fn projects_and_workers_persist_identity() {
        let (_directory, store, path) = temporary_store();
        let project = project("p1", "demo");
        let worker = worker("w1", "p1", "alpha");

        store.save_project(&project).unwrap();
        store.save_worker(&worker).unwrap();

        drop(store);
        let reopened = Store::open(&path).unwrap();
        let loaded = reopened.workers().unwrap().into_iter().find(|w| w.id == "w1").unwrap();
        assert_eq!(loaded.status, STATUS_EXITED);
        assert_eq!(loaded.pid, None);
        assert_eq!(loaded.branch, worker.branch);
        assert_eq!(loaded.base_commit, worker.base_commit);
        assert_eq!(loaded.session_path, worker.session_path);
        assert_eq!(reopened.projects().unwrap(), vec![project]);
    }

    fn terminal(id: &str, worker_id: &str) -> TerminalSession {
        TerminalSession {
            id: id.to_string(),
            worker_id: worker_id.to_string(),
            name: format!("Terminal {id}"),
            kind: "shell".to_string(),
            cwd: r"C:\repos\demo".to_string(),
            current_cwd: Some(r"C:\runtime".to_string()),
            session_path: format!(r"C:\sessions\{id}"),
            status: STATUS_RUNNING.to_string(),
            pid: Some(7),
            exit_code: Some(1),
            last_error: Some("boom".to_string()),
            desired_running: true,
            closed: false,
        }
    }

    #[test]
    fn terminal_runtime_state_resets_but_resume_state_persists() {
        let (_directory, store, path) = temporary_store();
        store.save_project(&project("p1", "demo")).unwrap();
        store.save_worker(&worker("w1", "p1", "alpha")).unwrap();
        let mut extra = terminal("t2", "w1");
        extra.closed = true;
        extra.desired_running = false;
        store.save_terminal(&terminal("t1", "w1")).unwrap();
        store.save_terminal(&extra).unwrap();
        drop(store);

        let reopened = Store::open(&path).unwrap();
        let terminals = reopened.terminals().unwrap();
        // Migration runs at open time, so a terminal saved later stays in creation order.
        assert_eq!(
            terminals.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["t1", "t2"]
        );
        let loaded = &terminals[0];
        assert_eq!(loaded.status, STATUS_EXITED);
        assert_eq!(loaded.pid, None);
        assert_eq!(loaded.exit_code, None);
        assert_eq!(loaded.current_cwd, None);
        assert_eq!(loaded.last_error.as_deref(), Some("boom"));
        assert!(loaded.desired_running);
        assert!(terminals[1].closed);
    }

    #[test]
    fn legacy_workers_migrate_once_to_a_primary_agent_terminal() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.sqlite");
        {
            // Shape of the verified pre-terminal database.
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    r#"PRAGMA foreign_keys = ON;
                     CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 1), omp_path TEXT NOT NULL, default_worker_count INTEGER NOT NULL);
                     CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, path TEXT NOT NULL, base_ref TEXT NOT NULL);
                     CREATE TABLE workers (
                         id TEXT PRIMARY KEY NOT NULL,
                         project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                         name TEXT NOT NULL, path TEXT NOT NULL, branch TEXT NOT NULL,
                         base_ref TEXT NOT NULL, base_commit TEXT NOT NULL, session_path TEXT NOT NULL,
                         desired_running INTEGER NOT NULL DEFAULT 1, last_error TEXT,
                         UNIQUE (project_id, name));
                     INSERT INTO projects VALUES ('p1', 'demo', 'C:\repos\demo', 'main');
                     INSERT INTO workers VALUES ('w1', 'p1', 'alpha', 'C:\wt\alpha', 'b1', 'main', 'abc', 'C:\sessions\w1', 1, NULL);
                     INSERT INTO workers VALUES ('w2', 'p1', 'beta', 'C:\wt\beta', 'b2', 'main', 'abc', 'C:\sessions\w2', 0, 'old failure');"#,
                )
                .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let first = store.terminals().unwrap();
        assert_eq!(first.len(), 2);
        let alpha = first.iter().find(|t| t.id == "w1").unwrap();
        assert_eq!(alpha.worker_id, "w1");
        assert_eq!(alpha.kind, "agent");
        assert_eq!(alpha.name, "alpha");
        assert_eq!(alpha.cwd, r"C:\wt\alpha");
        assert_eq!(alpha.session_path, r"C:\sessions\w1");
        assert!(alpha.desired_running && !alpha.closed);
        let beta = first.iter().find(|t| t.id == "w2").unwrap();
        assert!(!beta.desired_running);
        assert_eq!(beta.last_error.as_deref(), Some("old failure"));

        // A different terminal id marks the worker migrated; reopening adds nothing.
        let mut closed = terminal("t9", "w1");
        closed.closed = true;
        store.save_terminal(&closed).unwrap();
        drop(store);
        let again = Store::open(&path).unwrap();
        assert_eq!(again.terminals().unwrap().len(), 3);
    }

    #[test]
    fn saving_a_worker_does_not_rewrite_terminal_state() {
        let (_directory, store, _path) = temporary_store();
        store.save_project(&project("p1", "demo")).unwrap();
        let mut value = worker("w1", "p1", "alpha");
        store.save_worker(&value).unwrap();
        store.save_terminal(&terminal("w1", "w1")).unwrap();
        value.desired_running = false;
        value.last_error = None;
        store.save_worker(&value).unwrap();
        let primary = store.terminals().unwrap().into_iter().find(|t| t.id == "w1").unwrap();
        assert!(primary.desired_running);
        assert_eq!(primary.last_error.as_deref(), Some("boom"));
    }

    #[test]
    fn duplicate_worker_name_in_project_is_refused() {
        let (_directory, store, _path) = temporary_store();
        let first_project = project("p1", "one");
        let second_project = project("p2", "two");
        store.save_project(&first_project).unwrap();
        store.save_project(&second_project).unwrap();

        store.save_worker(&worker("w1", "p1", "same")).unwrap();
        let error = store.save_worker(&worker("w2", "p1", "same")).unwrap_err();
        assert!(error.contains("already exists"), "{error}");

        // The same name is fine in a different project.
        store.save_worker(&worker("w3", "p2", "same")).unwrap();

        let orphan = worker("w4", "missing-project", "gamma");
        let error = store.save_worker(&orphan).unwrap_err();
        assert!(error.contains("does not exist"), "{error}");
    }

    #[test]
    fn delete_worker_removes_the_record_and_its_terminals() {
        let (_directory, store, _path) = temporary_store();
        store.save_project(&project("p1", "demo")).unwrap();
        store.save_worker(&worker("w1", "p1", "alpha")).unwrap();
        store.save_terminal(&terminal("t1", "w1")).unwrap();

        assert_eq!(store.workers().unwrap().len(), 1);
        store.delete_worker("w1").unwrap();
        assert!(store.workers().unwrap().is_empty());
        assert!(store.terminals().unwrap().is_empty());
    }
}
