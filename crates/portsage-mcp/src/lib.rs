//! Shared MCP install/uninstall/status logic for both the Portsage CLI
//! (`portsage mcp install`) and the Tauri app (Settings > "Configure MCP").
//!
//! Both consumers extract the MCP source files differently - the CLI bundles
//! them via `include_str!`, the Tauri app reads them from its `.dmg` resource
//! dir - so file extraction is **not** part of this crate. Everything that
//! happens *after* the four files are on disk lives here:
//!
//!   - register / unregister `portsage` under `mcpServers` in the user's
//!     Claude config (`~/.claude.json` global, or `./.mcp.json` project),
//!   - install / remove the SKILL.md under `~/.claude/skills/portsage/`,
//!   - add / remove the allowlist entries in `~/.claude/settings.json`,
//!   - `status()` snapshot of all the above.
//!
//! All JSON writes go through a parse-or-bail + atomic-tmp-then-rename helper
//! so a corrupt config is never silently overwritten and a kill mid-write
//! never leaves the user with a half-truncated file.

use std::path::{Path, PathBuf};

/// Tools we add to the Claude Code allowlist on install. Must stay in sync
/// with the methods exposed by `src-tauri/src/socket.rs` and the tools
/// defined in `mcp/server.py`. The matching test in `src-tauri` asserts
/// parity with the socket dispatcher.
pub const MCP_TOOL_PERMISSIONS: &[&str] = &[
    "mcp__portsage__list_all",
    "mcp__portsage__reserve_range",
    "mcp__portsage__update_project",
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

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// The user's Claude config file exists but is not valid JSON. We refuse
    /// to overwrite it rather than silently clobbering their setup.
    #[error(
        "{path} appears to be corrupt and cannot be parsed: {reason}. \
         Refusing to overwrite. Fix or back up the file manually before retrying."
    )]
    CorruptConfig { path: PathBuf, reason: String },

    #[error("cannot find home directory ($HOME is unset)")]
    NoHome,
}

/// Where to register the MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Global: `~/.claude.json` (the default - applies to every Claude Code
    /// project).
    Global,
    /// Project-local: `./.mcp.json` next to the user's current working dir.
    Project,
}

impl Scope {
    pub fn config_path(&self) -> Result<PathBuf, McpError> {
        match self {
            Scope::Global => Ok(home_dir()?.join(".claude.json")),
            Scope::Project => Ok(std::env::current_dir()?.join(".mcp.json")),
        }
    }
}

pub fn home_dir() -> Result<PathBuf, McpError> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(McpError::NoHome)
}

/// `~/.claude/skills/portsage/`.
pub fn skills_dir() -> Result<PathBuf, McpError> {
    Ok(home_dir()?.join(".claude").join("skills").join("portsage"))
}

/// `~/.claude/settings.json`.
pub fn settings_path() -> Result<PathBuf, McpError> {
    Ok(home_dir()?.join(".claude").join("settings.json"))
}

