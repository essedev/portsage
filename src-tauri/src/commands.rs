use crate::actions::{self, KillOutcome, PortStatus, ProjectStatus};
use crate::backends::{BackendRouter, BackendTarget, RemoteBackendForm, TunnelStatus};
use crate::db::{Database, ForwardExclusion, RemoteBackend};
use crate::forwards::{ForwardManager, ForwardStatus};
use crate::scanner::ActivePort;
use portsage_client::{ConfigSnapshot, KillEntry, RangeBounds};
use std::path::Path;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

/// Read and parse a JSON file at `path`. If the file does not exist, returns
/// an empty object. If the file exists but is malformed, returns Err with a
/// clear "refusing to overwrite" message. This is the **safety-critical**
/// helper used by `install_mcp` before merging into the user's `~/.claude.json`
/// and `~/.claude/settings.json`: falling back to `{}` on parse failure would
/// silently destroy the user's entire editor config.
fn parse_existing_or_empty(path: &Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| {
        format!(
            "{} appears to be corrupt and cannot be parsed: {}. Refusing to overwrite. \
             Fix or back up the file manually before retrying.",
            path.display(),
            e
        )
    })
}

// === Project & port commands ===
//
// These dispatch via `BackendRouter::client()` so the active backend (Local
// or one of the Remote ones configured in Settings) drives every read and
// write. The Tauri command signatures still take `i64` IDs because that's
// what the frontend has cached from a previous list_all - resolution from
// id to name happens inside the command and travels exactly once.

#[tauri::command]
pub fn list_projects(router: State<Arc<BackendRouter>>) -> Result<Vec<ProjectStatus>, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.list_all()
}

#[tauri::command]
pub fn create_project(
    router: State<Arc<BackendRouter>>,
    name: String,
    path: Option<String>,
) -> Result<ProjectStatus, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.reserve_range(&name, path.as_deref())
}

#[tauri::command]
pub fn delete_project(router: State<Arc<BackendRouter>>, id: i64) -> Result<(), String> {
    let client = router.client().map_err(|e| e.to_string())?;
    let projects = client.list_all()?;
    let proj = projects
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("project {id} not found"))?;
    client.release_project(&proj.name)
}

#[tauri::command]
pub fn add_port(
    router: State<Arc<BackendRouter>>,
    project_id: i64,
    service: String,
    port: i64,
) -> Result<PortStatus, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    let projects = client.list_all()?;
    let proj = projects
        .iter()
        .find(|p| p.id == project_id)
        .ok_or_else(|| format!("project {project_id} not found"))?;
    client.register_port(&proj.name, &service, port)
}

#[tauri::command]
pub fn remove_port(router: State<Arc<BackendRouter>>, id: i64) -> Result<(), String> {
    let client = router.client().map_err(|e| e.to_string())?;
    let projects = client.list_all()?;
    for p in &projects {
        for port in &p.ports {
            if port.id == id {
                return client.remove_port(&p.name, &port.service);
            }
        }
    }
    Err(format!("port {id} not found"))
}

#[tauri::command]
pub fn scan_ports(router: State<Arc<BackendRouter>>) -> Result<Vec<i64>, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    let mut ports: Vec<i64> = client.scan_active()?.into_iter().map(|p| p.port).collect();
    ports.sort();
    Ok(ports)
}

#[tauri::command]
pub fn list_unmanaged_ports(router: State<Arc<BackendRouter>>) -> Result<Vec<ActivePort>, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.list_unmanaged()
}

#[tauri::command]
pub fn get_next_range(router: State<Arc<BackendRouter>>) -> Result<RangeBounds, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.next_range()
}

#[tauri::command]
pub fn open_in_finder(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_in_terminal(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .args(["-a", "Terminal", &path])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_in_browser(router: State<Arc<BackendRouter>>, port: i64) -> Result<(), String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.open_in_browser(port)
}

#[tauri::command]
pub async fn kill_port(
    router: State<'_, Arc<BackendRouter>>,
    port: i64,
) -> Result<KillOutcome, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.kill_port(port).await
}

