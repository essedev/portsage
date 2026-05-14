//! Multi-host backend dispatcher.
//!
//! The Mac UI talks to one of two kinds of backends:
//!   - `Local`: the per-user Portsage instance, reachable on the default Unix
//!     socket (`paths::socket_path()`).
//!   - `Remote { name }`: a remote `portsage-server` running on another box,
//!     reachable via an SSH local-socket forward (`ssh -L unix:...`).
//!
//! `BackendManager` owns the SSH tunnel map. Calling `connect(target)`:
//!   - For Local: returns a `Client` pointed at the local socket. Cheap.
//!   - For Remote: looks up the catalogue row, ensures an SSH tunnel is open
//!     and the local-side forwarded socket is reachable, and returns a
//!     `Client` pointed at that path.
//!
//! Concurrency: the outer tunnel map is locked only long enough to get-or-
//! insert the per-backend slot. Each per-backend slot has its own mutex, so
//! two callers trying to connect to the same backend serialize on it (the
//! second one sees `Connected` and skips the spawn); two callers connecting
//! to *different* backends run in parallel.
//!
//! Shell-out vs embedded SSH: we shell out to the system `ssh` binary on
//! purpose. The user's `~/.ssh/config` (ControlMaster, ProxyJump, key
//! lookup, hardware-token PIN prompts) is the source of truth for SSH; we
//! must not bypass it. The docs/multi-host-evolution.md plan covers the
//! reasoning in detail.

use crate::actions;
use crate::db::{Database, RemoteBackend, RemoteBackendInput};
use crate::paths;
use portsage_client::{
    ActivePort, Client, ConfigSnapshot, KillEntry, KillOutcome, PortStatus, ProjectStatus,
    RangeBounds,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long `ensure_tunnel` waits for the local-side forwarded socket to
/// appear after spawning `ssh`. Real ControlMaster reuse is sub-second; cold
/// auth (key passphrase prompt cached in ssh-agent, ProxyJump hop) can take
/// a few seconds. Five seconds is the line where it's almost certainly never
/// going to come up without manual intervention.
pub const TUNNEL_OPEN_TIMEOUT: Duration = Duration::from_secs(5);

/// Poll interval while waiting for the forwarded socket to appear.
const TUNNEL_OPEN_POLL: Duration = Duration::from_millis(100);

/// Owned-string form of `RemoteBackendInput` suitable for crossing the Tauri
/// command boundary. The borrowed version (`db::RemoteBackendInput`) is what
/// the database layer accepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteBackendForm {
    pub name: String,
    pub ssh_alias: String,
    pub remote_socket_path: String,
    pub local_socket_path: String,
    #[serde(default)]
    pub auto_forward_enabled: bool,
}

impl RemoteBackendForm {
    pub fn as_input(&self) -> RemoteBackendInput<'_> {
        RemoteBackendInput {
            name: &self.name,
            ssh_alias: &self.ssh_alias,
            remote_socket_path: &self.remote_socket_path,
            local_socket_path: &self.local_socket_path,
            auto_forward_enabled: self.auto_forward_enabled,
        }
    }
}

/// Which backend a caller wants to talk to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendTarget {
    Local,
    Remote { name: String },
}

/// State machine for a single SSH local-forward tunnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TunnelState {
    Disconnected,
    Connecting,
    Connected,
    Failed { reason: String },
}

/// Public-facing snapshot of a tunnel's runtime state. Distinct from the
/// internal `TunnelEntry` so we can expose state to the UI/CLI without
/// leaking the `Child` handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStatus {
    pub backend_name: String,
    pub ssh_alias: String,
    pub remote_socket: String,
    pub local_socket: String,
    pub state: TunnelState,
}

/// Per-backend entry in the tunnel map. The `Child` is owned so we can kill
/// it on `close()`; we never wait on it (a long-lived ssh tunnel is the
/// happy path and we don't want to reap it accidentally).
#[derive(Debug)]
struct TunnelEntry {
    backend_name: String,
    ssh_alias: String,
    remote_socket: PathBuf,
    local_socket: PathBuf,
    state: TunnelState,
    child: Option<Child>,
}

impl TunnelEntry {
    fn from_backend(b: &RemoteBackend) -> Self {
        Self {
            backend_name: b.name.clone(),
            ssh_alias: b.ssh_alias.clone(),
            remote_socket: PathBuf::from(&b.remote_socket_path),
            local_socket: PathBuf::from(&b.local_socket_path),
            state: TunnelState::Disconnected,
            child: None,
        }
    }