/// Read and parse a JSON file, returning `{}` if it doesn't exist. If the
/// file exists but is malformed, return [`McpError::CorruptConfig`] rather
/// than silently overwriting it - the Claude config file holds the user's
/// entire conversation index and we must never clobber it.
pub fn parse_existing_or_empty(path: &Path) -> Result<serde_json::Value, McpError> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(|e| McpError::CorruptConfig {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Atomic write via tmp + rename. Prevents leaving a half-written
/// `~/.claude.json` if the process is killed mid-write.
pub fn write_json_atomically(path: &Path, value: &serde_json::Value) -> Result<(), McpError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("portsage-tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// JSON object representing the `mcpServers.portsage` entry.
pub fn mcp_server_entry(mcp_dir: &Path) -> serde_json::Value {
    serde_json::json!({
        "type": "stdio",
        "command": "uv",
        "args": ["--directory", mcp_dir.to_string_lossy(), "run", "python", "server.py"],
    })
}

/// Add the `portsage` MCP server entry under `mcpServers` in the target
/// Claude config file. Returns the path that was written.
pub fn register_in_claude(scope: Scope, mcp_dir: &Path) -> Result<PathBuf, McpError> {
    let path = scope.config_path()?;
    let mut cfg = parse_existing_or_empty(&path)?;
    if !cfg.is_object() {
        cfg = serde_json::json!({});
    }
    cfg["mcpServers"]["portsage"] = mcp_server_entry(mcp_dir);
    write_json_atomically(&path, &cfg)?;
    Ok(path)
}

/// Inverse of [`register_in_claude`]. Returns `true` if the entry existed and
/// was removed.
pub fn unregister_from_claude(scope: Scope) -> Result<bool, McpError> {
    let path = scope.config_path()?;
    if !path.exists() {
        return Ok(false);
    }
    let mut cfg = parse_existing_or_empty(&path)?;
    let removed = cfg
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .map(|m| m.remove("portsage").is_some())
        .unwrap_or(false);
    if removed {
        write_json_atomically(&path, &cfg)?;
    }
    Ok(removed)
}

/// Copy the SKILL.md into `~/.claude/skills/portsage/SKILL.md`.
pub fn install_skill(mcp_dir: &Path) -> Result<PathBuf, McpError> {
    let dest = skills_dir()?;
    std::fs::create_dir_all(&dest)?;
    let dest_file = dest.join("SKILL.md");
    std::fs::copy(mcp_dir.join("SKILL.md"), &dest_file)?;
    Ok(dest_file)
}

/// Remove `~/.claude/skills/portsage/`. Returns `true` if it existed.
pub fn remove_skill() -> Result<bool, McpError> {
    let dir = skills_dir()?;
    if !dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(true)
}

/// Add the portsage MCP tools to the `permissions.allow` list in
/// `~/.claude/settings.json`. Idempotent. Returns the path that was written.
pub fn add_permissions() -> Result<PathBuf, McpError> {
    let path = settings_path()?;
    let mut settings = parse_existing_or_empty(&path)?;
    if !settings.is_object() {
        settings = serde_json::json!({});
    }

    let mut allow: Vec<String> = settings["permissions"]["allow"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    for tool in MCP_TOOL_PERMISSIONS {
        if !allow.iter().any(|s| s == tool) {
            allow.push((*tool).to_string());
        }
    }
    settings["permissions"]["allow"] =
        serde_json::Value::Array(allow.into_iter().map(serde_json::Value::String).collect());

    write_json_atomically(&path, &settings)?;
    Ok(path)
}

/// Remove the portsage MCP tools from `permissions.allow`. Other tools the
/// user added are preserved. Returns the number of entries removed.
pub fn remove_permissions() -> Result<usize, McpError> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(0);
    }
    let mut settings = parse_existing_or_empty(&path)?;
    let Some(arr) = settings["permissions"]["allow"].as_array().cloned() else {
        return Ok(0);
    };
    let before = arr.len();
    let kept: Vec<serde_json::Value> = arr
        .into_iter()
        .filter(|v| {
            v.as_str()
                .map(|s| !MCP_TOOL_PERMISSIONS.contains(&s))
                .unwrap_or(true)
        })
        .collect();
    let removed = before - kept.len();
    if removed == 0 {
        return Ok(0);
    }
    settings["permissions"]["allow"] = serde_json::Value::Array(kept);
    write_json_atomically(&path, &settings)?;
    Ok(removed)
}

/// Snapshot of the current MCP install state, for the `status` subcommand
/// and for the GUI's Settings panel.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct McpStatus {
    pub mcp_dir: String,
    pub files_present: bool,
    pub registered_global: bool,
    pub registered_project_cwd: bool,
    pub skill_installed: bool,
    pub allowlist_has_portsage: bool,
}

/// Build a status snapshot. `mcp_dir` is the directory where the embedded MCP
/// files should live - the caller decides where that is (CLI uses the data
/// dir; Tauri uses the same on macOS).
pub fn status(mcp_dir: &Path) -> Result<McpStatus, McpError> {
    let files_present = mcp_dir.join("server.py").exists();

    let global = Scope::Global.config_path()?;
    let registered_global = read_has_portsage_server(&global)?;

    let project_cwd = Scope::Project.config_path()?;
    let registered_project_cwd = read_has_portsage_server(&project_cwd)?;

    let skill_installed = skills_dir()?.join("SKILL.md").exists();

    let allowlist_path = settings_path()?;
    let allowlist_has_portsage = if allowlist_path.exists() {
        let v = parse_existing_or_empty(&allowlist_path)?;
        v["permissions"]["allow"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .any(|s| s.starts_with("mcp__portsage__"))
            })
            .unwrap_or(false)
    } else {
        false
    };

    Ok(McpStatus {
        mcp_dir: mcp_dir.to_string_lossy().to_string(),
        files_present,
        registered_global,
        registered_project_cwd,
        skill_installed,
        allowlist_has_portsage,
    })
}

