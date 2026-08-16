//! Pure operations on the Database + system, shared between Tauri commands
//! and the Unix socket handlers. No tauri::* dependencies - everything in
//! here must be callable from a plain async/sync Rust context so the CLI
//! (via the socket) and the GUI (via Tauri commands) get the same behavior
//! out of a single source of truth.
//!
//! Error style: `Result<T, String>` on purpose. Both consumers (Tauri commands
//! and the socket wire protocol) ship errors as strings to clients that
//! pattern-match on the *message*, not on a typed variant - the frontend's
//! `humanizeError` and the MCP server are the existing consumers, plus the
//! socket protocol freezes the message shape as part of its contract. Adding
//! a `thiserror` enum here would be transformed back into a string at every
//! call site without anyone differentiating on the variants, so it stays
//! stringly-typed deliberately. Submodules that *do* have differentiable
//! failure modes worth typing (`forwards::ForwardError`, `backends`'
//! `BackendError`, the CLI's `CliError`) introduce their own typed errors.

use crate::db::{Database, ProjectWithPorts};
use crate::scanner::{self, ActivePort};
use std::collections::HashSet;

// Wire types live in portsage-client so the CLI and the app speak the same
// language without drift. Re-exported here for the Tauri command layer.
pub use portsage_client::{
    KillOutcome, PortStatus, ProjectStatus, RestoreOutcome, StaleProject, StaleReason, TrashEntry,
};

/// 2 seconds is the empirical sweet spot: enough for Postgres-class daemons
/// to flush and exit cleanly, short enough that the UI doesn't feel stuck.
pub const KILL_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// Process names whose PID is a Docker port-forwarding proxy. On macOS every
/// published container port shows up in `lsof -iTCP -sTCP:LISTEN` as one of
/// these PIDs - SIGTERM'ing it would tear down every container at once, so
/// the action layer detects them and re-routes via `docker stop`.
const DOCKER_PROXY_PROCESSES: &[&str] = &[
    // Modern Docker Desktop on macOS.
    "com.docker.backend",
    "Docker",
    // Legacy Docker for Mac.
    "vpnkit",
    "com.docker.vpnkit",
    // Linux native docker engine.
    "docker-proxy",
];

/// Absolute locations to probe for the docker CLI when `PATH` does not
/// resolve it. This is not belt-and-braces: a macOS app bundle launched by
/// launchd (Finder, login item, tray) inherits `PATH=/usr/bin:/bin:/usr/sbin:/sbin`
/// because `launchctl getenv PATH` is unset on a stock system, and none of
/// those four directories holds a docker binary. Without this list every
/// container-port kill issued from the UI fails while the same kill from a
/// terminal succeeds, which is exactly the bug this list fixes.
const DOCKER_BIN_CANDIDATES: &[&str] = &[
    // Docker Desktop's symlink on macOS, and the usual spot on Linux.
    "/usr/local/bin/docker",
    // Homebrew on Apple Silicon (colima / docker-cli formula).
    "/opt/homebrew/bin/docker",
    // Docker Desktop's real binary, in case the symlink was never created.
    "/Applications/Docker.app/Contents/Resources/bin/docker",
    // OrbStack.
    "/Applications/OrbStack.app/Contents/MacOS/xbin/docker",
    // Distro packages.
    "/usr/bin/docker",
    "/snap/bin/docker",
];

/// Env var to point Portsage at a specific docker CLI, for setups the
/// candidate list cannot guess (podman shim, custom prefix).
const DOCKER_BIN_ENV: &str = "PORTSAGE_DOCKER_BIN";