    fn to_status(&self) -> TunnelStatus {
        TunnelStatus {
            backend_name: self.backend_name.clone(),
            ssh_alias: self.ssh_alias.clone(),
            remote_socket: self.remote_socket.to_string_lossy().into_owned(),
            local_socket: self.local_socket.to_string_lossy().into_owned(),
            state: self.state.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("unknown remote backend: {0}")]
    UnknownBackend(String),

    #[error("database error: {0}")]
    Db(String),

    #[error("failed to spawn ssh: {0}")]
    SshSpawn(String),

    #[error("ssh exited before the tunnel opened: {0}")]
    SshExited(String),

    #[error("tunnel for '{0}' did not become reachable within {1:?}")]
    TunnelTimeout(String, Duration),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Strategy for spawning the SSH child. Production uses `SystemSsh`; tests
/// inject `MockSsh` to exercise the manager without a real SSH server.
pub trait TunnelLauncher: Send + Sync + std::fmt::Debug {
    /// Spawn the SSH client and return a child handle. Implementations are
    /// responsible for whatever side-effects are needed for the local socket
    /// to eventually become connectable. Returning `Ok` does not mean the
    /// socket is up yet - the caller polls.
    fn spawn(&self, backend: &RemoteBackend) -> Result<Child, BackendError>;
}

#[derive(Debug)]
pub struct SystemSsh;

impl TunnelLauncher for SystemSsh {
    fn spawn(&self, backend: &RemoteBackend) -> Result<Child, BackendError> {
        // The forward spec is `<local_socket>:<remote_socket>` per OpenSSH's
        // socket-forward syntax. `StreamLocalBindUnlink=yes` makes ssh unlink
        // any stale local socket file before binding (without it, a leftover
        // file from a previous run causes the forward to fail). `ExitOnForward
        // Failure=yes` makes ssh exit immediately if the remote end refuses
        // the bind, so we don't wait the full timeout for a doomed tunnel.
        // `ServerAliveInterval` lets the tunnel notice a dead peer.
        let forward = format!(
            "{}:{}",
            backend.local_socket_path, backend.remote_socket_path,
        );
        Command::new("ssh")
            .args([
                "-N",
                "-T",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "StreamLocalBindUnlink=yes",
                "-o",
                "ServerAliveInterval=30",
                "-o",
                "ServerAliveCountMax=3",
                "-L",
                &forward,
                &backend.ssh_alias,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| BackendError::SshSpawn(e.to_string()))
    }
}

/// Owns the SSH tunnels for every configured remote backend. One instance
/// per app process.
pub struct BackendManager {
    db: Arc<Database>,
    tunnels: Mutex<HashMap<String, Arc<Mutex<TunnelEntry>>>>,
    launcher: Arc<dyn TunnelLauncher>,
}

impl std::fmt::Debug for BackendManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendManager")
            .field("launcher", &self.launcher)
            .finish_non_exhaustive()
    }
}

impl BackendManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self::with_launcher(db, Arc::new(SystemSsh))
    }

    pub fn with_launcher(db: Arc<Database>, launcher: Arc<dyn TunnelLauncher>) -> Self {
        Self {
            db,
            tunnels: Mutex::new(HashMap::new()),
            launcher,
        }
    }

    /// Open a `Client` pointed at the target backend. For Remote targets this
    /// ensures the SSH tunnel is up before returning.
    pub fn connect(&self, target: &BackendTarget) -> Result<Client, BackendError> {
        match target {
            BackendTarget::Local => Ok(Client::new(paths::socket_path())),
            BackendTarget::Remote { name } => {
                let local_socket = self.ensure_tunnel(name)?;
                Ok(Client::new(local_socket))
            }
        }
    }

    /// Ensure the tunnel for `name` is open and return its local-side socket
    /// path. Subsequent callers reuse the open tunnel.
    pub fn ensure_tunnel(&self, name: &str) -> Result<PathBuf, BackendError> {
        let entry = self.get_or_insert_entry(name)?;
        // Lock per-backend so two callers don't race to spawn ssh twice.
        let mut tunnel = entry.lock().unwrap_or_else(|p| p.into_inner());

        if matches!(tunnel.state, TunnelState::Connected) && is_socket_alive(&tunnel.local_socket) {
            return Ok(tunnel.local_socket.clone());
        }

        // Stale child (process exited but state never updated): reap and reset.
        if let Some(child) = tunnel.child.as_mut() {
            if let Ok(Some(_)) = child.try_wait() {
                tunnel.child = None;
                tunnel.state = TunnelState::Disconnected;
            }
        }

        tunnel.state = TunnelState::Connecting;
        let backend = self
            .db
            .get_remote_backend_by_name(name)
            .map_err(|e| BackendError::Db(e.to_string()))?
            .ok_or_else(|| BackendError::UnknownBackend(name.to_string()))?;

        let child = self.launcher.spawn(&backend).inspect_err(|e| {
            tunnel.state = TunnelState::Failed {
                reason: e.to_string(),
            };
        })?;
        tunnel.child = Some(child);
        tunnel.local_socket = PathBuf::from(&backend.local_socket_path);
        tunnel.remote_socket = PathBuf::from(&backend.remote_socket_path);
        tunnel.ssh_alias = backend.ssh_alias.clone();

        match wait_for_socket(&tunnel.local_socket, TUNNEL_OPEN_TIMEOUT) {
            Ok(()) => {
                tunnel.state = TunnelState::Connected;
                Ok(tunnel.local_socket.clone())
            }
            Err(open_err) => {
                let reason = self
                    .drain_ssh_stderr(&mut tunnel)
                    .unwrap_or_else(|| open_err.to_string());
                let _ = self.kill_child(&mut tunnel);
                tunnel.state = TunnelState::Failed {
                    reason: reason.clone(),
                };
                Err(BackendError::SshExited(reason))
            }
        }
    }

    /// Close the tunnel for `name` if open. Idempotent.
    pub fn close_tunnel(&self, name: &str) -> Result<(), BackendError> {
        let entry = {
            let map = self.tunnels.lock().unwrap_or_else(|p| p.into_inner());
            map.get(name).cloned()
        };
        let entry = match entry {
            Some(e) => e,
            None => return Ok(()),
        };
        let mut tunnel = entry.lock().unwrap_or_else(|p| p.into_inner());
        self.kill_child(&mut tunnel)?;
        tunnel.state = TunnelState::Disconnected;
        Ok(())
    }

    /// Snapshot of every tunnel known to this manager. Useful for the UI.
    pub fn statuses(&self) -> Vec<TunnelStatus> {
        let map = self.tunnels.lock().unwrap_or_else(|p| p.into_inner());
        map.values()
            .map(|entry| {
                let t = entry.lock().unwrap_or_else(|p| p.into_inner());
                t.to_status()
            })
            .collect()
    }

    fn get_or_insert_entry(&self, name: &str) -> Result<Arc<Mutex<TunnelEntry>>, BackendError> {
        let mut map = self.tunnels.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = map.get(name) {
            return Ok(entry.clone());
        }
        let backend = self
            .db
            .get_remote_backend_by_name(name)
            .map_err(|e| BackendError::Db(e.to_string()))?
            .ok_or_else(|| BackendError::UnknownBackend(name.to_string()))?;
        let entry = Arc::new(Mutex::new(TunnelEntry::from_backend(&backend)));
        map.insert(name.to_string(), entry.clone());
        Ok(entry)
    }

    fn kill_child(&self, tunnel: &mut TunnelEntry) -> Result<(), BackendError> {
        if let Some(mut child) = tunnel.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Best-effort cleanup of the local socket file. ssh's
        // StreamLocalBindUnlink=yes will unlink before the next bind anyway,
        // so failure here is not fatal.
        let _ = std::fs::remove_file(&tunnel.local_socket);
        Ok(())
    }

    /// Pull whatever the spawned ssh process wrote to stderr before it died.
    /// Used to surface a useful error to the user (`Host key verification
    /// failed`, `Permission denied`, etc.) rather than a generic timeout.
    fn drain_ssh_stderr(&self, tunnel: &mut TunnelEntry) -> Option<String> {
        let mut child = tunnel.child.take()?;
        use std::io::Read;
        let mut buf = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut buf);
        }
        // Put the child back so the caller can still kill+wait if needed.
        tunnel.child = Some(child);
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