fn read_has_portsage_server(path: &Path) -> Result<bool, McpError> {
    if !path.exists() {
        return Ok(false);
    }
    let v = parse_existing_or_empty(path)?;
    Ok(v["mcpServers"]["portsage"].is_object())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_existing_or_empty_returns_object_for_missing() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("missing.json");
        let v = parse_existing_or_empty(&p).unwrap();
        assert!(v.is_object());
    }

    #[test]
    fn parse_existing_or_empty_refuses_corrupt() {
        // Safety-critical: a broken claude.json must not be silently replaced
        // with {}.
        let dir = tempdir().unwrap();
        let p = dir.path().join("corrupt.json");
        std::fs::write(&p, "{not json").unwrap();
        let err = parse_existing_or_empty(&p).unwrap_err();
        assert!(matches!(err, McpError::CorruptConfig { .. }));
    }

    #[test]
    fn write_json_atomically_creates_parent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a/b/c/file.json");
        write_json_atomically(&path, &serde_json::json!({"x": 1})).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["x"], 1);
    }

    /// Exercise the JSON merge against a synthetic Claude config: (a) preserve
    /// the user's other entries and (b) overwrite an existing portsage entry
    /// rather than duplicating it.
    #[test]
    fn registers_portsage_under_mcp_servers_preserving_siblings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "version": "1.2.3",
                "mcpServers": {
                    "other-server": {"type": "stdio", "command": "x"}
                },
                "history": [{"id": 1}]
            }))
            .unwrap(),
        )
        .unwrap();

        let mut cfg = parse_existing_or_empty(&path).unwrap();
        cfg["mcpServers"]["portsage"] = mcp_server_entry(Path::new("/tmp/mcp"));
        write_json_atomically(&path, &cfg).unwrap();

        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk["version"], "1.2.3");
        assert!(on_disk["mcpServers"]["other-server"].is_object());
        assert_eq!(on_disk["mcpServers"]["portsage"]["command"], "uv");
        assert_eq!(on_disk["history"][0]["id"], 1);
    }

    #[test]
    fn remove_permissions_drops_only_portsage_entries() {
        // The real `remove_permissions` reads `$HOME`; we exercise the same
        // logic inline against an arbitrary path so the test stays hermetic.
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "permissions": {
                    "allow": [
                        "Bash(ls)",
                        "mcp__portsage__list_all",
                        "mcp__portsage__kill_port",
                        "Bash(grep)"
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let mut settings = parse_existing_or_empty(&path).unwrap();
        let arr = settings["permissions"]["allow"]
            .as_array()
            .cloned()
            .unwrap();
        let before = arr.len();
        let kept: Vec<serde_json::Value> = arr
            .into_iter()
            .filter(|v| {
                v.as_str()
                    .map(|s| !MCP_TOOL_PERMISSIONS.contains(&s))
                    .unwrap_or(true)
            })
            .collect();
        let removed = before - kept.len();
        settings["permissions"]["allow"] = serde_json::Value::Array(kept);
        write_json_atomically(&path, &settings).unwrap();

        assert_eq!(removed, 2);
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let allow = on_disk["permissions"]["allow"].as_array().unwrap();
        let strs: Vec<&str> = allow.iter().filter_map(|v| v.as_str()).collect();
        assert_eq!(strs, vec!["Bash(ls)", "Bash(grep)"]);
    }
}