pub fn enrich_with_status(
    projects: Vec<ProjectWithPorts>,
    active_ports: &[ActivePort],
) -> Vec<ProjectStatus> {
    use std::collections::HashMap;
    let port_map: HashMap<i64, &ActivePort> = active_ports.iter().map(|ap| (ap.port, ap)).collect();

    projects
        .into_iter()
        .map(|pwp| ProjectStatus {
            id: pwp.project.id,
            name: pwp.project.name,
            path: pwp.project.path,
            range_start: pwp.project.range_start,
            range_end: pwp.project.range_end,
            created_at: pwp.project.created_at,
            archived_at: pwp.project.archived_at,
            ports: pwp
                .ports
                .into_iter()
                .map(|p| {
                    let ap = port_map.get(&p.port);
                    PortStatus {
                        active: ap.is_some(),
                        process: ap.map(|a| a.process.clone()),
                        pid: ap.map(|a| a.pid),
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

pub fn list_with_status(db: &Database) -> Result<Vec<ProjectStatus>, String> {
    let active = scanner::scan_active_ports_detailed();
    // Self-healing: an archived project whose port is listening again is
    // being worked on, so it comes back to the main list on its own. Done
    // here because this is the one path that sees both the registry and the
    // live scan, and it runs on every refresh.
    let ports: Vec<i64> = active.iter().map(|ap| ap.port).collect();
    db.unarchive_projects_with_active_ports(&ports)
        .map_err(|e| e.to_string())?;
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    Ok(enrich_with_status(projects, &active))
}

/// Default inactivity window for `prune`. Ninety days is the point where a
/// project is plausibly finished rather than merely paused: on a real
/// registry it selects a handful of candidates you can judge at a glance,
/// where 60 days already sweeps in work you are between sprints on.
pub const DEFAULT_STALE_DAYS: i64 = 90;

/// Projects that look abandoned: their path is gone, or nothing has touched
/// them in `days` days. Anything with a port listening right now is excluded
/// whatever its age, which is what makes acting on this list safe. Already
/// archived projects are excluded too - they are the ones you have dealt
/// with already.
pub fn list_stale(db: &Database, days: i64) -> Result<Vec<StaleProject>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let active: HashSet<i64> = scanner::scan_active_ports_detailed()
        .into_iter()
        .map(|ap| ap.port)
        .collect();
    Ok(stale_from(
        projects,
        &active,
        days,
        std::time::SystemTime::now(),
    ))
}

/// The selection itself, with the scan and the clock passed in so it can be
/// tested without touching either.
pub fn stale_from(
    projects: Vec<ProjectWithPorts>,
    active: &HashSet<i64>,
    days: i64,
    now: std::time::SystemTime,
) -> Vec<StaleProject> {
    let mut out = Vec::new();
    for pwp in projects {
        if pwp.project.archived_at.is_some() {
            continue;
        }
        if pwp.ports.iter().any(|p| active.contains(&p.port)) {
            continue;
        }
        let (reason, inactive_days) =
            match crate::activity::classify(pwp.project.path.as_deref(), now) {
                crate::activity::Activity::PathMissing => (StaleReason::PathMissing, None),
                crate::activity::Activity::Days(d) if d >= days => (StaleReason::Inactive, Some(d)),
                // Recently touched, or no path to measure: not a candidate.
                _ => continue,
            };
        out.push(StaleProject {
            name: pwp.project.name,
            path: pwp.project.path,
            range_start: pwp.project.range_start,
            range_end: pwp.project.range_end,
            reason,
            inactive_days,
            registered_ports: pwp.ports.len() as i64,
        });
    }
    // Missing paths first, then longest idle: the order you want to act in.
    out.sort_by(|a, b| {
        let key = |s: &StaleProject| {
            (
                s.reason != StaleReason::PathMissing,
                -s.inactive_days.unwrap_or(i64::MAX),
            )
        };
        key(a).cmp(&key(b))
    });
    out
}

/// Shelve or unshelve a project. Returns its enriched status so the caller
/// can render the row without a second round-trip.
pub fn set_project_archived(
    db: &Database,
    name: &str,
    archived: bool,
) -> Result<ProjectStatus, String> {
    let updated = db
        .set_project_archived(name, archived)
        .map_err(|e| e.to_string())?;
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let active = scanner::scan_active_ports_detailed();
    enrich_with_status(projects, &active)
        .into_iter()
        .find(|p| p.id == updated.id)
        .ok_or_else(|| format!("project '{name}' not found"))
}

pub fn list_unmanaged(db: &Database) -> Result<Vec<ActivePort>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let registered: HashSet<i64> = projects
        .iter()
        .flat_map(|p| p.ports.iter().map(|port| port.port))
        .collect();
    let mut unmanaged = scanner::scan_unmanaged_ports(&registered);
    unmanaged.sort_by_key(|p| p.port);
    Ok(unmanaged)
}

pub fn scan_active_detailed() -> Vec<ActivePort> {
    let mut ports = scanner::scan_active_ports_detailed();
    ports.sort_by_key(|p| p.port);
    ports
}

pub fn next_range(db: &Database) -> Result<(i64, i64), String> {
    db.next_available_range().map_err(|e| e.to_string())
}

pub fn get_config(db: &Database) -> Result<(String, String), String> {
    let base_port = db.get_config("base_port").map_err(|e| e.to_string())?;
    let range_size = db.get_config("range_size").map_err(|e| e.to_string())?;
    Ok((base_port, range_size))
}

pub fn set_config(db: &Database, key: &str, value: &str) -> Result<(), String> {
    db.set_config(key, value).map_err(|e| e.to_string())
}

/// Returns the id of the trash entry the removal produced, so a caller can
/// offer an undo.
pub fn remove_port_by_service(
    db: &Database,
    project_name: &str,
    service: &str,
) -> Result<Option<i64>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let project = projects
        .iter()
        .find(|p| p.project.name == project_name)
        .ok_or_else(|| format!("project '{}' not found", project_name))?;
    let port = project
        .ports
        .iter()
        .find(|p| p.service == service)
        .ok_or_else(|| {
            format!(
                "service '{}' not found in project '{}'",
                service, project_name
            )
        })?;
    db.remove_port(port.id).map_err(|e| e.to_string())
}

/// Rename a project and/or change its path, keeping its range and registered
/// ports. Returns the updated project enriched with live port status so the
/// caller sees the preserved ports (mirrors what `list_all` would report).
pub fn update_project(
    db: &Database,
    current_name: &str,
    new_name: Option<&str>,
    new_path: Option<&str>,
) -> Result<ProjectStatus, String> {
    let updated = db
        .update_project(current_name, new_name, new_path)
        .map_err(|e| e.to_string())?;
    // Re-enrich so the response carries the project's surviving ports + their
    // live state, addressed by the stable id (the name may have just changed).
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let active = scanner::scan_active_ports_detailed();
    enrich_with_status(projects, &active)
        .into_iter()
        .find(|p| p.id == updated.id)
        .ok_or_else(|| "project not found after update".to_string())
}

pub fn list_trash(db: &Database) -> Result<Vec<TrashEntry>, String> {
    db.list_trash().map_err(|e| e.to_string())
}

pub fn restore_trash(db: &Database, id: i64) -> Result<RestoreOutcome, String> {
    db.restore_trash(id).map_err(|e| e.to_string())
}

pub fn purge_trash(db: &Database, id: i64) -> Result<(), String> {
    db.purge_trash(id).map_err(|e| e.to_string())
}

pub fn purge_trash_all(db: &Database) -> Result<usize, String> {
    db.purge_trash_all().map_err(|e| e.to_string())
}

/// Find the project whose `path` equals `query_path` or is an ancestor of it.
/// Returns the project with the longest matching prefix (most specific) so
/// that nested projects resolve correctly.
pub fn find_project_by_path(
    db: &Database,
    query_path: &str,
) -> Result<Option<ProjectStatus>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let active = scanner::scan_active_ports_detailed();
    let enriched = enrich_with_status(projects, &active);

    let query = std::path::PathBuf::from(query_path);
    let canonical = query.canonicalize().unwrap_or(query);

    let mut best: Option<(usize, ProjectStatus)> = None;
    for ps in enriched {
        if let Some(p) = &ps.path {
            let pp = std::path::PathBuf::from(p);
            // Match on canonical form when possible so symlinks don't hide a hit.
            let pp_canonical = pp.canonicalize().unwrap_or(pp.clone());
            if canonical == pp_canonical || canonical.starts_with(&pp_canonical) {
                let len = pp_canonical.components().count();
                if best.as_ref().map(|(l, _)| len > *l).unwrap_or(true) {
                    best = Some((len, ps));
                }
            }
        }
    }
    Ok(best.map(|(_, ps)| ps))
}

fn is_permission_error(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("operation not permitted") || s.contains("not permitted")
}

/// Send SIGTERM, wait for the grace period, escalate to SIGKILL if needed.
/// Errors from `kill` are mapped to KillOutcome rather than bubbled - the
/// caller only cares about the final state of the port, not which syscall
/// returned what.
pub async fn kill_pid_with_escalation(pid: i64) -> KillOutcome {
    let term = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();
    match term {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            if is_permission_error(&stderr) {
                return KillOutcome::PermissionDenied;
            }
            return KillOutcome::NotActive;
        }
        Err(_) => return KillOutcome::PermissionDenied,
    }

    tokio::time::sleep(KILL_GRACE).await;

    let probe = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output();
    let still_alive = matches!(probe, Ok(o) if o.status.success());
    if !still_alive {
        return KillOutcome::Terminated;
    }

    let force = std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .output();
    match force {
        Ok(o) if o.status.success() => KillOutcome::Killed,
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            if is_permission_error(&stderr) {
                KillOutcome::PermissionDenied
            } else {
                KillOutcome::Terminated
            }
        }
        Err(_) => KillOutcome::PermissionDenied,
    }
}

