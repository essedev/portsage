use crate::actions::{
    self, KillOutcome, PortStatus, ProjectStatus,
};
use crate::db::Database;
use crate::scanner::{scan_active_ports, ActivePort};
use std::path::Path;
use std::sync::Arc;
use tauri::{Manager, State};

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

#[tauri::command]
pub fn list_projects(db: State<Arc<Database>>) -> Result<Vec<ProjectStatus>, String> {
    actions::list_with_status(&db)
}

#[tauri::command]
pub fn create_project(
    db: State<Arc<Database>>,
    name: String,
    path: Option<String>,
) -> Result<ProjectStatus, String> {
    let project = db
        .create_project(&name, path.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(ProjectStatus {
        id: project.id,
        name: project.name,
        path: project.path,
        range_start: project.range_start,
        range_end: project.range_end,
        created_at: project.created_at,
        ports: Vec::new(),
    })
}

#[tauri::command]
pub fn delete_project(db: State<Arc<Database>>, id: i64) -> Result<(), String> {
    db.delete_project(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_port(
    db: State<Arc<Database>>,
    project_id: i64,
    service: String,
    port: i64,
) -> Result<PortStatus, String> {
    let p = db
        .add_port(project_id, &service, port)
        .map_err(|e| e.to_string())?;
    let active = scan_active_ports();
    Ok(PortStatus {
        active: active.contains(&p.port),
        process: None,
        pid: None,
        id: p.id,
        project_id: p.project_id,
        service: p.service,
        port: p.port,
        created_at: p.created_at,
    })
}

#[tauri::command]
pub fn remove_port(db: State<Arc<Database>>, id: i64) -> Result<(), String> {
    db.remove_port(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scan_ports() -> Vec<i64> {
    actions::scan_active_port_numbers()
}

#[tauri::command]
pub fn list_unmanaged_ports(db: State<Arc<Database>>) -> Result<Vec<ActivePort>, String> {
    actions::list_unmanaged(&db)
}

#[tauri::command]
pub fn get_next_range(db: State<Arc<Database>>) -> Result<(i64, i64), String> {
    actions::next_range(&db)
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
pub fn open_in_browser(port: i64) -> Result<(), String> {
    actions::open_in_browser(port)
}

#[tauri::command]
pub async fn kill_port(port: i64) -> Result<KillOutcome, String> {
    Ok(actions::kill_port_action(port).await)
}

#[tauri::command]
pub async fn kill_project(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<Vec<(i64, KillOutcome)>, String> {
    actions::kill_project_action(&db, project_id).await
}

#[tauri::command]
pub fn get_config(db: State<Arc<Database>>) -> Result<serde_json::Value, String> {
    let (base_port, range_size) = actions::get_config(&db)?;
    Ok(serde_json::json!({
        "base_port": base_port,
        "range_size": range_size,
    }))
}

#[tauri::command]
pub fn set_config(
    db: State<Arc<Database>>,
    key: String,
    value: String,
) -> Result<(), String> {
    actions::set_config(&db, &key, &value)
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

    zip.start_file("portsage.db", options).map_err(|e| e.to_string())?;
    let db_bytes = std::fs::read(&db_path).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip, &db_bytes).map_err(|e| e.to_string())?;

    zip.start_file("config.json", options).map_err(|e| e.to_string())?;
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
    let dev_mcp = exe
        .ancestors()
        .find_map(|p| {
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
    let parsed: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

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
    settings["permissions"]["allow"] =
        serde_json::Value::Array(allow_set.into_iter().map(serde_json::Value::String).collect());

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