// --- BackendClient: socket protocol against the *current* target ---

/// Adapter that surfaces the socket protocol against whichever backend the
/// app is currently targeting. Local is satisfied from the in-process
/// `Database` (no socket round-trip). Remote delegates to a
/// `portsage_client::Client` pointed at the local-side forwarded socket.
///
/// Phase 2 (this commit) only implements the read-side methods used by the
/// `test_remote_backend` command and by the future backend-aware fetch
/// hooks. The write-side (`reserve_range`, `register_port`, `release_project`,
/// `remove_port`, `set_config`, `kill_port`, `kill_project`,
/// `open_in_browser`) lands in 2.7 when the existing Tauri commands migrate
/// onto this adapter.
pub enum BackendClient {
    Local(Arc<Database>),
    Remote(Client),
}

impl BackendClient {
    pub fn list_all(&self) -> Result<Vec<ProjectStatus>, String> {
        match self {
            BackendClient::Local(db) => actions::list_with_status(db),
            BackendClient::Remote(c) => c.list_all().map_err(|e| e.to_string()),
        }
    }

    pub fn list_unmanaged(&self) -> Result<Vec<ActivePort>, String> {
        match self {
            BackendClient::Local(db) => actions::list_unmanaged(db),
            BackendClient::Remote(c) => c.list_unmanaged().map_err(|e| e.to_string()),
        }
    }

