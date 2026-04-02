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

fn handle_request(db: &Database, line: &str) -> Value {
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
