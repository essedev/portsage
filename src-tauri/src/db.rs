use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use crate::paths;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: Option<String>,
    pub range_start: i64,
    pub range_end: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Port {
    pub id: i64,
    pub project_id: i64,
    pub service: String,
    pub port: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectWithPorts {
    #[serde(flatten)]
    pub project: Project,
    pub ports: Vec<Port>,
}

// The on-disk and wire shape are identical, so re-export the canonical wire
// type from portsage-client instead of duplicating it. The Mac stores one
// row per remote (e.g. "dev", "staging"); the SSH config and tunnel state
// live elsewhere - this is just the catalogue.
pub use portsage_client::RemoteBackend;

/// Fields a caller supplies when creating or updating a remote backend.
/// Wrapping the inputs keeps the CRUD method signatures readable when more
/// optional fields are added later.
#[derive(Debug, Clone)]
pub struct RemoteBackendInput<'a> {
    pub name: &'a str,
    pub ssh_alias: &'a str,
    pub remote_socket_path: &'a str,
    pub local_socket_path: &'a str,
    pub auto_forward_enabled: bool,
}

/// A port the user has explicitly blocked from auto-forwarding for a given
/// remote backend. Phase 3 of the multi-host evolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForwardExclusion {
    pub id: i64,
    pub backend_id: i64,
    pub port: i64,
    pub created_at: String,
}

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = Self::db_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&db_path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Lock the inner connection, recovering from mutex poisoning.
    ///
    /// Poisoning only happens if some other thread panicked while holding
    /// this lock. SQLite statements are atomic and our closures keep no
    /// in-memory invariants across calls, so the on-disk state is always
    /// consistent even after such a panic. Recovering with `into_inner`
    /// lets the app keep running instead of cascading the panic to every
    /// future DB caller.
    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn db_path() -> PathBuf {
        paths::db_path()
    }

    /// Replace the open connection with a fresh one against the same
    /// on-disk path. Called after an `import_data` so the running app
    /// observes the new database without a restart.
    ///
    /// Holds the same mutex other readers use, so no concurrent query can
    /// see a half-replaced state.
    pub fn reopen(&self) -> Result<()> {
        let mut guard = self.conn();
        let new_conn = Connection::open(Self::db_path())?;
        *guard = new_conn;
        Ok(())
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                path TEXT,
                range_start INTEGER NOT NULL,
                range_end INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS ports (
                id INTEGER PRIMARY KEY,
                project_id INTEGER NOT NULL REFERENCES projects(id),
                service TEXT NOT NULL,
                port INTEGER NOT NULL UNIQUE,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            INSERT OR IGNORE INTO config (key, value) VALUES ('base_port', '4000');
            INSERT OR IGNORE INTO config (key, value) VALUES ('range_size', '10');

            -- Phase 2 of the multi-host evolution: catalogue of remote
            -- backends the Mac UI knows about. This table is meaningful only
            -- on the Mac; on a Linux server it stays empty (the server is
            -- itself a remote backend, not a consumer of remotes).
            CREATE TABLE IF NOT EXISTS remote_backends (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                ssh_alias TEXT NOT NULL,
                remote_socket_path TEXT NOT NULL,
                local_socket_path TEXT NOT NULL,
                auto_forward_enabled INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- Phase 3 of the multi-host evolution: per-backend blocklist of
            -- ports the user does not want auto-forwarded (e.g. port 4060 is
            -- in use locally by something else and they don't want Portsage
            -- to fight it). One row per excluded port; uniqueness is on
            -- (backend_id, port). Foreign key is informational - the cleanup
            -- on backend removal happens in code so we keep error reporting
            -- consistent with the rest of the CRUD path.
            CREATE TABLE IF NOT EXISTS forward_exclusions (
                id INTEGER PRIMARY KEY,
                backend_id INTEGER NOT NULL REFERENCES remote_backends(id),
                port INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(backend_id, port)
            );",
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_config(&self, key: &str) -> Result<String> {
        let conn = self.conn();
        conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
    }

    #[allow(dead_code)]
    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn next_available_range(&self) -> Result<(i64, i64)> {
        let conn = self.conn();
        Self::compute_next_range(&conn)
    }

    /// Pure helper that computes the next free range using the supplied
    /// connection, without acquiring the mutex. Lets `create_project` perform
    /// "compute range + insert" atomically under a single lock, preventing the
    /// race where two concurrent callers would otherwise read the same
    /// `MAX(range_end)` and produce overlapping projects.
    fn compute_next_range(conn: &Connection) -> Result<(i64, i64)> {
        let base_port: i64 = conn
            .query_row(
                "SELECT value FROM config WHERE key = 'base_port'",
                [],
                |row| row.get::<_, String>(0),
            )?
            .parse()
            .unwrap_or(4000);
        let range_size: i64 = conn
            .query_row(
                "SELECT value FROM config WHERE key = 'range_size'",
                [],
                |row| row.get::<_, String>(0),
            )?
            .parse()
            .unwrap_or(10);

        let max_end: Option<i64> =
            conn.query_row("SELECT MAX(range_end) FROM projects", [], |row| row.get(0))?;

        let start = match max_end {
            Some(end) => end + 1,
            None => base_port,
        };
        Ok((start, start + range_size - 1))
    }

    pub fn create_project(&self, name: &str, path: Option<&str>) -> Result<Project> {
        let conn = self.conn();
        let (range_start, range_end) = Self::compute_next_range(&conn)?;
        conn.execute(
            "INSERT INTO projects (name, path, range_start, range_end) \
             VALUES (?1, ?2, ?3, ?4)",
            params![name, path, range_start, range_end],
        )?;
        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, name, path, range_start, range_end, created_at \
             FROM projects WHERE id = ?1",
            params![id],
            |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    range_start: row.get(3)?,
                    range_end: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
    }

    pub fn delete_project(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM ports WHERE project_id = ?1", params![id])?;
        conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectWithPorts>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, path, range_start, range_end, created_at \
             FROM projects ORDER BY range_start",
        )?;
        let projects: Vec<Project> = stmt
            .query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    range_start: row.get(3)?,
                    range_end: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        let mut result = Vec::new();
        for project in projects {
            let mut port_stmt = conn.prepare(
                "SELECT id, project_id, service, port, created_at \
                 FROM ports WHERE project_id = ?1 ORDER BY port",
            )?;
            let ports: Vec<Port> = port_stmt
                .query_map(params![project.id], |row| {
                    Ok(Port {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        service: row.get(2)?,
                        port: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;
            result.push(ProjectWithPorts { project, ports });
        }
        Ok(result)
    }

    pub fn add_port(&self, project_id: i64, service: &str, port: i64) -> Result<Port> {
        let conn = self.conn();
        // Validate the port is within the project's reserved range.
        let (range_start, range_end): (i64, i64) = conn.query_row(
            "SELECT range_start, range_end FROM projects WHERE id = ?1",
            params![project_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if port < range_start || port > range_end {
            // Use SqliteFailure with a custom message: its Display impl prints only the
            // message (no "Invalid parameter name:" or other rusqlite-specific prefix),
            // so the error surfaces cleanly through the MCP socket and Tauri command layer.
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                Some(format!(
                    "port {} is outside project range {}-{}",
                    port, range_start, range_end
                )),
            ));
        }
        conn.execute(
            "INSERT INTO ports (project_id, service, port) VALUES (?1, ?2, ?3)",
            params![project_id, service, port],
        )?;
        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, project_id, service, port, created_at \
             FROM ports WHERE id = ?1",
            params![id],
            |row| {
                Ok(Port {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    service: row.get(2)?,
                    port: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )
    }

    pub fn remove_port(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM ports WHERE id = ?1", params![id])?;
        Ok(())
    }

    // --- remote_backends ---

    pub fn create_remote_backend(&self, input: RemoteBackendInput<'_>) -> Result<RemoteBackend> {
        validate_remote_backend_input(&input)?;
        let conn = self.conn();
        conn.execute(
            "INSERT INTO remote_backends \
             (name, ssh_alias, remote_socket_path, local_socket_path, auto_forward_enabled) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                input.name,
                input.ssh_alias,
                input.remote_socket_path,
                input.local_socket_path,
                input.auto_forward_enabled as i64,
            ],
        )?;
        let id = conn.last_insert_rowid();
        Self::fetch_remote_backend(&conn, id)
    }

    pub fn list_remote_backends(&self) -> Result<Vec<RemoteBackend>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, ssh_alias, remote_socket_path, local_socket_path, \
                    auto_forward_enabled, created_at \
             FROM remote_backends ORDER BY name",
        )?;
        let rows: Vec<RemoteBackend> = stmt
            .query_map([], row_to_remote_backend)?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_remote_backend_by_name(&self, name: &str) -> Result<Option<RemoteBackend>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, ssh_alias, remote_socket_path, local_socket_path, \
                    auto_forward_enabled, created_at \
             FROM remote_backends WHERE name = ?1",
        )?;
        let mut rows = stmt.query(params![name])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_remote_backend(row)?)),
            None => Ok(None),
        }
    }

    pub fn update_remote_backend(
        &self,
        id: i64,
        input: RemoteBackendInput<'_>,
    ) -> Result<RemoteBackend> {
        validate_remote_backend_input(&input)?;
        let conn = self.conn();
        let changed = conn.execute(
            "UPDATE remote_backends SET \
                name = ?1, ssh_alias = ?2, remote_socket_path = ?3, \
                local_socket_path = ?4, auto_forward_enabled = ?5 \
             WHERE id = ?6",
            params![
                input.name,
                input.ssh_alias,
                input.remote_socket_path,
                input.local_socket_path,
                input.auto_forward_enabled as i64,
                id,
            ],
        )?;
        if changed == 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
                Some(format!("remote backend {id} not found")),
            ));
        }
        Self::fetch_remote_backend(&conn, id)
    }

    pub fn set_remote_backend_auto_forward(&self, id: i64, enabled: bool) -> Result<()> {
        let conn = self.conn();
        let changed = conn.execute(
            "UPDATE remote_backends SET auto_forward_enabled = ?1 WHERE id = ?2",
            params![enabled as i64, id],
        )?;
        if changed == 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTFOUND),
                Some(format!("remote backend {id} not found")),
            ));
        }
        Ok(())
    }

    pub fn delete_remote_backend(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        // Drop dependent rows first so the foreign key isn't orphaned. The
        // schema doesn't declare ON DELETE CASCADE - we run rusqlite without
        // foreign_keys=ON anyway - so the cascade is in code.
        conn.execute(
            "DELETE FROM forward_exclusions WHERE backend_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM remote_backends WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn fetch_remote_backend(conn: &Connection, id: i64) -> Result<RemoteBackend> {
        conn.query_row(
            "SELECT id, name, ssh_alias, remote_socket_path, local_socket_path, \
                    auto_forward_enabled, created_at \
             FROM remote_backends WHERE id = ?1",
            params![id],
            row_to_remote_backend,
        )
    }

    // --- forward_exclusions ---

    pub fn list_forward_exclusions(&self, backend_id: i64) -> Result<Vec<ForwardExclusion>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, backend_id, port, created_at \
             FROM forward_exclusions WHERE backend_id = ?1 ORDER BY port",
        )?;
        let rows: Vec<ForwardExclusion> = stmt
            .query_map(params![backend_id], row_to_forward_exclusion)?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn add_forward_exclusion(&self, backend_id: i64, port: i64) -> Result<ForwardExclusion> {
        if !(1..=65535).contains(&port) {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                Some(format!("port {port} is out of range (1-65535)")),
            ));
        }
        let conn = self.conn();
        conn.execute(
            "INSERT INTO forward_exclusions (backend_id, port) VALUES (?1, ?2)",
            params![backend_id, port],
        )?;
        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, backend_id, port, created_at \
             FROM forward_exclusions WHERE id = ?1",
            params![id],
            row_to_forward_exclusion,
        )
    }

    pub fn remove_forward_exclusion(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM forward_exclusions WHERE id = ?1", params![id])?;
        Ok(())
    }

}

