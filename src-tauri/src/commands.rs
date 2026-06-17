use crate::actions::{KillOutcome, PortStatus, ProjectStatus};
use crate::backends::{BackendRouter, BackendTarget, RemoteBackendForm, TunnelStatus};
use crate::db::{Database, ForwardExclusion, RemoteBackend};
use crate::forwards::{ForwardManager, ForwardStatus};
use crate::paths;
use crate::scanner::ActivePort;
use portsage_client::{ConfigSnapshot, KillEntry, RangeBounds};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

// === Project & port commands ===
//
// These dispatch via `BackendRouter::client()` so the active backend (Local
// or one of the Remote ones configured in Settings) drives every read and
// write. Mutating commands take the project *name* directly (the frontend
// already has it from `list_projects`), so the backend doesn't have to do
// an id->name round-trip on every write. Names are the canonical handle in
// the wire protocol anyway, and ids would not match across backends.

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
pub fn delete_project(router: State<Arc<BackendRouter>>, name: String) -> Result<(), String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.release_project(&name)
}

#[tauri::command]
pub fn update_project(
    router: State<Arc<BackendRouter>>,
    current_name: String,
    new_name: Option<String>,
    new_path: Option<String>,
) -> Result<ProjectStatus, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.update_project(&current_name, new_name.as_deref(), new_path.as_deref())
}

#[tauri::command]
pub fn add_port(
    router: State<Arc<BackendRouter>>,
    project_name: String,
    service: String,
    port: i64,
) -> Result<PortStatus, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.register_port(&project_name, &service, port)
}

#[tauri::command]
pub fn remove_port(
    router: State<Arc<BackendRouter>>,
    project_name: String,
    service: String,
) -> Result<(), String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.remove_port(&project_name, &service)
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
    project_name: String,
) -> Result<Vec<KillEntry>, String> {
    let client = router.client().map_err(|e| e.to_string())?;
    client.kill_project(&project_name).await
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

    // Propagate config errors instead of substituting defaults: a backup that
    // silently drops the user's tuning is worse than a failed export.
    zip.start_file("config.json", options)
        .map_err(|e| e.to_string())?;
    let base_port = db
        .get_config("base_port")
        .map_err(|e| format!("read base_port: {e}"))?;
    let range_size = db
        .get_config("range_size")
        .map_err(|e| format!("read range_size: {e}"))?;
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
pub fn import_data(db: State<Arc<Database>>, source_path: String) -> Result<(), String> {
    let db_path = Database::db_path();

    // 1. Extract the .portsage zip's portsage.db into a temp file *next to*
    //    the target so the final rename stays on the same filesystem and is
    //    atomic. A failure here leaves the existing DB untouched.
    let file = std::fs::File::open(&source_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut db_file = archive
        .by_name("portsage.db")
        .map_err(|e| format!("archive missing portsage.db: {e}"))?;
    let mut db_bytes = Vec::new();
    std::io::Read::read_to_end(&mut db_file, &mut db_bytes).map_err(|e| e.to_string())?;
    drop(db_file);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp_path = db_path.with_extension("portsage-import-tmp");
    std::fs::write(&tmp_path, &db_bytes).map_err(|e| e.to_string())?;

    // 2. Validate: open the temp file as SQLite and run integrity_check. A
    //    bogus or truncated file fails here instead of corrupting the live DB.
    let validate = (|| -> rusqlite::Result<String> {
        let conn = rusqlite::Connection::open(&tmp_path)?;
        conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
    })();
    match validate {
        Ok(s) if s == "ok" => {}
        Ok(s) => {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("imported database failed integrity check: {s}"));
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("imported file is not a valid SQLite database: {e}"));
        }
    }

    // 3. Swap in. Use rename so the cutover is atomic on POSIX.
    std::fs::rename(&tmp_path, &db_path).map_err(|e| e.to_string())?;

    // 4. Reopen the running connection so the UI observes the imported state
    //    immediately - without this, the in-memory Connection still points at
    //    the old (now-replaced) inode and queries continue to return stale rows.
    db.reopen().map_err(|e| e.to_string())?;

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

/// Resolve the MCP install dir, populating it from bundled `.dmg` resources
/// when available so app upgrades (`brew upgrade`) propagate fixes to
/// `server.py` / `SKILL.md` for users with a pre-existing install.
#[tauri::command]
pub fn get_mcp_dir(app: tauri::AppHandle) -> Result<String, String> {
    let install_dir = paths::mcp_install_dir();

    let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
    let bundled_mcp = resource_dir.join("mcp");
    if bundled_mcp.join("server.py").exists() {
        std::fs::create_dir_all(&install_dir).map_err(|e| e.to_string())?;
        for file in &["server.py", "pyproject.toml", "uv.lock", "SKILL.md"] {
            let src = bundled_mcp.join(file);
            let dst = install_dir.join(file);
            if src.exists() {
                std::fs::copy(&src, &dst).map_err(|e| e.to_string())?;
            }
        }
        return Ok(install_dir.to_string_lossy().to_string());
    }

    if install_dir.join("server.py").exists() {
        return Ok(install_dir.to_string_lossy().to_string());
    }

    // Dev fallback: source tree's `mcp/` next to the running binary.
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
    let path = portsage_mcp::Scope::Global
        .config_path()
        .map_err(|e| e.to_string())?;
    if !path.exists() {
        return Ok(false);
    }
    let parsed = portsage_mcp::parse_existing_or_empty(&path).map_err(|e| e.to_string())?;
    Ok(parsed["mcpServers"]["portsage"].is_object())
}

#[tauri::command]
pub fn install_mcp(mcp_dir: String) -> Result<(), String> {
    let mcp_dir = std::path::PathBuf::from(&mcp_dir);
    portsage_mcp::register_in_claude(portsage_mcp::Scope::Global, &mcp_dir)
        .map_err(|e| e.to_string())?;
    portsage_mcp::install_skill(&mcp_dir).map_err(|e| e.to_string())?;
    portsage_mcp::add_permissions().map_err(|e| e.to_string())?;
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
    portsage_mcp::unregister_from_claude(portsage_mcp::Scope::Global)
        .map_err(|e| e.to_string())?;
    portsage_mcp::remove_skill().map_err(|e| e.to_string())?;
    portsage_mcp::remove_permissions().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn mcp_tool_permissions_cover_socket_methods() {
        // The MCP allowlist installed by Portsage and the methods dispatched
        // by the socket layer must stay in lockstep. This test asserts parity
        // against the shared `portsage_mcp::MCP_TOOL_PERMISSIONS` constant.
        let names: Vec<&str> = portsage_mcp::MCP_TOOL_PERMISSIONS
            .iter()
            .map(|s| s.trim_start_matches("mcp__portsage__"))
            .collect();
        for expected in [
            "list_all",
            "reserve_range",
            "update_project",
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
