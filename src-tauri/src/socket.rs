use crate::db::Database;
use crate::scanner::scan_active_ports;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

fn socket_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("grimport")
        .join("grimport.sock")
}

pub fn start_socket_server(db: Arc<Database>) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async move {
            let path = socket_path();
            // Remove stale socket file
            let _ = std::fs::remove_file(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let listener = UnixListener::bind(&path)
                .expect("failed to bind unix socket");

            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let db = db.clone();
                    tokio::spawn(async move {
                        let (reader, mut writer) = stream.into_split();
                        let mut lines = BufReader::new(reader).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let response = handle_request(&db, &line);
                            let mut out = serde_json::to_string(&response)
                                .unwrap_or_else(|_| r#"{"error":"serialize failed"}"#.into());
                            out.push('\n');
                            if writer.write_all(out.as_bytes()).await.is_err() {
                                break;
                            }
                        }
                    });
                }
            }
        });
    });
}

pub(crate) fn handle_request(db: &Database, line: &str) -> Value {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json!({"error": format!("invalid json: {}", e)}),
    };

    let method = req["method"].as_str().unwrap_or("");

    match method {
        "list_all" => {
            match db.list_projects() {
                Ok(projects) => {
                    let active_ports = scan_active_ports();
                    let result: Vec<Value> = projects
                        .into_iter()
                        .map(|pwp| {
                            let ports: Vec<Value> = pwp
                                .ports
                                .into_iter()
                                .map(|p| {
                                    json!({
                                        "service": p.service,
                                        "port": p.port,
                                        "active": active_ports.contains(&p.port),
                                    })
                                })
                                .collect();
                            json!({
                                "id": pwp.project.id,
                                "name": pwp.project.name,
                                "path": pwp.project.path,
                                "range": [pwp.project.range_start, pwp.project.range_end],
                                "ports": ports,
                            })
                        })
                        .collect();
                    json!({"result": result})
                }
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "reserve_range" => {
            let name = match req["params"]["name"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.name"}),
            };
            let path = req["params"]["path"].as_str();
            match db.create_project(name, path) {
                Ok(project) => json!({
                    "result": {
                        "name": project.name,
                        "range": [project.range_start, project.range_end],
                    }
                }),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "register_port" => {
            let project_name = match req["params"]["project"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.project"}),
            };
            let service = match req["params"]["service"].as_str() {
                Some(s) => s,
                None => return json!({"error": "missing params.service"}),
            };
            let port = match req["params"]["port"].as_i64() {
                Some(p) => p,
                None => return json!({"error": "missing params.port"}),
            };

            // Find project by name
            let projects = match db.list_projects() {
                Ok(p) => p,
                Err(e) => return json!({"error": e.to_string()}),
            };
            let project = match projects.iter().find(|p| p.project.name == project_name) {
                Some(p) => p,
                None => return json!({"error": format!("project '{}' not found", project_name)}),
            };

            match db.add_port(project.project.id, service, port) {
                Ok(p) => json!({
                    "result": {
                        "service": p.service,
                        "port": p.port,
                    }
                }),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "release_project" => {
            let name = match req["params"]["name"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.name"}),
            };
            let projects = match db.list_projects() {
                Ok(p) => p,
                Err(e) => return json!({"error": e.to_string()}),
            };
            let project = match projects.iter().find(|p| p.project.name == name) {
                Some(p) => p,
                None => return json!({"error": format!("project '{}' not found", name)}),
            };
            match db.delete_project(project.project.id) {
                Ok(_) => json!({"result": "ok"}),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "scan_active" => {
            let mut ports: Vec<i64> = scan_active_ports().into_iter().collect();
            ports.sort();
            json!({"result": ports})
        }

        _ => json!({"error": format!("unknown method: {}", method)}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn fresh_db() -> Database {
        Database::in_memory().expect("failed to create in-memory db")
    }

    fn req(db: &Database, json: &str) -> Value {
        handle_request(db, json)
    }

    // --- Protocol errors ---

    #[test]
    fn invalid_json_returns_error() {
        let db = fresh_db();
        let res = req(&db, "not json");
        assert!(res["error"].as_str().unwrap().contains("invalid json"));
    }

    #[test]
    fn unknown_method_returns_error() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"foo"}"#);
        assert!(res["error"].as_str().unwrap().contains("unknown method"));
    }

    #[test]
    fn missing_method_returns_unknown() {
        let db = fresh_db();
        let res = req(&db, r#"{"params":{}}"#);
        assert!(res["error"].as_str().unwrap().contains("unknown method"));
    }

    // --- reserve_range ---

    #[test]
    fn reserve_range_success() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"reserve_range","params":{"name":"test"}}"#);
        assert_eq!(res["result"]["name"], "test");
        let range = res["result"]["range"].as_array().unwrap();
        assert_eq!(range[0], 4000);
        assert_eq!(range[1], 4009);
    }

    #[test]
    fn reserve_range_missing_name() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"reserve_range","params":{}}"#);
        assert!(res["error"].as_str().unwrap().contains("missing params.name"));
    }

    #[test]
    fn reserve_range_duplicate_name() {
        let db = fresh_db();
        req(&db, r#"{"method":"reserve_range","params":{"name":"test"}}"#);
        let res = req(&db, r#"{"method":"reserve_range","params":{"name":"test"}}"#);
        assert!(res["error"].is_string());
    }

    #[test]
    fn reserve_range_sequential() {
        let db = fresh_db();
        req(&db, r#"{"method":"reserve_range","params":{"name":"alpha"}}"#);
        let res = req(&db, r#"{"method":"reserve_range","params":{"name":"bravo"}}"#);
        let range = res["result"]["range"].as_array().unwrap();
        assert_eq!(range[0], 4010);
    }

    // --- register_port ---

    #[test]
    fn register_port_success() {
        let db = fresh_db();
        req(&db, r#"{"method":"reserve_range","params":{"name":"test"}}"#);
        let res = req(
            &db,
            r#"{"method":"register_port","params":{"project":"test","service":"vite","port":4000}}"#,
        );
        assert_eq!(res["result"]["service"], "vite");
        assert_eq!(res["result"]["port"], 4000);
    }

    #[test]
    fn register_port_missing_params() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"register_port","params":{}}"#);
        assert!(res["error"].as_str().unwrap().contains("missing"));
    }

    #[test]
    fn register_port_unknown_project() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"register_port","params":{"project":"ghost","service":"vite","port":4000}}"#,
        );
        assert!(res["error"].as_str().unwrap().contains("not found"));
    }

    // --- list_all ---

    #[test]
    fn list_all_empty() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"list_all"}"#);
        assert!(res["result"].as_array().unwrap().is_empty());
    }

    #[test]
    fn list_all_with_projects_and_ports() {
        let db = fresh_db();
        req(&db, r#"{"method":"reserve_range","params":{"name":"alpha"}}"#);
        req(
            &db,
            r#"{"method":"register_port","params":{"project":"alpha","service":"vite","port":4000}}"#,
        );
        let res = req(&db, r#"{"method":"list_all"}"#);
        let projects = res["result"].as_array().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["name"], "alpha");
        assert_eq!(projects[0]["ports"].as_array().unwrap().len(), 1);
        assert_eq!(projects[0]["ports"][0]["service"], "vite");
    }

    // --- release_project ---

    #[test]
    fn release_project_success() {
        let db = fresh_db();
        req(&db, r#"{"method":"reserve_range","params":{"name":"test"}}"#);
        let res = req(&db, r#"{"method":"release_project","params":{"name":"test"}}"#);
        assert_eq!(res["result"], "ok");
        // Verify it's gone
        let list = req(&db, r#"{"method":"list_all"}"#);
        assert!(list["result"].as_array().unwrap().is_empty());
    }

    #[test]
    fn release_project_not_found() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"release_project","params":{"name":"ghost"}}"#);
        assert!(res["error"].as_str().unwrap().contains("not found"));
    }

    // --- Full workflow ---

    #[test]
    fn full_workflow_reserve_register_list_release() {
        let db = fresh_db();

        // Reserve
        let res = req(&db, r#"{"method":"reserve_range","params":{"name":"myapp","path":"/tmp/myapp"}}"#);
        assert_eq!(res["result"]["name"], "myapp");

        // Register ports
        req(
            &db,
            r#"{"method":"register_port","params":{"project":"myapp","service":"vite","port":4000}}"#,
        );
        req(
            &db,
            r#"{"method":"register_port","params":{"project":"myapp","service":"api","port":4001}}"#,
        );

        // List and verify
        let res = req(&db, r#"{"method":"list_all"}"#);
        let projects = res["result"].as_array().unwrap();
        assert_eq!(projects[0]["ports"].as_array().unwrap().len(), 2);
        assert_eq!(projects[0]["path"], "/tmp/myapp");

        // Release
        let res = req(&db, r#"{"method":"release_project","params":{"name":"myapp"}}"#);
        assert_eq!(res["result"], "ok");

        // Verify empty
        let res = req(&db, r#"{"method":"list_all"}"#);
        assert!(res["result"].as_array().unwrap().is_empty());
    }
}