#[tauri::command]
pub async fn kill_project(
    router: State<'_, Arc<BackendRouter>>,
    project_id: i64,
) -> Result<Vec<KillEntry>, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    let projects = client.list_all()?;
    let proj = projects
        .iter()
        .find(|p| p.id == project_id)
        .ok_or_else(|| format!("project {project_id} not found"))?;
    client.kill_project(&proj.name).await
}

#[tauri::command]
pub fn get_config(router: State<Arc<BackendRouter>>) -> Result<ConfigSnapshot, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.get_config()
}

#[tauri::command]
pub fn set_config(
    router: State<Arc<BackendRouter>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.set_config(&key, &value)
}

#[tauri::command]
pub fn export_data(db: State<Arc<Database>>, dest_path: String) -> Result<(), String> {
    let db_path = Database::db_path();

    if !db_path.exists() {
        return Err("Database not found".into());
    }

    let file = std::fs::File::create(&dest_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("portsage.db", options)
        .map_err(|e| e.to_string())?;
    let db_bytes = std::fs::read(&db_path).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip, &db_bytes).map_err(|e| e.to_string())?;

    zip.start_file("config.json", options)
        .map_err(|e| e.to_string())?;
    let base_port = db.get_config("base_port").unwrap_or("4000".into());
    let range_size = db.get_config("range_size").unwrap_or("10".into());
    let config = serde_json::json!({
        "base_port": base_port,
        "range_size": range_size,
    });
    let config_bytes = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip, config_bytes.as_bytes()).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn import_data(source_path: String) -> Result<(), String> {
    let db_path = Database::db_path();

    let file = std::fs::File::open(&source_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut db_file = archive.by_name("portsage.db").map_err(|e| e.to_string())?;
    let mut db_bytes = Vec::new();
    std::io::Read::read_to_end(&mut db_file, &mut db_bytes).map_err(|e| e.to_string())?;
    drop(db_file);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&db_path, &db_bytes).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn get_mcp_dir(app: tauri::AppHandle) -> Result<String, String> {
    let config_mcp = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("portsage")
        .join("mcp");

    // Always prefer bundled resources when available, overwriting any existing files in
    // the config dir. This is critical: it lets app upgrades (e.g. brew upgrade) propagate
    // fixes to server.py / SKILL.md to users who already have a copy from a previous install,
    // instead of leaving them stuck with stale files.
    let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
    let bundled_mcp = resource_dir.join("mcp");
    if bundled_mcp.join("server.py").exists() {
        std::fs::create_dir_all(&config_mcp).map_err(|e| e.to_string())?;
        for file in &["server.py", "pyproject.toml", "SKILL.md"] {
            let src = bundled_mcp.join(file);
            let dst = config_mcp.join(file);
            if src.exists() {
                std::fs::copy(&src, &dst).map_err(|e| e.to_string())?;
            }
        }
        return Ok(config_mcp.to_string_lossy().to_string());
    }

    if config_mcp.join("server.py").exists() {
        return Ok(config_mcp.to_string_lossy().to_string());
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dev_mcp = exe.ancestors().find_map(|p| {
        let candidate = p.join("mcp").join("server.py");
        candidate.exists().then(|| p.join("mcp"))
    });

    if let Some(path) = dev_mcp {
        return Ok(path.to_string_lossy().to_string());
    }

    Err("MCP server files not found".into())
}

#[tauri::command]
pub fn check_mcp_installed() -> Result<bool, String> {
    let claude_json = dirs::home_dir()
        .ok_or("cannot find home dir")?
        .join(".claude.json");

    if !claude_json.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&claude_json).map_err(|e| e.to_string())?;
    let parsed: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    Ok(parsed["mcpServers"]["portsage"].is_object())
}

/// Tools to allow in the user's `~/.claude/settings.json` when Portsage installs
/// the MCP. Must stay in sync with the methods exposed by `socket.rs` and the
/// tools defined in `mcp/server.py`.
pub(crate) const MCP_TOOL_PERMISSIONS: &[&str] = &[
    "mcp__portsage__list_all",
    "mcp__portsage__reserve_range",
    "mcp__portsage__register_port",
    "mcp__portsage__release_project",
    "mcp__portsage__remove_port",
    "mcp__portsage__list_unmanaged",
    "mcp__portsage__next_range",
    "mcp__portsage__get_config",
    "mcp__portsage__set_config",
    "mcp__portsage__scan_active",
    "mcp__portsage__kill_port",
    "mcp__portsage__kill_project",
    "mcp__portsage__open_in_browser",
    "mcp__portsage__find_project_by_path",
];

#[tauri::command]
pub fn install_mcp(mcp_dir: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot find home dir")?;
    let mcp_dir = std::path::PathBuf::from(&mcp_dir);

    // 1. Write MCP server config to ~/.claude.json
    let claude_json_path = home.join(".claude.json");
    let mut claude_json = parse_existing_or_empty(&claude_json_path)?;

    let mcp_dir_str = mcp_dir.to_string_lossy().to_string();
    claude_json["mcpServers"]["portsage"] = serde_json::json!({
        "type": "stdio",
        "command": "uv",
        "args": ["--directory", mcp_dir_str, "run", "python", "server.py"]
    });

    std::fs::write(
        &claude_json_path,
        serde_json::to_string_pretty(&claude_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    // 2. Install skill
    let skill_dir = home.join(".claude").join("skills").join("portsage");
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;

    let skill_source = mcp_dir.join("SKILL.md");
    let skill_dest = skill_dir.join("SKILL.md");
    std::fs::copy(&skill_source, &skill_dest).map_err(|e| e.to_string())?;

    // 3. Add tool permissions to ~/.claude/settings.json (same parse-or-bail policy as above)
    let settings_path = home.join(".claude").join("settings.json");
    let mut settings = parse_existing_or_empty(&settings_path)?;

    let allow = settings["permissions"]["allow"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut allow_set: Vec<String> = allow
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    for tool in MCP_TOOL_PERMISSIONS {
        if !allow_set.contains(&tool.to_string()) {
            allow_set.push(tool.to_string());
        }
    }
    settings["permissions"]["allow"] = serde_json::Value::Array(
        allow_set
            .into_iter()
            .map(serde_json::Value::String)
            .collect(),
    );

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

// === Remote backends (Phase 2) ===
//
// CRUD commands for the `remote_backends` table plus a `test_remote_backend`
// command that exercises the BackendRouter end-to-end. These commands sit on
// top of `BackendRouter`; the existing project/port commands above still talk
// to the local DB directly (the routing refactor lands in Phase 2.7).

/// Tauri event name emitted whenever a backend's tunnel state changes. The
/// payload is a `TunnelStatus`. The frontend listens on this to update the
/// status dot in the sidebar without polling.
pub const TUNNEL_EVENT: &str = "tunnel://state-changed";

#[tauri::command]
pub fn list_remote_backends(
    router: State<Arc<BackendRouter>>,
) -> Result<Vec<RemoteBackend>, String> {
    router
        .database()
        .list_remote_backends()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_remote_backend(
    router: State<Arc<BackendRouter>>,
    form: RemoteBackendForm,
) -> Result<RemoteBackend, String> {
    router
        .database()
        .create_remote_backend(form.as_input())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_remote_backend(
    router: State<Arc<BackendRouter>>,
    id: i64,
    form: RemoteBackendForm,
) -> Result<RemoteBackend, String> {
    router
        .database()
        .update_remote_backend(id, form.as_input())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_remote_backend(
    app: tauri::AppHandle,
    router: State<Arc<BackendRouter>>,
    id: i64,
) -> Result<(), String> {
    // Close any open tunnel first so we don't leak an ssh child for a backend
    // that no longer exists in the catalogue. Look up the row to get its name
    // before we delete it.
    let row = router
        .database()
        .list_remote_backends()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|b| b.id == id);
    if let Some(b) = row.as_ref() {
        router
            .manager()
            .close_tunnel(&b.name)
            .map_err(|e| e.to_string())?;
        let _ = app.emit(
            TUNNEL_EVENT,
            TunnelStatus {
                backend_name: b.name.clone(),
                ssh_alias: b.ssh_alias.clone(),
                remote_socket: b.remote_socket_path.clone(),
                local_socket: b.local_socket_path.clone(),
                state: crate::backends::TunnelState::Disconnected,
            },
        );
    }
    router
        .database()
        .delete_remote_backend(id)
        .map_err(|e| e.to_string())?;
    // If the user removed the backend they were viewing, snap back to Local.
    let current = router.current();
    if let BackendTarget::Remote { name } = &current {
        if let Some(b) = row {
            if &b.name == name {
                router
                    .set_current(BackendTarget::Local)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_remote_backend_auto_forward(
    router: State<Arc<BackendRouter>>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    router
        .database()
        .set_remote_backend_auto_forward(id, enabled)
        .map_err(|e| e.to_string())
}

/// Probe an existing backend end-to-end: ensure the tunnel is open, then
/// call `list_all` on the remote side. Returns the project count on success
/// and a verbatim error otherwise (so the UI can show "Permission denied"
/// or "Host key verification failed" rather than a generic "tunnel error").
/// Emits a `TUNNEL_EVENT` with the resulting state.
#[tauri::command]
pub fn test_remote_backend(
    app: tauri::AppHandle,
    router: State<Arc<BackendRouter>>,
    name: String,
) -> Result<usize, String> {
    let target = BackendTarget::Remote { name: name.clone() };
    let result = (|| -> Result<usize, String> {
        let client = router.client_for(&target).map_err(|e| e.to_string())?;
        let projects = client.list_all()?;
        Ok(projects.len())
    })();
    emit_tunnel_status(&app, &router, &name);
    result
}

#[tauri::command]
pub fn get_current_backend(router: State<Arc<BackendRouter>>) -> Result<BackendTarget, String> {
    Ok(router.current())
}

#[tauri::command]
pub fn set_current_backend(
    router: State<Arc<BackendRouter>>,
    target: BackendTarget,
) -> Result<(), String> {
    router.set_current(target).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_tunnel_statuses(router: State<Arc<BackendRouter>>) -> Result<Vec<TunnelStatus>, String> {
    Ok(router.manager().statuses())
}

#[tauri::command]
pub fn close_tunnel(
    app: tauri::AppHandle,
    router: State<Arc<BackendRouter>>,
    name: String,
) -> Result<(), String> {
    router
        .manager()
        .close_tunnel(&name)
        .map_err(|e| e.to_string())?;
    emit_tunnel_status(&app, &router, &name);
    Ok(())
}

/// Look up the current tunnel status for `name` and emit it via `TUNNEL_EVENT`.
/// Silent no-op if no entry exists yet - that's fine, the frontend would only
/// care about a state change, not the absence of one.
fn emit_tunnel_status(app: &tauri::AppHandle, router: &BackendRouter, name: &str) {
    if let Some(status) = router
        .manager()
        .statuses()
        .into_iter()
        .find(|s| s.backend_name == name)
    {
        let _ = app.emit(TUNNEL_EVENT, status);
    }
}

// === Forwards (Phase 3) ===

/// Tauri event emitted when one or more forwards change state. The payload
/// is a `Vec<ForwardStatus>` (a delta snapshot for the affected backend),
/// not a single entry, so the frontend can render the whole row in one
/// update.
pub const FORWARD_EVENT: &str = "forward://state-changed";

#[tauri::command]
pub fn list_forward_statuses(
    forwards: State<Arc<ForwardManager>>,
    backend: String,
) -> Result<Vec<ForwardStatus>, String> {
    Ok(forwards.statuses_for(&backend))
}

#[tauri::command]
pub fn enable_forward(
    app: tauri::AppHandle,
    forwards: State<Arc<ForwardManager>>,
    backend: String,
    port: i64,
) -> Result<ForwardStatus, String> {
    let result = forwards.enable(&backend, port).map_err(|e| e.to_string());
    let _ = app.emit(FORWARD_EVENT, forwards.statuses_for(&backend));
    result
}

#[tauri::command]
pub fn disable_forward(
    app: tauri::AppHandle,
    forwards: State<Arc<ForwardManager>>,
    backend: String,
    port: i64,
) -> Result<ForwardStatus, String> {
    let result = forwards.disable(&backend, port).map_err(|e| e.to_string());
    let _ = app.emit(FORWARD_EVENT, forwards.statuses_for(&backend));
    result
}

#[tauri::command]
pub fn sync_forwards(
    app: tauri::AppHandle,
    forwards: State<Arc<ForwardManager>>,
    backend: String,
) -> Result<Vec<ForwardStatus>, String> {
    let result = forwards.sync(&backend).map_err(|e| e.to_string());
    let _ = app.emit(FORWARD_EVENT, forwards.statuses_for(&backend));
    result
}

#[tauri::command]
pub fn list_forward_exclusions(
    router: State<Arc<BackendRouter>>,
    backend_id: i64,
) -> Result<Vec<ForwardExclusion>, String> {
    router
        .database()
        .list_forward_exclusions(backend_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_forward_exclusion(
    router: State<Arc<BackendRouter>>,
    backend_id: i64,
    port: i64,
) -> Result<ForwardExclusion, String> {
    router
        .database()
        .add_forward_exclusion(backend_id, port)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_forward_exclusion(router: State<Arc<BackendRouter>>, id: i64) -> Result<(), String> {
    router
        .database()
        .remove_forward_exclusion(id)
        .map_err(|e| e.to_string())
}

// (uninstall_mcp follows)

#[tauri::command]
pub fn uninstall_mcp() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot find home dir")?;

    // 1. Remove from ~/.claude.json
    let claude_json_path = home.join(".claude.json");
    if claude_json_path.exists() {
        let content = std::fs::read_to_string(&claude_json_path).map_err(|e| e.to_string())?;
        let mut parsed: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;
        if let Some(servers) = parsed["mcpServers"].as_object_mut() {
            servers.remove("portsage");
        }
        std::fs::write(
            &claude_json_path,
            serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }

    // 2. Remove skill
    let skill_dir = home.join(".claude").join("skills").join("portsage");
    let _ = std::fs::remove_dir_all(&skill_dir);

    // 3. Remove permissions
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?;
        let mut settings: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;
        if let Some(allow) = settings["permissions"]["allow"].as_array_mut() {
            allow.retain(|v| {
                v.as_str()
                    .map(|s| !s.starts_with("mcp__portsage__"))
                    .unwrap_or(true)
            });
        }
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_existing_or_empty ---

    #[test]
    fn parse_existing_or_empty_returns_empty_object_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.json");
        let result = parse_existing_or_empty(&path).unwrap();
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn parse_existing_or_empty_parses_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"mcpServers": {"foo": {"command": "bar"}}}"#).unwrap();
        let result = parse_existing_or_empty(&path).unwrap();
        assert_eq!(result["mcpServers"]["foo"]["command"], "bar");
    }

    #[test]
    fn parse_existing_or_empty_bails_on_malformed_json() {
        // Safety-critical: a broken claude.json must not be silently replaced with {}.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.json");
        std::fs::write(&path, "{ this is not valid json").unwrap();
        let err = parse_existing_or_empty(&path).unwrap_err();
        assert!(
            err.contains("appears to be corrupt"),
            "expected 'corrupt' message, got: {err}",
        );
        assert!(
            err.contains("Refusing to overwrite"),
            "expected refusal message, got: {err}",
        );
        assert!(
            err.contains(&path.display().to_string()),
            "expected the path to be mentioned in the error, got: {err}",
        );
    }

    #[test]
    fn parse_existing_or_empty_handles_empty_file_as_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.json");
        std::fs::write(&path, "").unwrap();
        let err = parse_existing_or_empty(&path).unwrap_err();
        assert!(err.contains("appears to be corrupt"));
    }

    #[test]
    fn parse_existing_or_empty_accepts_empty_object() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty-obj.json");
        std::fs::write(&path, "{}").unwrap();
        let result = parse_existing_or_empty(&path).unwrap();
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn mcp_tool_permissions_cover_socket_methods() {
        // The MCP permissions list and the methods dispatched by the socket
        // must stay in lockstep. The set below is the contract surfaced in
        // SKILL.md and registered into ~/.claude/settings.json on install.
        let names: Vec<&str> = MCP_TOOL_PERMISSIONS
            .iter()
            .map(|s| s.trim_start_matches("mcp__portsage__"))
            .collect();
        for expected in [
            "list_all",
            "reserve_range",
            "register_port",
            "release_project",
            "remove_port",
            "list_unmanaged",
            "next_range",
            "get_config",
            "set_config",
            "scan_active",
            "kill_port",
            "kill_project",
            "open_in_browser",
            "find_project_by_path",
        ] {
            assert!(
                names.contains(&expected),
                "MCP_TOOL_PERMISSIONS missing tool: {expected}",
            );
        }
    }
}
