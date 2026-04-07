use crate::db::{Database, ProjectWithPorts};
use crate::scanner::{self, scan_active_ports, ActivePort};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Arc;
use tauri::{Manager, State};

#[derive(Debug, Serialize)]
pub struct PortStatus {
    pub id: i64,
    pub project_id: i64,
    pub service: String,
    pub port: i64,
    pub active: bool,
    pub process: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectStatus {
    pub id: i64,
    pub name: String,
    pub path: Option<String>,
    pub range_start: i64,
    pub range_end: i64,
    pub created_at: String,
    pub ports: Vec<PortStatus>,
}

fn enrich_with_status(
    projects: Vec<ProjectWithPorts>,
    active_ports: &[ActivePort],
) -> Vec<ProjectStatus> {
    use std::collections::HashMap;
    let port_map: HashMap<i64, &ActivePort> = active_ports
        .iter()
        .map(|ap| (ap.port, ap))
        .collect();

    projects
        .into_iter()
        .map(|pwp| ProjectStatus {
            id: pwp.project.id,
            name: pwp.project.name,
            path: pwp.project.path,
            range_start: pwp.project.range_start,
            range_end: pwp.project.range_end,
            created_at: pwp.project.created_at,
            ports: pwp
                .ports
                .into_iter()
                .map(|p| {
                    let ap = port_map.get(&p.port);
                    PortStatus {
                        active: ap.is_some(),
                        process: ap.map(|a| a.process.clone()),
                        id: p.id,
                        project_id: p.project_id,
                        service: p.service,
                        port: p.port,
                        created_at: p.created_at,
                    }
                })
                .collect(),
        })
        .collect()
}

#[tauri::command]
pub fn list_projects(db: State<Arc<Database>>) -> Result<Vec<ProjectStatus>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let active = scanner::scan_active_ports_detailed();
    Ok(enrich_with_status(projects, &active))
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
    let mut ports: Vec<i64> = scan_active_ports().into_iter().collect();
    ports.sort();
    ports
}

#[tauri::command]
pub fn list_unmanaged_ports(db: State<Arc<Database>>) -> Result<Vec<ActivePort>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let registered: HashSet<i64> = projects
        .iter()
        .flat_map(|p| p.ports.iter().map(|port| port.port))
        .collect();
    let mut unmanaged = scanner::scan_unmanaged_ports(&registered);
    unmanaged.sort_by_key(|p| p.port);
    Ok(unmanaged)
}

#[tauri::command]
pub fn get_next_range(db: State<Arc<Database>>) -> Result<(i64, i64), String> {
    db.next_available_range().map_err(|e| e.to_string())
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
pub fn get_config(db: State<Arc<Database>>) -> Result<serde_json::Value, String> {
    let base_port = db.get_config("base_port").map_err(|e| e.to_string())?;
    let range_size = db.get_config("range_size").map_err(|e| e.to_string())?;
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
    db.set_config(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_data(db: State<Arc<Database>>, dest_path: String) -> Result<(), String> {
    let db_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("portsage")
        .join("portsage.db");

    if !db_path.exists() {
        return Err("Database not found".into());
    }

    // Create a zip with the db
    let file = std::fs::File::create(&dest_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Add database
    zip.start_file("portsage.db", options).map_err(|e| e.to_string())?;
    let db_bytes = std::fs::read(&db_path).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip, &db_bytes).map_err(|e| e.to_string())?;

    // Add config as JSON
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
    let db_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("portsage")
        .join("portsage.db");

    let file = std::fs::File::open(&source_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    // Extract database
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

    // No bundled resources (dev mode without resource_dir, or unusual install): if the
    // user already has files in the config dir, use them as-is.
    if config_mcp.join("server.py").exists() {
        return Ok(config_mcp.to_string_lossy().to_string());
    }

    // Dev mode: resolve from executable location, walking up to find a sibling mcp dir.
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

#[tauri::command]
pub fn install_mcp(mcp_dir: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot find home dir")?;
    let mcp_dir = std::path::PathBuf::from(&mcp_dir);

    // 1. Write MCP server config to ~/.claude.json
    // If the file already exists we MUST be able to parse it. Falling back to {} on parse
    // failure would silently destroy the user's entire Claude config (other MCP servers,
    // settings, history, etc.) so we bail with a clear error instead.
    let claude_json_path = home.join(".claude.json");
    let mut claude_json: serde_json::Value = if claude_json_path.exists() {
        let content = std::fs::read_to_string(&claude_json_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| {
            format!(
                "{} appears to be corrupt and cannot be parsed: {}. Refusing to overwrite. \
                 Fix or back up the file manually before retrying.",
                claude_json_path.display(),
                e
            )
        })?
    } else {
        serde_json::json!({})
    };

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
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| {
            format!(
                "{} appears to be corrupt and cannot be parsed: {}. Refusing to overwrite. \
                 Fix or back up the file manually before retrying.",
                settings_path.display(),
                e
            )
        })?
    } else {
        serde_json::json!({})
    };

    let tools = vec![
        "mcp__portsage__list_all",
        "mcp__portsage__reserve_range",
        "mcp__portsage__register_port",
        "mcp__portsage__release_project",
        "mcp__portsage__scan_active",
    ];

    let allow = settings["permissions"]["allow"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut allow_set: Vec<String> = allow
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    for tool in &tools {
        if !allow_set.contains(&tool.to_string()) {
            allow_set.push(tool.to_string());
        }
    }
    settings["permissions"]["allow"] =
        serde_json::Value::Array(allow_set.into_iter().map(serde_json::Value::String).collect());

    std::fs::create_dir_all(settings_path.parent().unwrap()).map_err(|e| e.to_string())?;
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
