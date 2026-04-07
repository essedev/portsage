use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

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

    fn db_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("grimport")
            .join("grimport.db")
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
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
            INSERT OR IGNORE INTO config (key, value) VALUES ('range_size', '10');",
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_config(&self, key: &str) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
    }

    #[allow(dead_code)]
    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn next_available_range(&self) -> Result<(i64, i64)> {
        let conn = self.conn.lock().unwrap();
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

        let max_end: Option<i64> = conn
            .query_row(
                "SELECT MAX(range_end) FROM projects",
                [],
                |row| row.get(0),
            )?;

        let start = match max_end {
            Some(end) => end + 1,
            None => base_port,
        };
        Ok((start, start + range_size - 1))
    }

    pub fn create_project(
        &self,
        name: &str,
        path: Option<&str>,
    ) -> Result<Project> {
        let (range_start, range_end) = self.next_available_range()?;
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM ports WHERE project_id = ?1", params![id])?;
        conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectWithPorts>> {
        let conn = self.conn.lock().unwrap();
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

    pub fn add_port(
        &self,
        project_id: i64,
        service: &str,
        port: i64,
    ) -> Result<Port> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM ports WHERE id = ?1", params![id])?;
        Ok(())
    }
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
}