pub fn is_docker_proxy(process: &str) -> bool {
    DOCKER_PROXY_PROCESSES
        .iter()
        .any(|p| process.eq_ignore_ascii_case(p))
}

/// Parse `docker ps --format '{{.ID}}'` stdout into a list of container IDs.
/// Trims whitespace and drops empty lines. Pure function so it can be tested
/// without invoking docker.
pub fn parse_docker_ps_ids(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// True when a path exists and carries at least one executable bit.
fn is_executable(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Look `name` up in a `PATH`-shaped string. Takes the variable as an argument
/// rather than reading the environment so tests can drive it without mutating
/// process-global state.
fn find_in_path_var(path_var: &str, name: &str) -> Option<std::path::PathBuf> {
    path_var
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| std::path::Path::new(dir).join(name))
        .find(|candidate| is_executable(candidate))
}

/// Locate the docker CLI: explicit override, then `PATH`, then the known
/// install locations. Deliberately uncached - the app lives in the tray for
/// days and Docker Desktop may well be installed after launch, so a cached
/// `None` would keep failing until the next restart. The cost is a handful of
/// `stat` calls on a path that runs once per kill.
fn resolve_docker_bin() -> Option<std::path::PathBuf> {
    if let Some(explicit) = std::env::var_os(DOCKER_BIN_ENV) {
        let p = std::path::PathBuf::from(explicit);
        if is_executable(&p) {
            return Some(p);
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        if let Some(found) = find_in_path_var(&path_var.to_string_lossy(), "docker") {
            return Some(found);
        }
    }
    // Docker Desktop can also install per-user, outside any system prefix.
    let home_candidate = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|h| h.join(".docker/bin/docker"));
    DOCKER_BIN_CANDIDATES
        .iter()
        .map(std::path::PathBuf::from)
        .chain(home_candidate)
        .find(|c| is_executable(c))
}

/// True when a failed `docker ps` failed because the daemon is not reachable,
/// as opposed to a malformed invocation. Docker changed the wording (the
/// classic "Cannot connect to the Docker daemon" became "failed to connect to
/// the docker API" in newer CLIs), so both are matched.
fn is_daemon_unreachable(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("cannot connect to the docker daemon")
        || s.contains("failed to connect to the docker api")
        || s.contains("is the docker daemon running")
}

/// Resolve the host `port` to the container(s) publishing it and call
/// `docker stop` on each. The failure modes are kept apart because they need
/// different things from the user: install or point at a CLI, start Docker,
/// or look at why no container claims that port. `docker stop --time` handles
/// its own SIGTERM->SIGKILL escalation so we don't replicate
/// `kill_pid_with_escalation` here.
async fn stop_docker_container_for_port(port: i64) -> KillOutcome {
    let Some(docker) = resolve_docker_bin() else {
        return KillOutcome::DockerCliMissing;
    };
    let filter = format!("publish={}", port);
    let ps = std::process::Command::new(&docker)
        .args([
            "ps",
            "--filter",
            &filter,
            "--format",
            "{{.ID}}",
            "--no-trunc",
        ])
        .output();
    let ids = match ps {
        Ok(o) if o.status.success() => parse_docker_ps_ids(&String::from_utf8_lossy(&o.stdout)),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            return if is_daemon_unreachable(&stderr) {
                KillOutcome::DockerDaemonDown
            } else {
                KillOutcome::DockerError
            };
        }
        // The binary resolved a moment ago, so a spawn failure here is a
        // vanished or unreadable file rather than a missing install.
        Err(_) => return KillOutcome::DockerError,
    };
    if ids.is_empty() {
        return KillOutcome::DockerNoContainer;
    }
    let timeout = KILL_GRACE.as_secs().to_string();
    let mut any_ok = false;
    for id in ids {
        let stop = std::process::Command::new(&docker)
            .args(["stop", "--time", &timeout, &id])
            .output();
        if matches!(stop, Ok(o) if o.status.success()) {
            any_ok = true;
        }
    }
    if any_ok {
        KillOutcome::DockerStopped
    } else {
        KillOutcome::DockerError
    }
}