    pub fn scan_active(&self) -> Result<Vec<ActivePort>, String> {
        match self {
            BackendClient::Local(_) => Ok(actions::scan_active_detailed()),
            BackendClient::Remote(c) => c.scan_active().map_err(|e| e.to_string()),
        }
    }

    pub fn next_range(&self) -> Result<RangeBounds, String> {
        match self {
            BackendClient::Local(db) => {
                let (range_start, range_end) = actions::next_range(db)?;
                Ok(RangeBounds {
                    range_start,
                    range_end,
                })
            }
            BackendClient::Remote(c) => c.next_range().map_err(|e| e.to_string()),
        }
    }

    pub fn get_config(&self) -> Result<ConfigSnapshot, String> {
        match self {
            BackendClient::Local(db) => {
                let (base_port, range_size) = actions::get_config(db)?;
                Ok(ConfigSnapshot {
                    base_port,
                    range_size,
                })
            }
            BackendClient::Remote(c) => c.get_config().map_err(|e| e.to_string()),
        }
    }

    // -- write-side methods (mirrors of the socket protocol; in Phase 2 only
    // the Local variants are wired through the Tauri command layer; Remote
    // variants will be used by 2.7 when the routing refactor lands).

    pub fn reserve_range(&self, name: &str, path: Option<&str>) -> Result<ProjectStatus, String> {
        match self {
            BackendClient::Local(db) => {
                let p = db.create_project(name, path).map_err(|e| e.to_string())?;
                Ok(ProjectStatus {
                    id: p.id,
                    name: p.name,
                    path: p.path,
                    range_start: p.range_start,
                    range_end: p.range_end,
                    created_at: p.created_at,
                    ports: Vec::new(),
                })
            }
            BackendClient::Remote(c) => c.reserve_range(name, path).map_err(|e| e.to_string()),
        }
    }

    pub fn register_port(
        &self,
        project: &str,
        service: &str,
        port: i64,
    ) -> Result<PortStatus, String> {
        match self {
            BackendClient::Local(db) => {
                let projects = db.list_projects().map_err(|e| e.to_string())?;
                let proj = projects
                    .iter()
                    .find(|p| p.project.name == project)
                    .ok_or_else(|| format!("project '{project}' not found"))?;
                let p = db
                    .add_port(proj.project.id, service, port)
                    .map_err(|e| e.to_string())?;
                Ok(PortStatus {
                    id: p.id,
                    project_id: p.project_id,
                    service: p.service,
                    port: p.port,
                    created_at: p.created_at,
                    active: false,
                    process: None,
                    pid: None,
                })
            }
            BackendClient::Remote(c) => c
                .register_port(project, service, port)
                .map_err(|e| e.to_string()),
        }
    }

    pub fn remove_port(&self, project: &str, service: &str) -> Result<(), String> {
        match self {
            BackendClient::Local(db) => actions::remove_port_by_service(db, project, service),
            BackendClient::Remote(c) => c.remove_port(project, service).map_err(|e| e.to_string()),
        }
    }