fn row_to_forward_exclusion(row: &rusqlite::Row<'_>) -> Result<ForwardExclusion> {
    Ok(ForwardExclusion {
        id: row.get(0)?,
        backend_id: row.get(1)?,
        port: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn validate_remote_backend_input(input: &RemoteBackendInput<'_>) -> Result<()> {
    fn fail(msg: &str) -> Result<()> {
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some(msg.to_string()),
        ))
    }
    if input.name.trim().is_empty() {
        return fail("name is required");
    }
    if input.ssh_alias.trim().is_empty() {
        return fail("ssh_alias is required");
    }
    if input.remote_socket_path.trim().is_empty() {
        return fail("remote_socket_path is required");
    }
    if input.local_socket_path.trim().is_empty() {
        return fail("local_socket_path is required");
    }
    Ok(())
}

fn row_to_remote_backend(row: &rusqlite::Row<'_>) -> Result<RemoteBackend> {
    Ok(RemoteBackend {
        id: row.get(0)?,
        name: row.get(1)?,
        ssh_alias: row.get(2)?,
        remote_socket_path: row.get(3)?,
        local_socket_path: row.get(4)?,
        auto_forward_enabled: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Database {
        Database::in_memory().expect("failed to create in-memory db")
    }

    #[test]
    fn default_config_values() {
        let db = fresh_db();
        assert_eq!(db.get_config("base_port").unwrap(), "4000");
        assert_eq!(db.get_config("range_size").unwrap(), "10");
    }

    #[test]
    fn config_update() {
        let db = fresh_db();
        db.set_config("base_port", "5000").unwrap();
        assert_eq!(db.get_config("base_port").unwrap(), "5000");
        // range_size unchanged
        assert_eq!(db.get_config("range_size").unwrap(), "10");
    }

    #[test]
    fn first_project_starts_at_base_port() {
        let db = fresh_db();
        let p = db.create_project("alpha", None).unwrap();
        assert_eq!(p.range_start, 4000);
        assert_eq!(p.range_end, 4009);
    }

    #[test]
    fn sequential_ranges_do_not_overlap() {
        let db = fresh_db();
        let a = db.create_project("alpha", None).unwrap();
        let b = db.create_project("bravo", None).unwrap();
        let c = db.create_project("charlie", None).unwrap();
        assert_eq!(a.range_start, 4000);
        assert_eq!(b.range_start, 4010);
        assert_eq!(c.range_start, 4020);
        // No overlap
        assert!(a.range_end < b.range_start);
        assert!(b.range_end < c.range_start);
    }

    #[test]
    fn custom_config_affects_next_range() {
        let db = fresh_db();
        db.set_config("base_port", "8000").unwrap();
        db.set_config("range_size", "5").unwrap();
        let p = db.create_project("alpha", None).unwrap();
        assert_eq!(p.range_start, 8000);
        assert_eq!(p.range_end, 8004);
    }

    #[test]
    fn gap_after_delete_not_reused() {
        let db = fresh_db();
        let a = db.create_project("alpha", None).unwrap();
        let b = db.create_project("bravo", None).unwrap();
        db.delete_project(a.id).unwrap();
        // Next range continues after bravo, does not fill the gap
        let c = db.create_project("charlie", None).unwrap();
        assert_eq!(c.range_start, b.range_end + 1);
    }

    #[test]
    fn duplicate_project_name_fails() {
        let db = fresh_db();
        db.create_project("alpha", None).unwrap();
        let err = db.create_project("alpha", None);
        assert!(err.is_err());
    }

    #[test]
    fn duplicate_port_fails() {
        let db = fresh_db();
        let p = db.create_project("alpha", None).unwrap();
        db.add_port(p.id, "vite", 4000).unwrap();
        let err = db.add_port(p.id, "api", 4000);
        assert!(err.is_err());
    }

    #[test]
    fn port_outside_range_fails() {
        let db = fresh_db();
        let p = db.create_project("alpha", None).unwrap();
        // range is 4000-4009; 9999 is outside
        let err = db.add_port(p.id, "cache", 9999);
        assert!(err.is_err(), "expected error for port outside range");
        // boundaries should work
        db.add_port(p.id, "low", 4000).unwrap();
        db.add_port(p.id, "high", 4009).unwrap();
        // one past the end fails
        assert!(db.add_port(p.id, "off", 4010).is_err());
    }

    #[test]
    fn delete_project_cascades_ports() {
        let db = fresh_db();
        let p = db.create_project("alpha", None).unwrap();
        db.add_port(p.id, "vite", 4000).unwrap();
        db.add_port(p.id, "api", 4001).unwrap();
        db.delete_project(p.id).unwrap();
        let projects = db.list_projects().unwrap();
        assert!(projects.is_empty());
        // Ports also gone - can reuse them
        let q = db.create_project("bravo", None).unwrap();
        db.add_port(q.id, "vite", 4000).unwrap(); // would fail if not cascaded
    }

    #[test]
    fn list_projects_returns_with_ports() {
        let db = fresh_db();
        let p = db.create_project("alpha", Some("/tmp/alpha")).unwrap();
        db.add_port(p.id, "vite", 4000).unwrap();
        db.add_port(p.id, "api", 4001).unwrap();
        let projects = db.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project.name, "alpha");
        assert_eq!(projects[0].project.path, Some("/tmp/alpha".to_string()));
        assert_eq!(projects[0].ports.len(), 2);
        // Ports ordered by port number
        assert_eq!(projects[0].ports[0].port, 4000);
        assert_eq!(projects[0].ports[1].port, 4001);
    }

    #[test]
    fn remove_single_port() {
        let db = fresh_db();
        let p = db.create_project("alpha", None).unwrap();
        let port_a = db.add_port(p.id, "vite", 4000).unwrap();
        db.add_port(p.id, "api", 4001).unwrap();
        db.remove_port(port_a.id).unwrap();
        let projects = db.list_projects().unwrap();
        assert_eq!(projects[0].ports.len(), 1);
        assert_eq!(projects[0].ports[0].service, "api");
    }

    /// Regression test for the create_project race condition.
    ///
    /// Before the fix, `create_project` called `next_available_range()` (which
    /// took the mutex, computed MAX(range_end), released it) and then locked
    /// the mutex again to insert. Two threads could read the same MAX value
    /// concurrently and produce overlapping ranges. After the fix, range
    /// computation and insert happen under a single lock.
    ///
    /// This test spawns N threads that all create a project simultaneously
    /// and asserts that the resulting ranges are unique and non-overlapping.
    #[test]
    fn concurrent_create_project_produces_no_overlapping_ranges() {
        use std::sync::Arc;
        use std::thread;

        const THREADS: usize = 16;

        let db = Arc::new(fresh_db());
        let mut handles = Vec::with_capacity(THREADS);

        for i in 0..THREADS {
            let db = db.clone();
            handles.push(thread::spawn(move || {
                let name = format!("project-{}", i);
                db.create_project(&name, None).unwrap()
            }));
        }

        let mut projects: Vec<Project> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        projects.sort_by_key(|p| p.range_start);

        // All starts must be unique.
        let starts: std::collections::HashSet<i64> =
            projects.iter().map(|p| p.range_start).collect();
        assert_eq!(
            starts.len(),
            THREADS,
            "duplicate range_start across concurrent inserts: {:?}",
            projects.iter().map(|p| p.range_start).collect::<Vec<_>>()
        );

        // No range may overlap with the previous one (sorted by start).
        for pair in projects.windows(2) {
            assert!(
                pair[0].range_end < pair[1].range_start,
                "overlapping ranges: {}-{} and {}-{}",
                pair[0].range_start,
                pair[0].range_end,
                pair[1].range_start,
                pair[1].range_end,
            );
        }

        // And the contiguous packing invariant still holds: with the default
        // base_port=4000 and range_size=10, N projects should fully cover
        // [4000, 4000 + N*10).
        assert_eq!(projects.first().unwrap().range_start, 4000);
        assert_eq!(
            projects.last().unwrap().range_end,
            4000 + (THREADS as i64) * 10 - 1,
        );
    }

    // --- remote_backends CRUD ---

    fn input<'a>(name: &'a str, alias: &'a str) -> RemoteBackendInput<'a> {
        RemoteBackendInput {
            name,
            ssh_alias: alias,
            remote_socket_path: "/run/portsage/portsage.sock",
            local_socket_path: "/tmp/portsage-dev.sock",
            auto_forward_enabled: false,
        }
    }

    #[test]
    fn remote_backend_round_trip() {
        let db = fresh_db();
        let created = db
            .create_remote_backend(input("dev", "dev-server"))
            .unwrap();
        assert!(created.id > 0);
        assert_eq!(created.name, "dev");
        assert_eq!(created.ssh_alias, "dev-server");
        assert_eq!(created.remote_socket_path, "/run/portsage/portsage.sock");
        assert_eq!(created.local_socket_path, "/tmp/portsage-dev.sock");
        assert!(!created.auto_forward_enabled);
        assert!(!created.created_at.is_empty());

        let found = db
            .get_remote_backend_by_name("dev")
            .unwrap()
            .expect("backend should exist");
        assert_eq!(found, created);
    }

    #[test]
    fn remote_backend_list_orders_by_name() {
        let db = fresh_db();
        db.create_remote_backend(input("staging", "stage")).unwrap();
        db.create_remote_backend(input("dev", "dev-server"))
            .unwrap();
        let all = db.list_remote_backends().unwrap();
        let names: Vec<_> = all.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["dev", "staging"]);
    }

    #[test]
    fn remote_backend_duplicate_name_fails() {
        let db = fresh_db();
        db.create_remote_backend(input("dev", "dev-server"))
            .unwrap();
        let err = db.create_remote_backend(input("dev", "other-server"));
        assert!(err.is_err(), "second insert with same name should fail");
    }

    #[test]
    fn remote_backend_empty_name_rejected_before_db() {
        // Defends against the frontend posting blank values. The DB has NOT NULL
        // but allows empty strings; we want a clear error at insert time.
        let db = fresh_db();
        let bad = RemoteBackendInput {
            name: "  ",
            ssh_alias: "x",
            remote_socket_path: "/x",
            local_socket_path: "/y",
            auto_forward_enabled: false,
        };
        let err = db.create_remote_backend(bad).unwrap_err().to_string();
        assert!(err.contains("name is required"), "got: {err}");
    }

    #[test]
    fn remote_backend_update_changes_fields() {
        let db = fresh_db();
        let created = db
            .create_remote_backend(input("dev", "dev-server"))
            .unwrap();
        let updated = db
            .update_remote_backend(
                created.id,
                RemoteBackendInput {
                    name: "dev",
                    ssh_alias: "dev2",
                    remote_socket_path: "/run/portsage/v2.sock",
                    local_socket_path: "/tmp/portsage-dev.sock",
                    auto_forward_enabled: true,
                },
            )
            .unwrap();
        assert_eq!(updated.ssh_alias, "dev2");
        assert_eq!(updated.remote_socket_path, "/run/portsage/v2.sock");
        assert!(updated.auto_forward_enabled);
        // created_at preserved across updates.
        assert_eq!(updated.created_at, created.created_at);
    }

    #[test]
    fn remote_backend_update_unknown_id_errors() {
        let db = fresh_db();
        let err = db
            .update_remote_backend(9999, input("dev", "dev-server"))
            .unwrap_err();
        assert!(err.to_string().contains("not found"), "got: {err}");
    }

    #[test]
    fn remote_backend_delete_removes() {
        let db = fresh_db();
        let b = db
            .create_remote_backend(input("dev", "dev-server"))
            .unwrap();
        db.delete_remote_backend(b.id).unwrap();
        assert!(db.get_remote_backend_by_name("dev").unwrap().is_none());
    }

    #[test]
    fn remote_backend_set_auto_forward_toggle() {
        let db = fresh_db();
        let b = db
            .create_remote_backend(input("dev", "dev-server"))
            .unwrap();
        assert!(!b.auto_forward_enabled);
        db.set_remote_backend_auto_forward(b.id, true).unwrap();
        let after = db.get_remote_backend_by_name("dev").unwrap().unwrap();
        assert!(after.auto_forward_enabled);
        db.set_remote_backend_auto_forward(b.id, false).unwrap();
        let after2 = db.get_remote_backend_by_name("dev").unwrap().unwrap();
        assert!(!after2.auto_forward_enabled);
    }

    #[test]
    fn remote_backend_set_auto_forward_unknown_id_errors() {
        let db = fresh_db();
        let err = db.set_remote_backend_auto_forward(9999, true).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // --- forward_exclusions ---

    fn make_backend(db: &Database, name: &str) -> RemoteBackend {
        db.create_remote_backend(input(name, "alias")).unwrap()
    }

    #[test]
    fn forward_exclusion_add_then_list() {
        let db = fresh_db();
        let b = make_backend(&db, "dev");
        let e = db.add_forward_exclusion(b.id, 4060).unwrap();
        assert_eq!(e.backend_id, b.id);
        assert_eq!(e.port, 4060);
        let all = db.list_forward_exclusions(b.id).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].port, 4060);
    }

    #[test]
    fn forward_exclusion_list_orders_by_port() {
        let db = fresh_db();
        let b = make_backend(&db, "dev");
        db.add_forward_exclusion(b.id, 4070).unwrap();
        db.add_forward_exclusion(b.id, 4060).unwrap();
        db.add_forward_exclusion(b.id, 4080).unwrap();
        let ports: Vec<i64> = db
            .list_forward_exclusions(b.id)
            .unwrap()
            .into_iter()
            .map(|e| e.port)
            .collect();
        assert_eq!(ports, [4060, 4070, 4080]);
    }

    #[test]
    fn forward_exclusion_duplicate_rejected() {
        // The unique constraint protects the sync logic from "is 4060 excluded?"
        // returning a list with duplicates - we'd waste branches checking
        // both rows.
        let db = fresh_db();
        let b = make_backend(&db, "dev");
        db.add_forward_exclusion(b.id, 4060).unwrap();
        let err = db.add_forward_exclusion(b.id, 4060);
        assert!(err.is_err());
    }

    #[test]
    fn forward_exclusion_same_port_on_different_backends_ok() {
        let db = fresh_db();
        let a = make_backend(&db, "dev");
        let b = make_backend(&db, "stage");
        db.add_forward_exclusion(a.id, 4060).unwrap();
        db.add_forward_exclusion(b.id, 4060).unwrap();
        assert_eq!(db.list_forward_exclusions(a.id).unwrap().len(), 1);
        assert_eq!(db.list_forward_exclusions(b.id).unwrap().len(), 1);
    }

    #[test]
    fn forward_exclusion_out_of_range_port_rejected() {
        let db = fresh_db();
        let b = make_backend(&db, "dev");
        assert!(db.add_forward_exclusion(b.id, 0).is_err());
        assert!(db.add_forward_exclusion(b.id, 70000).is_err());
        assert!(db.add_forward_exclusion(b.id, -1).is_err());
    }

    #[test]
    fn forward_exclusion_remove_by_id_and_by_port() {
        let db = fresh_db();
        let b = make_backend(&db, "dev");
        let e1 = db.add_forward_exclusion(b.id, 4060).unwrap();
        db.add_forward_exclusion(b.id, 4070).unwrap();
        db.remove_forward_exclusion(e1.id).unwrap();
        let ports: Vec<i64> = db
            .list_forward_exclusions(b.id)
            .unwrap()
            .into_iter()
            .map(|e| e.port)
            .collect();
        assert_eq!(ports, [4070]);
    }

    #[test]
    fn delete_remote_backend_cascades_forward_exclusions() {
        // Regression guard: deleting a backend used to leave orphan exclusion
        // rows behind, which the sync logic would then read for a backend
        // that no longer exists.
        let db = fresh_db();
        let b = make_backend(&db, "dev");
        db.add_forward_exclusion(b.id, 4060).unwrap();
        db.add_forward_exclusion(b.id, 4061).unwrap();
        db.delete_remote_backend(b.id).unwrap();
        assert!(db.list_forward_exclusions(b.id).unwrap().is_empty());
    }
}
