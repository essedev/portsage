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