    pub fn release_project(&self, name: &str) -> Result<(), String> {
        match self {
            BackendClient::Local(db) => {
                let projects = db.list_projects().map_err(|e| e.to_string())?;
                let proj = projects
                    .iter()
                    .find(|p| p.project.name == name)
                    .ok_or_else(|| format!("project '{name}' not found"))?;
                db.delete_project(proj.project.id)
                    .map_err(|e| e.to_string())
            }
            BackendClient::Remote(c) => c.release_project(name).map_err(|e| e.to_string()),
        }
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<(), String> {
        // The socket dispatcher whitelists set_config keys; mirror that here
        // so Remote calls fail the same way and Local writes don't accept
        // arbitrary keys via this adapter either.
        if !matches!(key, "base_port" | "range_size") {
            return Err(format!("unknown config key: {key}"));
        }
        match self {
            BackendClient::Local(db) => actions::set_config(db, key, value),
            BackendClient::Remote(c) => c.set_config(key, value).map_err(|e| e.to_string()),
        }
    }

    pub async fn kill_port(&self, port: i64) -> Result<KillOutcome, String> {
        match self {
            BackendClient::Local(_) => Ok(actions::kill_port_action(port).await),
            BackendClient::Remote(c) => {
                // The synchronous portsage_client call would block the tokio
                // worker; spawn_blocking keeps the async runtime responsive.
                let c = c.clone();
                tokio::task::spawn_blocking(move || c.kill_port(port))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())
            }
        }
    }

    pub async fn kill_project(&self, name: &str) -> Result<Vec<KillEntry>, String> {
        match self {
            BackendClient::Local(db) => actions::kill_project_by_name(db, name).await.map(|v| {
                v.into_iter()
                    .map(|(port, outcome)| KillEntry { port, outcome })
                    .collect()
            }),
            BackendClient::Remote(c) => {
                let c = c.clone();
                let name = name.to_string();
                tokio::task::spawn_blocking(move || c.kill_project(&name))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())
            }
        }
    }

    pub fn open_in_browser(&self, port: i64) -> Result<(), String> {
        // Always opens on the *local* machine (the Mac) - even when the
        // current target is Remote. The remote box has no GUI and the Mac
        // is where the user is looking. Until Phase 3 wires SSH port
        // forwarding, this call may surface a connection-refused error in
        // the browser when the underlying port is on the remote host; the
        // call itself still succeeds (it just spawns `open`/`xdg-open`).
        actions::open_in_browser(port)
    }
}

// --- BackendRouter: the singleton that owns current target + manager ---

/// App-level singleton that holds the user's current backend choice and
/// dispatches to a `BackendClient`. Lives in Tauri state alongside the
/// `Database`.
pub struct BackendRouter {
    db: Arc<Database>,
    manager: Arc<BackendManager>,
    current: Mutex<BackendTarget>,
}

impl std::fmt::Debug for BackendRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendRouter")
            .field("current", &self.current())
            .finish_non_exhaustive()
    }
}

/// Config key under which the last-used backend target is persisted. Picked
/// to be ignored by the socket protocol's `set_config` whitelist - the
/// frontend changes the target via `set_current_backend`, never via the
/// generic config setter.
pub const CURRENT_BACKEND_KEY: &str = "current_backend";

impl BackendRouter {
    pub fn new(db: Arc<Database>) -> Self {
        let manager = Arc::new(BackendManager::new(db.clone()));
        let initial = load_persisted_target(&db).unwrap_or(BackendTarget::Local);
        Self {
            db,
            manager,
            current: Mutex::new(initial),
        }
    }

    pub fn manager(&self) -> &Arc<BackendManager> {
        &self.manager
    }

