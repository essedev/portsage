use crate::actions::{self, PortStatus, ProjectStatus};
use crate::db::Database;
use crate::paths;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

/// Idle timeout for socket connections. Closes clients that connect and
/// then sit silent, so a leaked or abandoned client cannot keep a tokio
/// task pinned forever.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub fn start_socket_server(db: Arc<Database>) {
    start_socket_server_at(db, paths::socket_path());
}

pub fn start_socket_server_at(db: Arc<Database>, path: PathBuf) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("portsage: failed to create tokio runtime, MCP socket disabled: {e}");
                return;
            }
        };
        rt.block_on(async move {
            // Remove stale socket file
            let _ = std::fs::remove_file(&path);
            // Track whether we created the parent directory ourselves. Two
            // distinct deployment shapes share this code:
            //
            //   - User XDG (`$XDG_RUNTIME_DIR/portsage/portsage.sock`): we
            //     create the directory if missing and lock it down to
            //     0700/0600 - this is a per-user-only path; nobody but the
            //     running user has any business connecting.
            //   - System-wide systemd (`/run/portsage/portsage.sock`): the
            //     unit's `RuntimeDirectory=portsage` + `RuntimeDirectoryMode=
            //     0750` create the dir for us with group `portsage` access
            //     so members of that group can connect. If we then chmod
            //     0700 on the dir or 0600 on the socket we'd silently break
            //     that policy.
            //
            // Heuristic: if the parent existed before we touched anything,
            // an external policy is in charge - leave the dir alone and put
            // the socket at 0660 so the parent's group access still works.
            // Otherwise (we created the parent) keep the legacy lockdown.
            let mut external_parent_policy = false;
            if let Some(parent) = path.parent() {
                let parent_existed_before = parent.exists();
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "portsage: cannot create socket dir {}: {e}",
                        parent.display()
                    );
                    return;
                }
                if parent_existed_before {
                    external_parent_policy = true;
                } else if let Err(e) =
                    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                {
                    eprintln!("portsage: cannot chmod 0700 on {}: {e}", parent.display());
                }
            }

            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!(
                        "portsage: failed to bind unix socket {}: {e}",
                        path.display()
                    );
                    return;
                }
            };

            // 0660 when an external policy created the parent (systemd-style
            // shared group install), 0600 otherwise (per-user XDG install).
            let socket_mode = if external_parent_policy { 0o660 } else { 0o600 };
            if let Err(e) =
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(socket_mode))
            {
                eprintln!(
                    "portsage: cannot chmod {:o} on {}: {e}",
                    socket_mode,
                    path.display()
                );
            }

            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let db = db.clone();
                    tokio::spawn(async move {
                        let (reader, mut writer) = stream.into_split();
                        let mut lines = BufReader::new(reader).lines();
                        loop {
                            let next = tokio::time::timeout(IDLE_TIMEOUT, lines.next_line()).await;
                            let line = match next {
                                Ok(Ok(Some(line))) => line,
                                // Idle timeout, EOF, or read error: close the connection.
                                _ => break,
                            };
                            let response = handle_request(&db, &line).await;
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

pub(crate) async fn handle_request(db: &Database, line: &str) -> Value {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json!({"error": format!("invalid json: {}", e)}),
    };

    let method = req["method"].as_str().unwrap_or("");
    let params = &req["params"];

    match method {
        "list_all" => match actions::list_with_status(db) {
            Ok(projects) => json!({ "result": projects }),
            Err(e) => json!({ "error": e }),
        },

        "reserve_range" => {
            let name = match params["name"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.name"}),
            };
            let path = params["path"].as_str();
            match db.create_project(name, path) {
                Ok(project) => {
                    let ps = ProjectStatus {
                        id: project.id,
                        name: project.name,
                        path: project.path,
                        range_start: project.range_start,
                        range_end: project.range_end,
                        created_at: project.created_at,
                        ports: Vec::new(),
                    };
                    json!({ "result": ps })
                }
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "register_port" => {
            let project_name = match params["project"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.project"}),
            };
            let service = match params["service"].as_str() {
                Some(s) => s,
                None => return json!({"error": "missing params.service"}),
            };
            let port = match params["port"].as_i64() {
                Some(p) => p,
                None => return json!({"error": "missing params.port"}),
            };

            let projects = match db.list_projects() {
                Ok(p) => p,
                Err(e) => return json!({"error": e.to_string()}),
            };
            let project = match projects.iter().find(|p| p.project.name == project_name) {
                Some(p) => p,
                None => return json!({"error": format!("project '{}' not found", project_name)}),
            };

            match db.add_port(project.project.id, service, port) {
                Ok(p) => {
                    let ps = PortStatus {
                        id: p.id,
                        project_id: p.project_id,
                        service: p.service,
                        port: p.port,
                        created_at: p.created_at,
                        active: false,
                        process: None,
                        pid: None,
                    };
                    json!({ "result": ps })
                }
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "release_project" => {
            let name = match params["name"].as_str() {
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

        "remove_port" => {
            let project_name = match params["project"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.project"}),
            };
            let service = match params["service"].as_str() {
                Some(s) => s,
                None => return json!({"error": "missing params.service"}),
            };
            match actions::remove_port_by_service(db, project_name, service) {
                Ok(()) => json!({"result": "ok"}),
                Err(e) => json!({"error": e}),
            }
        }

        "scan_active" => {
            let ports = actions::scan_active_detailed();
            json!({ "result": ports })
        }

        "list_unmanaged" => match actions::list_unmanaged(db) {
            Ok(ports) => json!({ "result": ports }),
            Err(e) => json!({ "error": e }),
        },

        "next_range" => match actions::next_range(db) {
            Ok((start, end)) => json!({
                "result": {
                    "range_start": start,
                    "range_end": end,
                }
            }),
            Err(e) => json!({ "error": e }),
        },

        "get_config" => match actions::get_config(db) {
            Ok((base_port, range_size)) => json!({
                "result": {
                    "base_port": base_port,
                    "range_size": range_size,
                }
            }),
            Err(e) => json!({ "error": e }),
        },

        "set_config" => {
            let key = match params["key"].as_str() {
                Some(k) => k,
                None => return json!({"error": "missing params.key"}),
            };
            let value = match params["value"].as_str() {
                Some(v) => v,
                None => return json!({"error": "missing params.value"}),
            };
            // Whitelist: only base_port and range_size are mutable via the socket.
            // The config table is also used by future feature toggles; we don't
            // want a remote caller setting arbitrary keys without explicit support.
            if !matches!(key, "base_port" | "range_size") {
                return json!({"error": format!("unknown config key: {}", key)});
            }
            match actions::set_config(db, key, value) {
                Ok(()) => json!({"result": "ok"}),
                Err(e) => json!({ "error": e }),
            }
        }

        "kill_port" => {
            let port = match params["port"].as_i64() {
                Some(p) => p,
                None => return json!({"error": "missing params.port"}),
            };
            let outcome = actions::kill_port_action(port).await;
            json!({ "result": { "outcome": outcome } })
        }

        "kill_project" => {
            let name = match params["name"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.name"}),
            };
            match actions::kill_project_by_name(db, name).await {
                Ok(results) => {
                    let entries: Vec<Value> = results
                        .into_iter()
                        .map(|(port, outcome)| json!({ "port": port, "outcome": outcome }))
                        .collect();
                    json!({ "result": entries })
                }
                Err(e) => json!({ "error": e }),
            }
        }

        "open_in_browser" => {
            let port = match params["port"].as_i64() {
                Some(p) => p,
                None => return json!({"error": "missing params.port"}),
            };
            match actions::open_in_browser(port) {
                Ok(()) => json!({"result": "ok"}),
                Err(e) => json!({ "error": e }),
            }
        }

        "find_project_by_path" => {
            let path = match params["path"].as_str() {
                Some(p) => p,
                None => return json!({"error": "missing params.path"}),
            };
            match actions::find_project_by_path(db, path) {
                Ok(Some(p)) => json!({ "result": p }),
                Ok(None) => json!({ "result": null }),
                Err(e) => json!({ "error": e }),
            }
        }

        "update_project" => {
            let current = match params["current_name"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.current_name"}),
            };
            // Absent keys mean "leave unchanged"; the actions/db layer rejects
            // the all-absent case so we don't have to duplicate that check here.
            let new_name = params["new_name"].as_str();
            let new_path = params["new_path"].as_str();
            match actions::update_project(db, current, new_name, new_path) {
                Ok(ps) => json!({ "result": ps }),
                Err(e) => json!({ "error": e }),
            }
        }

        "get_remote_backend" => {
            let name = match params["name"].as_str() {
                Some(n) => n,
                None => return json!({"error": "missing params.name"}),
            };
            match db.get_remote_backend_by_name(name) {
                Ok(Some(b)) => json!({ "result": b }),
                Ok(None) => json!({ "result": null }),
                Err(e) => json!({ "error": e.to_string() }),
            }
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

    async fn req(db: &Database, json: &str) -> Value {
        handle_request(db, json).await
    }

    // --- Protocol errors ---

    #[tokio::test]
    async fn invalid_json_returns_error() {
        let db = fresh_db();
        let res = req(&db, "not json").await;
        assert!(res["error"].as_str().unwrap().contains("invalid json"));
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"foo"}"#).await;
        assert!(res["error"].as_str().unwrap().contains("unknown method"));
    }

    #[tokio::test]
    async fn missing_method_returns_unknown() {
        let db = fresh_db();
        let res = req(&db, r#"{"params":{}}"#).await;
        assert!(res["error"].as_str().unwrap().contains("unknown method"));
    }

    // --- reserve_range ---

    #[tokio::test]
    async fn reserve_range_returns_full_project_payload() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"test","path":"/tmp/test"}}"#,
        )
        .await;
        assert_eq!(res["result"]["name"], "test");
        assert_eq!(res["result"]["path"], "/tmp/test");
        // id must come back so the caller can address the project directly.
        assert!(res["result"]["id"].is_i64());
        assert_eq!(res["result"]["range_start"], 4000);
        assert_eq!(res["result"]["range_end"], 4009);
        assert!(res["result"]["ports"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reserve_range_missing_name() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"reserve_range","params":{}}"#).await;
        assert!(res["error"]
            .as_str()
            .unwrap()
            .contains("missing params.name"));
    }

    #[tokio::test]
    async fn reserve_range_duplicate_name() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"test"}}"#,
        )
        .await;
        let res = req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"test"}}"#,
        )
        .await;
        assert!(res["error"].is_string());
    }

    #[tokio::test]
    async fn reserve_range_sequential() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"alpha"}}"#,
        )
        .await;
        let res = req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"bravo"}}"#,
        )
        .await;
        assert_eq!(res["result"]["range_start"], 4010);
    }

    // --- register_port ---

    #[tokio::test]
    async fn register_port_returns_full_port_payload() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"test"}}"#,
        )
        .await;
        let res = req(
            &db,
            r#"{"method":"register_port","params":{"project":"test","service":"vite","port":4000}}"#,
        ).await;
        assert_eq!(res["result"]["service"], "vite");
        assert_eq!(res["result"]["port"], 4000);
        // id and created_at must travel so the caller can reference this row later.
        assert!(res["result"]["id"].is_i64());
        assert!(res["result"]["created_at"].is_string());
        assert!(res["result"]["project_id"].is_i64());
    }

    #[tokio::test]
    async fn register_port_missing_params() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"register_port","params":{}}"#).await;
        assert!(res["error"].as_str().unwrap().contains("missing"));
    }

    #[tokio::test]
    async fn register_port_unknown_project() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"register_port","params":{"project":"ghost","service":"vite","port":4000}}"#,
        ).await;
        assert!(res["error"].as_str().unwrap().contains("not found"));
    }

    // --- list_all (enriched payload) ---

    #[tokio::test]
    async fn list_all_empty() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"list_all"}"#).await;
        assert!(res["result"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_all_returns_full_project_status_shape() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"alpha","path":"/tmp/alpha"}}"#,
        )
        .await;
        req(
            &db,
            r#"{"method":"register_port","params":{"project":"alpha","service":"vite","port":4000}}"#,
        ).await;
        let res = req(&db, r#"{"method":"list_all"}"#).await;
        let projects = res["result"].as_array().unwrap();
        assert_eq!(projects.len(), 1);
        let p = &projects[0];
        // Enrichment must carry ids and range fields explicitly (no `range: [a, b]` legacy shape).
        assert!(p["id"].is_i64());
        assert_eq!(p["name"], "alpha");
        assert_eq!(p["path"], "/tmp/alpha");
        assert_eq!(p["range_start"], 4000);
        assert_eq!(p["range_end"], 4009);
        assert!(p["created_at"].is_string());
        let port = &p["ports"].as_array().unwrap()[0];
        assert!(port["id"].is_i64());
        assert_eq!(port["service"], "vite");
        assert_eq!(port["port"], 4000);
        assert!(port["active"].is_boolean());
        // process and pid are Option<...> in Rust - present as null when inactive.
        assert!(port["process"].is_null() || port["process"].is_string());
        assert!(port["pid"].is_null() || port["pid"].is_i64());
    }

    // --- release_project ---

    #[tokio::test]
    async fn release_project_success() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"test"}}"#,
        )
        .await;
        let res = req(
            &db,
            r#"{"method":"release_project","params":{"name":"test"}}"#,
        )
        .await;
        assert_eq!(res["result"], "ok");
        let list = req(&db, r#"{"method":"list_all"}"#).await;
        assert!(list["result"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn release_project_not_found() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"release_project","params":{"name":"ghost"}}"#,
        )
        .await;
        assert!(res["error"].as_str().unwrap().contains("not found"));
    }

    // --- remove_port ---

    #[tokio::test]
    async fn remove_port_by_service_succeeds() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"alpha"}}"#,
        )
        .await;
        req(
            &db,
            r#"{"method":"register_port","params":{"project":"alpha","service":"vite","port":4000}}"#,
        ).await;
        let res = req(
            &db,
            r#"{"method":"remove_port","params":{"project":"alpha","service":"vite"}}"#,
        )
        .await;
        assert_eq!(res["result"], "ok");
        let list = req(&db, r#"{"method":"list_all"}"#).await;
        let ports = list["result"][0]["ports"].as_array().unwrap();
        assert!(ports.is_empty());
    }

    #[tokio::test]
    async fn remove_port_unknown_service_errors() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"alpha"}}"#,
        )
        .await;
        let res = req(
            &db,
            r#"{"method":"remove_port","params":{"project":"alpha","service":"ghost"}}"#,
        )
        .await;
        assert!(res["error"].is_string());
    }

    #[tokio::test]
    async fn remove_port_missing_params() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"remove_port","params":{"project":"alpha"}}"#,
        )
        .await;
        assert!(res["error"].as_str().unwrap().contains("missing"));
    }

    // --- list_unmanaged / scan_active ---

    #[tokio::test]
    async fn list_unmanaged_returns_array() {
        // We can't assert specific ports (depends on host), but the shape must
        // be an array of objects with port/process/pid fields.
        let db = fresh_db();
        let res = req(&db, r#"{"method":"list_unmanaged"}"#).await;
        assert!(res["result"].is_array(), "expected array, got {res}");
    }

    #[tokio::test]
    async fn scan_active_returns_detailed_objects() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"scan_active"}"#).await;
        // Same as list_unmanaged: structural only.
        assert!(res["result"].is_array());
    }

    // --- next_range / config ---

    #[tokio::test]
    async fn next_range_returns_explicit_fields() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"next_range"}"#).await;
        assert_eq!(res["result"]["range_start"], 4000);
        assert_eq!(res["result"]["range_end"], 4009);
    }

    #[tokio::test]
    async fn get_config_returns_defaults_on_fresh_db() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"get_config"}"#).await;
        assert_eq!(res["result"]["base_port"], "4000");
        assert_eq!(res["result"]["range_size"], "10");
    }

    #[tokio::test]
    async fn set_config_round_trips() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"set_config","params":{"key":"base_port","value":"5000"}}"#,
        )
        .await;
        assert_eq!(res["result"], "ok");
        let res = req(&db, r#"{"method":"get_config"}"#).await;
        assert_eq!(res["result"]["base_port"], "5000");
    }

    #[tokio::test]
    async fn set_config_rejects_unknown_key() {
        // Defends against a remote caller mutating arbitrary keys (e.g. feature
        // flags or future booleans) the server has not opted in to expose.
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"set_config","params":{"key":"secret","value":"x"}}"#,
        )
        .await;
        assert!(res["error"]
            .as_str()
            .unwrap()
            .contains("unknown config key"));
    }

    // --- kill_port (process-free path: nothing listening) ---

    #[tokio::test]
    async fn kill_port_inactive_returns_not_active() {
        let db = fresh_db();
        // Port 1 should never be listening (privileged) so the action returns NotActive
        // without actually killing anything; this keeps the test fast and harmless.
        let res = req(&db, r#"{"method":"kill_port","params":{"port":1}}"#).await;
        assert_eq!(res["result"]["outcome"], "not_active");
    }

    #[tokio::test]
    async fn kill_port_missing_param() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"kill_port","params":{}}"#).await;
        assert!(res["error"].as_str().unwrap().contains("missing"));
    }

    #[tokio::test]
    async fn kill_project_returns_empty_results_when_nothing_active() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"alpha"}}"#,
        )
        .await;
        req(
            &db,
            r#"{"method":"register_port","params":{"project":"alpha","service":"vite","port":4000}}"#,
        ).await;
        let res = req(
            &db,
            r#"{"method":"kill_project","params":{"name":"alpha"}}"#,
        )
        .await;
        // No process on 4000 -> the registered port is not "active" so it's filtered
        // out before kill_pid_with_escalation runs. Result is an empty list.
        let arr = res["result"].as_array().unwrap();
        assert!(arr.is_empty());
    }

    #[tokio::test]
    async fn kill_project_unknown_project_errors() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"kill_project","params":{"name":"ghost"}}"#,
        )
        .await;
        assert!(res["error"].as_str().unwrap().contains("not found"));
    }

    // --- open_in_browser ---

    #[tokio::test]
    async fn open_in_browser_rejects_invalid_port() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"open_in_browser","params":{"port":0}}"#).await;
        assert!(res["error"].as_str().unwrap().contains("invalid port"));
    }

    #[tokio::test]
    async fn open_in_browser_missing_port() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"open_in_browser","params":{}}"#).await;
        assert!(res["error"].as_str().unwrap().contains("missing"));
    }

    // --- find_project_by_path ---

    #[tokio::test]
    async fn find_project_by_path_returns_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();
        let db = fresh_db();
        req(
            &db,
            &format!(
                r#"{{"method":"reserve_range","params":{{"name":"alpha","path":"{}"}}}}"#,
                path
            ),
        )
        .await;
        let res = req(
            &db,
            &format!(
                r#"{{"method":"find_project_by_path","params":{{"path":"{}"}}}}"#,
                path
            ),
        )
        .await;
        assert_eq!(res["result"]["name"], "alpha");
    }

    #[tokio::test]
    async fn find_project_by_path_returns_null_when_no_match() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"find_project_by_path","params":{"path":"/no/such/place"}}"#,
        )
        .await;
        assert!(res["result"].is_null());
    }

    #[tokio::test]
    async fn find_project_by_path_missing_param() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"find_project_by_path","params":{}}"#).await;
        assert!(res["error"].as_str().unwrap().contains("missing"));
    }

    // --- update_project ---

    #[tokio::test]
    async fn update_project_renames_and_keeps_ports() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"omnia","path":"/old"}}"#,
        )
        .await;
        req(
            &db,
            r#"{"method":"register_port","params":{"project":"omnia","service":"vite","port":4000}}"#,
        )
        .await;
        let res = req(
            &db,
            r#"{"method":"update_project","params":{"current_name":"omnia","new_name":"omnia-ddt","new_path":"/new"}}"#,
        )
        .await;
        assert_eq!(res["result"]["name"], "omnia-ddt");
        assert_eq!(res["result"]["path"], "/new");
        assert_eq!(res["result"]["range_start"], 4000);
        assert_eq!(res["result"]["ports"].as_array().unwrap().len(), 1);
        // The old name is gone, the new one resolves.
        let list = req(&db, r#"{"method":"list_all"}"#).await;
        assert_eq!(list["result"][0]["name"], "omnia-ddt");
    }

    #[tokio::test]
    async fn update_project_missing_current_name() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"update_project","params":{}}"#).await;
        assert!(res["error"]
            .as_str()
            .unwrap()
            .contains("missing params.current_name"));
    }

    #[tokio::test]
    async fn update_project_requires_a_field() {
        let db = fresh_db();
        req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"alpha"}}"#,
        )
        .await;
        let res = req(
            &db,
            r#"{"method":"update_project","params":{"current_name":"alpha"}}"#,
        )
        .await;
        assert!(res["error"].as_str().unwrap().contains("at least one"));
    }

    #[tokio::test]
    async fn update_project_unknown_project_errors() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"update_project","params":{"current_name":"ghost","new_name":"x"}}"#,
        )
        .await;
        assert!(res["error"].as_str().unwrap().contains("not found"));
    }

    // --- get_remote_backend ---

    // Uses `Database::create_remote_backend`, which lives in the GUI-gated
    // CRUD block - the headless server only needs to *look up* backends, not
    // create them.
    #[cfg(feature = "gui")]
    #[tokio::test]
    async fn get_remote_backend_returns_match() {
        let db = fresh_db();
        db.create_remote_backend(crate::db::RemoteBackendInput {
            name: "dev",
            ssh_alias: "dev-server",
            remote_socket_path: "/run/portsage/portsage.sock",
            local_socket_path: "/tmp/portsage-dev.sock",
            auto_forward_enabled: false,
        })
        .unwrap();
        let res = req(
            &db,
            r#"{"method":"get_remote_backend","params":{"name":"dev"}}"#,
        )
        .await;
        assert_eq!(res["result"]["name"], "dev");
        assert_eq!(res["result"]["ssh_alias"], "dev-server");
        assert_eq!(res["result"]["local_socket_path"], "/tmp/portsage-dev.sock");
    }

    #[tokio::test]
    async fn get_remote_backend_returns_null_when_missing() {
        let db = fresh_db();
        let res = req(
            &db,
            r#"{"method":"get_remote_backend","params":{"name":"ghost"}}"#,
        )
        .await;
        assert!(res["result"].is_null());
    }

    #[tokio::test]
    async fn get_remote_backend_missing_param() {
        let db = fresh_db();
        let res = req(&db, r#"{"method":"get_remote_backend","params":{}}"#).await;
        assert!(res["error"].as_str().unwrap().contains("missing"));
    }

    // --- Full workflow ---

    // --- End-to-end: real socket + real handler + portsage_client::Client ---
    //
    // This is the load-bearing integration test for the wire protocol. The
    // unit tests above poke `handle_request` directly with JSON strings; here
    // we round-trip through a real Unix socket using `portsage_client::Client`,
    // catching any drift between what the server emits (via `json!` macros and
    // typed Serialize) and what the client deserializes (typed structs).
    //
    // We use multi-threaded flavor so the sync client (blocking UnixStream)
    // doesn't deadlock with the async server task on a single worker.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn end_to_end_round_trip_via_real_client() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("portsage.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let db = Arc::new(fresh_db());

        // Server task: accept connections and dispatch requests via handle_request.
        let db_for_server = db.clone();
        let server = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let db = db_for_server.clone();
                tokio::spawn(async move {
                    let (reader, mut writer) = stream.into_split();
                    let mut lines = BufReader::new(reader).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let response = handle_request(&db, &line).await;
                        let mut out = serde_json::to_string(&response).unwrap();
                        out.push('\n');
                        if writer.write_all(out.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });

        let sock_clone = sock_path.clone();
        let client_result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let client = portsage_client::Client::new(sock_clone);

            // 1. reserve_range -> ProjectStatus
            let proj = client
                .reserve_range("alpha", Some("/tmp/alpha"))
                .map_err(|e| format!("reserve_range: {e}"))?;
            assert_eq!(proj.name, "alpha");
            assert_eq!(proj.range_start, 4000);
            assert_eq!(proj.range_end, 4009);
            assert_eq!(proj.path.as_deref(), Some("/tmp/alpha"));
            assert!(proj.ports.is_empty());

            // 2. register_port -> PortStatus
            let p = client
                .register_port("alpha", "vite", 4000)
                .map_err(|e| format!("register_port: {e}"))?;
            assert_eq!(p.service, "vite");
            assert_eq!(p.port, 4000);
            assert!(p.id > 0);

            // 3. list_all -> Vec<ProjectStatus>
            let all = client.list_all().map_err(|e| format!("list_all: {e}"))?;
            assert_eq!(all.len(), 1);
            assert_eq!(all[0].name, "alpha");
            assert_eq!(all[0].ports.len(), 1);

            // 4. next_range -> RangeBounds
            let next = client
                .next_range()
                .map_err(|e| format!("next_range: {e}"))?;
            assert_eq!(next.range_start, 4010);
            assert_eq!(next.range_end, 4019);

            // 5. get_config -> ConfigSnapshot
            let cfg = client
                .get_config()
                .map_err(|e| format!("get_config: {e}"))?;
            assert_eq!(cfg.base_port, "4000");
            assert_eq!(cfg.range_size, "10");

            // 6. find_project_by_path
            // Use a path that won't canonicalize to anything different than what we registered.
            let found = client
                .find_project_by_path("/tmp/alpha")
                .map_err(|e| format!("find_project_by_path: {e}"))?;
            // The registered path /tmp/alpha may not exist on the test host, so
            // canonicalize() falls back to the literal string. Either way, the
            // server should resolve it.
            assert!(
                found.is_some(),
                "expected to find project by registered path"
            );
            assert_eq!(found.unwrap().name, "alpha");

            // 7. update_project -> ProjectStatus (rename, keep the port)
            let renamed = client
                .update_project("alpha", Some("alpha2"), None)
                .map_err(|e| format!("update_project: {e}"))?;
            assert_eq!(renamed.name, "alpha2");
            assert_eq!(renamed.range_start, 4000);
            assert_eq!(renamed.ports.len(), 1, "port survives the rename");

            // 8. release_project (by the new name)
            client
                .release_project("alpha2")
                .map_err(|e| format!("release_project: {e}"))?;
            let after = client.list_all().map_err(|e| format!("list_all: {e}"))?;
            assert!(after.is_empty());

            Ok(())
        })
        .await
        .unwrap();

        server.abort();
        client_result.expect("end-to-end round trip failed");
    }

    #[tokio::test]
    async fn full_workflow_reserve_register_list_release() {
        let db = fresh_db();

        let res = req(
            &db,
            r#"{"method":"reserve_range","params":{"name":"myapp","path":"/tmp/myapp"}}"#,
        )
        .await;
        assert_eq!(res["result"]["name"], "myapp");

        req(
            &db,
            r#"{"method":"register_port","params":{"project":"myapp","service":"vite","port":4000}}"#,
        ).await;
        req(
            &db,
            r#"{"method":"register_port","params":{"project":"myapp","service":"api","port":4001}}"#,
        ).await;

        let res = req(&db, r#"{"method":"list_all"}"#).await;
        let projects = res["result"].as_array().unwrap();
        assert_eq!(projects[0]["ports"].as_array().unwrap().len(), 2);
        assert_eq!(projects[0]["path"], "/tmp/myapp");

        let res = req(
            &db,
            r#"{"method":"release_project","params":{"name":"myapp"}}"#,
        )
        .await;
        assert_eq!(res["result"], "ok");

        let res = req(&db, r#"{"method":"list_all"}"#).await;
        assert!(res["result"].as_array().unwrap().is_empty());
    }
}