/// Single entry point for "free this host port". Detects docker-proxy
/// listeners and routes to `docker stop`; otherwise falls through to the
/// generic SIGTERM+grace+SIGKILL path.
async fn kill_active_port(ap: ActivePort) -> KillOutcome {
    if is_docker_proxy(&ap.process) {
        return stop_docker_container_for_port(ap.port).await;
    }
    kill_pid_with_escalation(ap.pid).await
}

pub async fn kill_port_action(port: i64) -> KillOutcome {
    let active = scanner::scan_active_ports_detailed();
    let Some(target) = active.into_iter().find(|p| p.port == port) else {
        return KillOutcome::NotActive;
    };
    kill_active_port(target).await
}

pub async fn kill_project_action(
    db: &Database,
    project_id: i64,
) -> Result<Vec<(i64, KillOutcome)>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let registered: HashSet<i64> = projects
        .iter()
        .find(|p| p.project.id == project_id)
        .ok_or_else(|| format!("project {project_id} not found"))?
        .ports
        .iter()
        .map(|p| p.port)
        .collect();

    let active: Vec<ActivePort> = scanner::scan_active_ports_detailed()
        .into_iter()
        .filter(|ap| registered.contains(&ap.port))
        .collect();

    let handles: Vec<_> = active
        .into_iter()
        .map(|ap| {
            let port = ap.port;
            tokio::spawn(async move { (port, kill_active_port(ap).await) })
        })
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(r) = h.await {
            results.push(r);
        }
    }
    results.sort_by_key(|(port, _)| *port);
    Ok(results)
}