    pub fn database(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn current(&self) -> BackendTarget {
        self.current
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Update the active backend. For Remote targets the existence of the
    /// named backend is validated against the database before the switch
    /// takes effect; an unknown name returns `UnknownBackend` instead of
    /// silently switching to a target that can never connect.
    pub fn set_current(&self, target: BackendTarget) -> Result<(), BackendError> {
        if let BackendTarget::Remote { ref name } = target {
            if self
                .db
                .get_remote_backend_by_name(name)
                .map_err(|e| BackendError::Db(e.to_string()))?
                .is_none()
            {
                return Err(BackendError::UnknownBackend(name.clone()));
            }
        }
        {
            let mut guard = self.current.lock().unwrap_or_else(|p| p.into_inner());
            *guard = target.clone();
        }
        // Persist via the raw db.set_config so the entry survives restarts.
        // The socket-protocol set_config whitelist doesn't include this key
        // on purpose - it's local UX state, not protocol config.
        let value = serde_json::to_string(&target)
            .map_err(|e| BackendError::Db(format!("serialize backend target: {e}")))?;
        self.db
            .set_config(CURRENT_BACKEND_KEY, &value)
            .map_err(|e| BackendError::Db(e.to_string()))?;
        Ok(())
    }

    /// Open a `BackendClient` against the current target. For Remote this
    /// ensures the SSH tunnel is up before returning.
    pub fn client(&self) -> Result<BackendClient, BackendError> {
        let target = self.current();
        self.client_for(&target)
    }

    /// Open a `BackendClient` against a specific target without changing
    /// the "current" selection. Used by `test_remote_backend` so a Test
    /// button in Settings doesn't yank the rest of the UI out of Local.
    pub fn client_for(&self, target: &BackendTarget) -> Result<BackendClient, BackendError> {
        match target {
            BackendTarget::Local => Ok(BackendClient::Local(self.db.clone())),
            BackendTarget::Remote { name } => {
                let path = self.manager.ensure_tunnel(name)?;
                Ok(BackendClient::Remote(Client::new(path)))
            }
        }
    }
}

fn load_persisted_target(db: &Database) -> Option<BackendTarget> {
    let value = db.get_config(CURRENT_BACKEND_KEY).ok()?;
    serde_json::from_str(&value).ok()
}

// --- Helpers for socket liveness ---

/// Cheap liveness probe: the tunnel is open iff connecting to the local
/// socket succeeds. We don't try to send a real protocol message - the
/// portsage server will close us out if we just open and drop, but that's
/// fine; we only need to know the socket is bindable from our end.
fn is_socket_alive(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    std::os::unix::net::UnixStream::connect(path).is_ok()
}

fn wait_for_socket(path: &Path, timeout: Duration) -> Result<(), BackendError> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if is_socket_alive(path) {
            return Ok(());
        }
        std::thread::sleep(TUNNEL_OPEN_POLL);
    }
    Err(BackendError::TunnelTimeout(
        path.to_string_lossy().into_owned(),
        timeout,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, RemoteBackendInput};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    fn fresh_db() -> Arc<Database> {
        Arc::new(Database::in_memory().expect("in-memory db"))
    }

    fn add_backend(db: &Database, name: &str, local: &Path, remote: &Path) -> RemoteBackend {
        db.create_remote_backend(RemoteBackendInput {
            name,
            ssh_alias: "test-alias",
            remote_socket_path: &remote.to_string_lossy(),
            local_socket_path: &local.to_string_lossy(),
            auto_forward_enabled: false,
        })
        .unwrap()
    }

    /// Test launcher that simulates ssh by binding a UnixListener on the
    /// local socket path. The "tunnel" never forwards anything, but to the
    /// BackendManager it looks like a connectable forward. We model the
    /// `Child` as a long-lived sleep so kill/wait behave naturally.
    #[derive(Debug)]
    struct FakeLauncher {
        // Keep the listener alive for the lifetime of the launcher so the
        // bound socket stays connectable after `spawn` returns.
        listeners: Mutex<Vec<UnixListener>>,
        spawn_count: AtomicUsize,
    }

    impl FakeLauncher {
        fn new() -> Self {
            Self {
                listeners: Mutex::new(Vec::new()),
                spawn_count: AtomicUsize::new(0),
            }
        }

        fn spawned(&self) -> usize {
            self.spawn_count.load(Ordering::SeqCst)
        }
    }

    impl TunnelLauncher for FakeLauncher {
        fn spawn(&self, backend: &RemoteBackend) -> Result<Child, BackendError> {
            self.spawn_count.fetch_add(1, Ordering::SeqCst);
            let path = Path::new(&backend.local_socket_path);
            let _ = std::fs::remove_file(path);
            let listener =
                UnixListener::bind(path).map_err(|e| BackendError::SshSpawn(e.to_string()))?;
            self.listeners
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(listener);
            // A long sleep stands in for the long-lived `ssh -N` process. The
            // BackendManager will kill it on close_tunnel.
            Command::new("sleep")
                .arg("3600")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| BackendError::SshSpawn(e.to_string()))
        }
    }

    /// Launcher that always fails synchronously. Models "ssh binary missing"
    /// or "spawn EACCES".
    #[derive(Debug)]
    struct FailingLauncher;

    impl TunnelLauncher for FailingLauncher {
        fn spawn(&self, _backend: &RemoteBackend) -> Result<Child, BackendError> {
            Err(BackendError::SshSpawn("nope".into()))
        }
    }

    #[test]
    fn backend_target_local_returns_default_socket() {
        let db = fresh_db();
        let mgr = BackendManager::new(db);
        let client = mgr.connect(&BackendTarget::Local).unwrap();
        assert_eq!(client.socket_path(), paths::socket_path().as_path());
    }

    #[test]
    fn remote_target_unknown_backend_errors() {
        let db = fresh_db();
        let mgr = BackendManager::new(db);
        let err = mgr
            .connect(&BackendTarget::Remote {
                name: "ghost".into(),
            })
            .unwrap_err();
        assert!(
            matches!(err, BackendError::UnknownBackend(_)),
            "got: {err}",
        );
    }

    #[test]
    fn remote_target_opens_tunnel_via_launcher() {
        let dir = TempDir::new().unwrap();
        let local = dir.path().join("dev.sock");
        let remote = dir.path().join("remote.sock");
        let db = fresh_db();
        add_backend(&db, "dev", &local, &remote);

        let launcher = Arc::new(FakeLauncher::new());
        let mgr = BackendManager::with_launcher(db, launcher.clone());
        let client = mgr
            .connect(&BackendTarget::Remote { name: "dev".into() })
            .unwrap();
        assert_eq!(client.socket_path(), local.as_path());
        assert_eq!(launcher.spawned(), 1);
        // Second connect reuses the open tunnel - no new spawn.
        let _ = mgr
            .connect(&BackendTarget::Remote { name: "dev".into() })
            .unwrap();
        assert_eq!(launcher.spawned(), 1);
    }

    #[test]
    fn close_tunnel_resets_state_so_next_connect_respawns() {
        let dir = TempDir::new().unwrap();
        let local = dir.path().join("dev.sock");
        let remote = dir.path().join("remote.sock");
        let db = fresh_db();
        add_backend(&db, "dev", &local, &remote);

        let launcher = Arc::new(FakeLauncher::new());
        let mgr = BackendManager::with_launcher(db, launcher.clone());
        mgr.connect(&BackendTarget::Remote { name: "dev".into() })
            .unwrap();
        assert_eq!(launcher.spawned(), 1);
        mgr.close_tunnel("dev").unwrap();
        mgr.connect(&BackendTarget::Remote { name: "dev".into() })
            .unwrap();
        assert_eq!(launcher.spawned(), 2);
    }

    #[test]
    fn close_tunnel_is_idempotent_on_unknown_name() {
        let db = fresh_db();
        let mgr = BackendManager::new(db);
        // No backend, no tunnel - close should be a no-op, not an error.
        mgr.close_tunnel("never-existed").unwrap();
    }

    #[test]
    fn launcher_failure_propagates_and_marks_state_failed() {
        let dir = TempDir::new().unwrap();
        let local = dir.path().join("dev.sock");
        let remote = dir.path().join("remote.sock");
        let db = fresh_db();
        add_backend(&db, "dev", &local, &remote);

        let mgr = BackendManager::with_launcher(db, Arc::new(FailingLauncher));
        let err = mgr
            .connect(&BackendTarget::Remote { name: "dev".into() })
            .unwrap_err();
        assert!(matches!(err, BackendError::SshSpawn(_)), "got: {err}");

        let statuses = mgr.statuses();
        assert_eq!(statuses.len(), 1);
        assert!(matches!(statuses[0].state, TunnelState::Failed { .. }));
    }

    #[test]
    fn statuses_includes_every_known_backend() {
        let dir = TempDir::new().unwrap();
        let local_a = dir.path().join("a.sock");
        let local_b = dir.path().join("b.sock");
        let remote = dir.path().join("r.sock");
        let db = fresh_db();
        add_backend(&db, "alpha", &local_a, &remote);
        add_backend(&db, "bravo", &local_b, &remote);

        let launcher = Arc::new(FakeLauncher::new());
        let mgr = BackendManager::with_launcher(db, launcher);
        mgr.connect(&BackendTarget::Remote {
            name: "alpha".into(),
        })
        .unwrap();
        mgr.connect(&BackendTarget::Remote {
            name: "bravo".into(),
        })
        .unwrap();

        let statuses = mgr.statuses();
        let names: std::collections::HashSet<_> =
            statuses.iter().map(|s| s.backend_name.clone()).collect();
        assert!(names.contains("alpha"));
        assert!(names.contains("bravo"));
        assert!(statuses
            .iter()
            .all(|s| matches!(s.state, TunnelState::Connected)));
    }