pub async fn kill_project_by_name(
    db: &Database,
    name: &str,
) -> Result<Vec<(i64, KillOutcome)>, String> {
    let projects = db.list_projects().map_err(|e| e.to_string())?;
    let project = projects
        .iter()
        .find(|p| p.project.name == name)
        .ok_or_else(|| format!("project '{}' not found", name))?;
    let id = project.project.id;
    kill_project_action(db, id).await
}

pub fn open_in_browser(port: i64) -> Result<(), String> {
    if !(1..=65535).contains(&port) {
        return Err(format!("invalid port: {port}"));
    }
    let url = format!("http://localhost:{port}");
    let (cmd, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("open", &[])
    } else if cfg!(target_os = "windows") {
        ("cmd", &["/C", "start", ""])
    } else {
        // Linux + other unix - rely on freedesktop.org's xdg-open. On a
        // headless server this still spawns successfully but the URL goes
        // nowhere; that's the caller's responsibility to avoid.
        ("xdg-open", &[])
    };
    std::process::Command::new(cmd)
        .args(args)
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, Port, Project, ProjectWithPorts};

    fn project(id: i64, name: &str, range: (i64, i64)) -> Project {
        Project {
            id,
            name: name.into(),
            path: None,
            range_start: range.0,
            range_end: range.1,
            created_at: "now".into(),
            archived_at: None,
        }
    }

    fn port(id: i64, project_id: i64, service: &str, port: i64) -> Port {
        Port {
            id,
            project_id,
            service: service.into(),
            port,
            created_at: "now".into(),
        }
    }

    fn active(port: i64, process: &str) -> ActivePort {
        ActivePort {
            port,
            process: process.into(),
            pid: 999,
        }
    }

    #[test]
    fn enrich_marks_active_ports_and_attaches_process_name() {
        let projects = vec![ProjectWithPorts {
            project: project(1, "alpha", (4000, 4009)),
            ports: vec![port(10, 1, "vite", 4000), port(11, 1, "api", 4001)],
        }];
        let active_list = vec![active(4000, "node")];

        let result = enrich_with_status(projects, &active_list);

        assert_eq!(result.len(), 1);
        let p = &result[0];
        assert_eq!(p.name, "alpha");
        assert_eq!(p.ports.len(), 2);

        let vite = p.ports.iter().find(|p| p.service == "vite").unwrap();
        assert!(vite.active);
        assert_eq!(vite.process.as_deref(), Some("node"));
        assert_eq!(vite.pid, Some(999));

        let api = p.ports.iter().find(|p| p.service == "api").unwrap();
        assert!(!api.active);
        assert!(api.process.is_none());
        assert!(api.pid.is_none());
    }

    #[test]
    fn enrich_with_no_active_ports_marks_everything_inactive() {
        let projects = vec![ProjectWithPorts {
            project: project(1, "alpha", (4000, 4009)),
            ports: vec![port(10, 1, "vite", 4000)],
        }];
        let result = enrich_with_status(projects, &[]);
        assert!(!result[0].ports[0].active);
        assert!(result[0].ports[0].process.is_none());
        assert!(result[0].ports[0].pid.is_none());
    }

    #[test]
    fn enrich_active_port_outside_any_project_is_ignored() {
        let projects = vec![ProjectWithPorts {
            project: project(1, "alpha", (4000, 4009)),
            ports: vec![port(10, 1, "vite", 4000)],
        }];
        let active_list = vec![active(9999, "node")];

        let result = enrich_with_status(projects, &active_list);
        assert_eq!(result[0].ports.len(), 1);
        assert!(!result[0].ports[0].active);
    }

    #[test]
    fn enrich_preserves_project_order_and_metadata() {
        let projects = vec![
            ProjectWithPorts {
                project: Project {
                    id: 1,
                    name: "alpha".into(),
                    path: Some("/tmp/alpha".into()),
                    range_start: 4000,
                    range_end: 4009,
                    created_at: "t1".into(),
                    archived_at: None,
                },
                ports: vec![],
            },
            ProjectWithPorts {
                project: project(2, "bravo", (4010, 4019)),
                ports: vec![],
            },
        ];
        let result = enrich_with_status(projects, &[]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "alpha");
        assert_eq!(result[0].path.as_deref(), Some("/tmp/alpha"));
        assert_eq!(result[0].range_start, 4000);
        assert_eq!(result[0].range_end, 4009);
        assert_eq!(result[1].name, "bravo");
    }

    #[test]
    fn is_permission_error_matches_macos_and_linux_phrasing() {
        assert!(is_permission_error(
            "kill: (12345) - Operation not permitted"
        ));
        assert!(is_permission_error("kill: 12345: Operation not permitted"));
        assert!(is_permission_error("OPERATION NOT PERMITTED"));
    }

    #[test]
    fn is_permission_error_rejects_other_failures() {
        assert!(!is_permission_error("kill: (12345) - No such process"));
        assert!(!is_permission_error(""));
    }

    // --- docker proxy detection ---

    #[test]
    fn is_docker_proxy_matches_known_processes() {
        assert!(is_docker_proxy("com.docker.backend"));
        assert!(is_docker_proxy("Com.Docker.Backend")); // case-insensitive
        assert!(is_docker_proxy("vpnkit"));
        assert!(is_docker_proxy("com.docker.vpnkit"));
        assert!(is_docker_proxy("docker-proxy"));
        assert!(is_docker_proxy("Docker"));
    }

    #[test]
    fn is_docker_proxy_rejects_unrelated_processes() {
        assert!(!is_docker_proxy("node"));
        assert!(!is_docker_proxy("python"));
        assert!(!is_docker_proxy("postgres"));
        // Partial match must not trigger: "dockerd" is the daemon, not a
        // port-proxy; we never want to SIGTERM it nor route it through the
        // container-resolution path (it doesn't publish ports on the host).
        assert!(!is_docker_proxy("dockerd"));
        // Random docker-named binary in a user's PATH must not be considered
        // a proxy either - we explicitly enumerate the proxies we know about.
        assert!(!is_docker_proxy("docker-compose"));
        assert!(!is_docker_proxy(""));
    }

    #[test]
    fn parse_docker_ps_ids_extracts_one_per_line() {
        let stdout = "abc123\ndef456\n";
        assert_eq!(parse_docker_ps_ids(stdout), vec!["abc123", "def456"]);
    }

    #[test]
    fn parse_docker_ps_ids_trims_and_skips_blanks() {
        let stdout = "  abc123  \n\n  def456\n\n";
        assert_eq!(parse_docker_ps_ids(stdout), vec!["abc123", "def456"]);
    }

    #[test]
    fn parse_docker_ps_ids_empty_when_no_match() {
        assert!(parse_docker_ps_ids("").is_empty());
        assert!(parse_docker_ps_ids("\n\n   \n").is_empty());
    }

    // --- docker CLI resolution ---

    /// Write an executable stub at `dir/name` and return its path.
    fn write_stub(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("portsage-test-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn find_in_path_var_returns_first_executable_hit() {
        let first = temp_dir("path-first");
        let second = temp_dir("path-second");
        let expected = write_stub(&first, "docker");
        write_stub(&second, "docker");
        let path_var = format!("{}:{}", first.display(), second.display());
        assert_eq!(find_in_path_var(&path_var, "docker"), Some(expected));
    }

    #[test]
    fn find_in_path_var_skips_non_executable_and_missing_entries() {
        let dir = temp_dir("path-noexec");
        // A plain file with no executable bit must not be picked up: this is
        // what a stray `docker` config file or a wrapper without +x looks like.
        let plain = dir.join("docker");
        std::fs::write(&plain, "not a binary").unwrap();
        let path_var = format!("/nonexistent-dir-portsage::{}", dir.display());
        assert_eq!(find_in_path_var(&path_var, "docker"), None);
    }

    #[test]
    fn is_daemon_unreachable_matches_both_cli_wordings() {
        // Classic wording, still emitted by older CLIs.
        assert!(is_daemon_unreachable(
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. \
             Is the docker daemon running?"
        ));
        // Wording emitted by current Docker Desktop CLIs.
        assert!(is_daemon_unreachable(
            "failed to connect to the docker API at unix:///tmp/nope.sock; \
             check if the path is correct and if the daemon is running"
        ));
    }

    #[test]
    fn is_daemon_unreachable_rejects_other_failures() {
        assert!(!is_daemon_unreachable(
            "Error response from daemon: No such container: abc123"
        ));
        assert!(!is_daemon_unreachable("unknown flag: --publish"));
        assert!(!is_daemon_unreachable(""));
    }

    // --- stale_from (prune selection) ---

    fn stale_project(name: &str, path: Option<&str>, ports: &[i64]) -> ProjectWithPorts {
        ProjectWithPorts {
            project: Project {
                id: 1,
                name: name.into(),
                path: path.map(str::to_string),
                range_start: 4000,
                range_end: 4009,
                created_at: "t".into(),
                archived_at: None,
            },
            ports: ports
                .iter()
                .map(|p| Port {
                    id: *p,
                    project_id: 1,
                    service: "svc".into(),
                    port: *p,
                    created_at: "t".into(),
                })
                .collect(),
        }
    }

    fn fresh_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("portsage-stale-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn stale_from_flags_a_missing_path_regardless_of_age() {
        let projects = vec![stale_project(
            "gone",
            Some("/nope/portsage-missing"),
            &[4000],
        )];
        let out = stale_from(projects, &HashSet::new(), 90, std::time::SystemTime::now());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].reason, StaleReason::PathMissing);
        // No day count to report when there is no directory to measure.
        assert_eq!(out[0].inactive_days, None);
        assert_eq!(out[0].registered_ports, 1);
    }

    #[test]
    fn stale_from_never_touches_a_project_with_a_listening_port() {
        // The safety property that makes acting on this list safe: something
        // is bound to that range right now, so it is in use whatever the
        // filesystem says.
        let projects = vec![stale_project(
            "running",
            Some("/nope/portsage-missing"),
            &[4000],
        )];
        let active = HashSet::from([4000]);
        assert!(stale_from(projects, &active, 90, std::time::SystemTime::now()).is_empty());
    }

    #[test]
    fn stale_from_skips_already_archived_projects() {
        let mut p = stale_project("shelved", Some("/nope/portsage-missing"), &[]);
        p.project.archived_at = Some("2026-01-01 00:00:00".into());
        assert!(stale_from(vec![p], &HashSet::new(), 90, std::time::SystemTime::now()).is_empty());
    }

    #[test]
    fn stale_from_respects_the_threshold() {
        let dir = fresh_dir("threshold");
        let path = dir.to_str().unwrap().to_string();
        let now = std::time::SystemTime::now() + std::time::Duration::from_secs(80 * 86_400);

        // 80 days idle: under a 90-day threshold, over a 60-day one.
        let under = stale_from(
            vec![stale_project("p", Some(&path), &[])],
            &HashSet::new(),
            90,
            now,
        );
        assert!(under.is_empty(), "80 days must not trip a 90-day threshold");

        let over = stale_from(
            vec![stale_project("p", Some(&path), &[])],
            &HashSet::new(),
            60,
            now,
        );
        assert_eq!(over.len(), 1);
        assert_eq!(over[0].reason, StaleReason::Inactive);
        assert_eq!(over[0].inactive_days, Some(80));
    }

    #[test]
    fn stale_from_ignores_projects_without_a_path() {
        // No path means no signal. Guessing "abandoned" from silence would
        // put every path-less project on the chopping block.
        let projects = vec![stale_project("nopath", None, &[])];
        assert!(stale_from(projects, &HashSet::new(), 1, std::time::SystemTime::now()).is_empty());
    }

    #[test]
    fn stale_from_lists_missing_paths_before_idle_ones() {
        let dir = fresh_dir("order");
        let path = dir.to_str().unwrap().to_string();
        let now = std::time::SystemTime::now() + std::time::Duration::from_secs(200 * 86_400);
        let mut idle = stale_project("idle", Some(&path), &[]);
        idle.project.name = "idle".into();
        let out = stale_from(
            vec![
                idle,
                stale_project("gone", Some("/nope/portsage-missing"), &[]),
            ],
            &HashSet::new(),
            90,
            now,
        );
        assert_eq!(out.len(), 2);
        // A missing directory is a stronger signal than 200 idle days, and
        // it is also the one that might just need its path corrected.
        assert_eq!(out[0].name, "gone");
        assert_eq!(out[1].name, "idle");
    }

    // --- remove_port_by_service ---

    #[test]
    fn remove_port_by_service_removes_matching_port() {
        let db = Database::in_memory().unwrap();
        let p = db.create_project("alpha", None).unwrap();
        db.add_port(p.id, "vite", 4000).unwrap();
        db.add_port(p.id, "api", 4001).unwrap();
        remove_port_by_service(&db, "alpha", "vite").unwrap();
        let projects = db.list_projects().unwrap();
        assert_eq!(projects[0].ports.len(), 1);
        assert_eq!(projects[0].ports[0].service, "api");
    }

    #[test]
    fn remove_port_by_service_rejects_unknown_project() {
        let db = Database::in_memory().unwrap();
        let err = remove_port_by_service(&db, "ghost", "vite").unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
    }

    #[test]
    fn remove_port_by_service_rejects_unknown_service() {
        let db = Database::in_memory().unwrap();
        db.create_project("alpha", None).unwrap();
        let err = remove_port_by_service(&db, "alpha", "ghost").unwrap_err();
        assert!(err.contains("service"), "got: {err}");
    }

    // --- update_project ---

    #[test]
    fn update_project_returns_enriched_status_with_ports() {
        let db = Database::in_memory().unwrap();
        let p = db.create_project("omnia", Some("/old")).unwrap();
        db.add_port(p.id, "vite", p.range_start).unwrap();
        let updated = update_project(&db, "omnia", Some("omnia-ddt"), Some("/new")).unwrap();
        assert_eq!(updated.id, p.id);
        assert_eq!(updated.name, "omnia-ddt");
        assert_eq!(updated.path.as_deref(), Some("/new"));
        assert_eq!(updated.ports.len(), 1, "ports survive the rename");
        assert_eq!(updated.ports[0].service, "vite");
    }

    #[test]
    fn update_project_unknown_surfaces_error_string() {
        let db = Database::in_memory().unwrap();
        let err = update_project(&db, "ghost", Some("x"), None).unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
    }

    // --- list_unmanaged / list_with_status / config ---

    #[test]
    fn list_with_status_returns_enriched_projects() {
        let db = Database::in_memory().unwrap();
        let p = db.create_project("alpha", Some("/tmp/alpha")).unwrap();
        db.add_port(p.id, "vite", 4000).unwrap();
        let result = list_with_status(&db).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "alpha");
        assert_eq!(result[0].path.as_deref(), Some("/tmp/alpha"));
        assert_eq!(result[0].ports.len(), 1);
        // We don't assert `active` because the answer depends on which ports
        // happen to be listening on the test host; the structural shape is
        // what matters.
    }

    #[test]
    fn next_range_after_existing_project_skips_taken() {
        let db = Database::in_memory().unwrap();
        db.create_project("alpha", None).unwrap();
        let (start, end) = next_range(&db).unwrap();
        assert_eq!(start, 4010);
        assert_eq!(end, 4019);
    }

    #[test]
    fn config_round_trip() {
        let db = Database::in_memory().unwrap();
        let (base, size) = get_config(&db).unwrap();
        assert_eq!(base, "4000");
        assert_eq!(size, "10");

        set_config(&db, "base_port", "5000").unwrap();
        let (base, _size) = get_config(&db).unwrap();
        assert_eq!(base, "5000");
    }

    // --- find_project_by_path ---

    #[test]
    fn find_project_by_path_exact_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();
        let db = Database::in_memory().unwrap();
        db.create_project("alpha", Some(&path)).unwrap();
        let found = find_project_by_path(&db, &path).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "alpha");
    }

    #[test]
    fn find_project_by_path_descendant_match() {
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().to_string_lossy().to_string();
        let nested = dir.path().join("src").join("components");
        std::fs::create_dir_all(&nested).unwrap();
        let nested_str = nested.to_string_lossy().to_string();

        let db = Database::in_memory().unwrap();
        db.create_project("alpha", Some(&project_path)).unwrap();
        let found = find_project_by_path(&db, &nested_str).unwrap();
        assert!(
            found.is_some(),
            "descendant should resolve to ancestor project"
        );
        assert_eq!(found.unwrap().name, "alpha");
    }

    #[test]
    fn find_project_by_path_no_match_returns_none() {
        let db = Database::in_memory().unwrap();
        db.create_project("alpha", Some("/tmp/somewhere/else"))
            .unwrap();
        let found = find_project_by_path(&db, "/var/log").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn find_project_by_path_prefers_more_specific_ancestor() {
        // Two projects, parent and nested. The deeper one wins.
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path().to_string_lossy().to_string();
        let inner_dir = dir.path().join("inner");
        std::fs::create_dir_all(&inner_dir).unwrap();
        let inner = inner_dir.to_string_lossy().to_string();
        let leaf_dir = inner_dir.join("leaf");
        std::fs::create_dir_all(&leaf_dir).unwrap();
        let leaf = leaf_dir.to_string_lossy().to_string();

        let db = Database::in_memory().unwrap();
        db.create_project("outer", Some(&outer)).unwrap();
        db.create_project("inner", Some(&inner)).unwrap();

        let found = find_project_by_path(&db, &leaf).unwrap().unwrap();
        assert_eq!(found.name, "inner", "deeper project should win");
    }
}