    #[test]
    fn tunnel_state_serialization_is_tagged() {
        // The frontend reads tunnel state via Tauri events, so the JSON shape
        // matters. We use serde's adjacently-tagged form via `tag = "state"`.
        let s = serde_json::to_string(&TunnelState::Connected).unwrap();
        assert_eq!(s, r#"{"state":"connected"}"#);
        let f = serde_json::to_string(&TunnelState::Failed {
            reason: "boom".into(),
        })
        .unwrap();
        assert_eq!(f, r#"{"state":"failed","reason":"boom"}"#);
    }

    #[test]
    fn backend_target_serialization() {
        let s = serde_json::to_string(&BackendTarget::Local).unwrap();
        assert_eq!(s, r#"{"kind":"local"}"#);
        let r = serde_json::to_string(&BackendTarget::Remote { name: "dev".into() }).unwrap();
        assert_eq!(r, r#"{"kind":"remote","name":"dev"}"#);
    }

    // --- BackendRouter / BackendClient ---

    #[test]
    fn router_defaults_to_local() {
        let db = fresh_db();
        let router = BackendRouter::new(db);
        assert_eq!(router.current(), BackendTarget::Local);
    }

    #[test]
    fn router_set_current_unknown_remote_errors() {
        let db = fresh_db();
        let router = BackendRouter::new(db);
        let err = router
            .set_current(BackendTarget::Remote {
                name: "ghost".into(),
            })
            .unwrap_err();
        assert!(
            matches!(err, BackendError::UnknownBackend(_)),
            "got: {err}",
        );
    }

    #[test]
    fn router_persists_target_across_rebuild() {
        // The same Database (and on-disk equivalent) reused by a second router
        // instance should restore the persisted target. Without persistence,
        // the user loses their selection on every app restart.
        let dir = TempDir::new().unwrap();
        let local = dir.path().join("dev.sock");
        let remote = dir.path().join("r.sock");
        let db = fresh_db();
        add_backend(&db, "dev", &local, &remote);

        let r1 = BackendRouter::new(db.clone());
        r1.set_current(BackendTarget::Remote { name: "dev".into() })
            .unwrap();
        assert_eq!(r1.current(), BackendTarget::Remote { name: "dev".into() });

        let r2 = BackendRouter::new(db);
        assert_eq!(r2.current(), BackendTarget::Remote { name: "dev".into() });
    }

    #[test]
    fn backend_client_local_list_all_round_trip() {
        let db = fresh_db();
        let router = BackendRouter::new(db);
        let client = router.client_for(&BackendTarget::Local).unwrap();
        client.reserve_range("alpha", Some("/tmp/alpha")).unwrap();
        let projects = client.list_all().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "alpha");
    }

    #[test]
    fn backend_client_local_register_port_resolves_project_by_name() {
        let db = fresh_db();
        let router = BackendRouter::new(db);
        let client = router.client_for(&BackendTarget::Local).unwrap();
        client.reserve_range("alpha", None).unwrap();
        let p = client.register_port("alpha", "vite", 4000).unwrap();
        assert_eq!(p.port, 4000);
        assert_eq!(p.service, "vite");
    }

    #[test]
    fn backend_client_local_register_unknown_project_errors() {
        let db = fresh_db();
        let router = BackendRouter::new(db);
        let client = router.client_for(&BackendTarget::Local).unwrap();
        let err = client.register_port("ghost", "vite", 4000).unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
    }

    #[test]
    fn backend_client_local_release_project_round_trips() {
        let db = fresh_db();
        let router = BackendRouter::new(db);
        let client = router.client_for(&BackendTarget::Local).unwrap();
        client.reserve_range("alpha", None).unwrap();
        client.release_project("alpha").unwrap();
        assert!(client.list_all().unwrap().is_empty());
    }

    #[test]
    fn backend_client_set_config_rejects_unknown_key() {
        let db = fresh_db();
        let router = BackendRouter::new(db);
        let client = router.client_for(&BackendTarget::Local).unwrap();
        let err = client.set_config("arbitrary", "x").unwrap_err();
        assert!(err.contains("unknown config key"), "got: {err}");
    }
}
