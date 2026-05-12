//! SSH local-forward lifecycle for remote backends. Phase 3 of the
//! multi-host evolution.
//!
//! Phase 2 (`backends.rs`) opens *the protocol tunnel* to
//! `/run/portsage/portsage.sock` on the remote box. This module opens
//! *per-port forwards* (e.g. `-L 4060:localhost:4060`) so that
//! `http://localhost:4060` from the Mac browser actually reaches the remote
//! service. Both layers share the same SSH ControlMaster: Phase 2 either
//! piggybacks on the user's `ControlMaster auto` config, or opens a
//! Portsage-managed master we then track for clean-up.
//!
//! Concurrency: the forward map is keyed by (backend_name, port). A
//! single outer mutex guards the map; the operations themselves are short
//! (one ssh exec each) so we don't bother with per-port locks. Two callers
//! syncing the same backend at once will both compute the desired set and
//! issue the same `-O forward` calls - ssh treats a redundant forward as
//! a no-op (`already requested` on stderr, success on exit code).

use crate::backends::{BackendManager, BackendTarget};
use crate::db::Database;
use crate::paths;
use crate::scanner;
use portsage_client::{ActivePort, ProjectStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    #[error("unknown remote backend: {0}")]
    UnknownBackend(String),

    #[error("database error: {0}")]
    Db(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("ssh error: {0}")]
    Ssh(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// State machine for a single port forward.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ForwardState {
    /// Forward request issued; we haven't yet observed it as `active`.
    Pending,
    /// Forward is open and (presumably) usable from this Mac.
    Active,
    /// Forward could not be opened or was torn down out-of-band.
    Failed { reason: String },
    /// Explicitly cancelled, either by the user or because the port was
    /// removed from the remote backend.
    Cancelled,
}

/// Public-facing snapshot of one forward.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForwardStatus {
    pub backend_name: String,
    pub port: i64,
    pub state: ForwardState,
}

/// Strategy for invoking the SSH binary. Tests inject a fake; production
/// uses `SystemSshForwardController`.
pub trait ForwardController: Send + Sync + std::fmt::Debug {
    /// `ssh -O check <alias>` - returns true if a usable ControlMaster
    /// is already running for `alias`. The `control_path` overrides the
    /// alias's default ControlPath; pass `None` to let `ssh` resolve from
    /// the user's `~/.ssh/config`.
    fn check_master(&self, alias: &str, control_path: Option<&Path>) -> bool;

    /// Open a Portsage-managed ControlMaster at `control_path`. Called when
    /// the user has not configured `ControlMaster auto` and we need a
    /// long-lived master to issue `-O forward` / `-O cancel`. The path is
    /// chosen by the caller and tracked for shutdown.
    fn open_master(&self, alias: &str, control_path: &Path) -> Result<(), ForwardError>;

    /// `ssh -O exit -S <control_path> <alias>` - graceful close of a
    /// master we opened.
    fn close_master(&self, alias: &str, control_path: &Path) -> Result<(), ForwardError>;

    /// `ssh -O forward -L <port>:localhost:<port> [-S <ctrl>] <alias>`.
    /// Idempotent: a redundant forward returns success on exit.
    fn open_forward(
        &self,
        alias: &str,
        port: i64,
        control_path: Option<&Path>,
    ) -> Result<(), ForwardError>;

    /// `ssh -O cancel -L <port>:localhost:<port> [-S <ctrl>] <alias>`.
    fn close_forward(
        &self,
        alias: &str,
        port: i64,
        control_path: Option<&Path>,
    ) -> Result<(), ForwardError>;
}

#[derive(Debug)]
pub struct SystemSshForwardController;

impl ForwardController for SystemSshForwardController {
    fn check_master(&self, alias: &str, control_path: Option<&Path>) -> bool {
        let mut cmd = Command::new("ssh");
        if let Some(p) = control_path {
            cmd.args(["-S", &p.to_string_lossy()]);
        }
        cmd.args(["-O", "check", alias])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        cmd.status().map(|s| s.success()).unwrap_or(false)
    }

    fn open_master(&self, alias: &str, control_path: &Path) -> Result<(), ForwardError> {
        // `-f -N -M` = detach, no remote command, master. ControlPersist
        // keeps the master alive briefly after the last client disconnects
        // so the next operation doesn't have to re-authenticate.
        let status = Command::new("ssh")
            .args([
                "-fNM",
                "-S",
                &control_path.to_string_lossy(),
                "-o",
                "ControlPersist=4h",
                alias,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| ForwardError::Ssh(format!("spawn ssh -M: {e}")))?;
        if status.status.success() {
            Ok(())
        } else {
            Err(ForwardError::Ssh(
                String::from_utf8_lossy(&status.stderr).trim().to_string(),
            ))
        }
    }

    fn close_master(&self, alias: &str, control_path: &Path) -> Result<(), ForwardError> {
        let _ = Command::new("ssh")
            .args(["-S", &control_path.to_string_lossy(), "-O", "exit", alias])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        // ssh -O exit can fail if the master is already gone; not fatal.
        Ok(())
    }

    fn open_forward(
        &self,
        alias: &str,
        port: i64,
        control_path: Option<&Path>,
    ) -> Result<(), ForwardError> {
        run_o(alias, "forward", port, control_path)
    }

    fn close_forward(
        &self,
        alias: &str,
        port: i64,
        control_path: Option<&Path>,
    ) -> Result<(), ForwardError> {
        run_o(alias, "cancel", port, control_path)
    }
}

fn run_o(
    alias: &str,
    op: &str,
    port: i64,
    control_path: Option<&Path>,
) -> Result<(), ForwardError> {
    let spec = format!("{}:localhost:{}", port, port);
    let mut cmd = Command::new("ssh");
    if let Some(p) = control_path {
        cmd.args(["-S", &p.to_string_lossy()]);
    }
    cmd.args(["-O", op, "-L", &spec, alias])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .map_err(|e| ForwardError::Ssh(format!("spawn ssh -O {op}: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // `-O cancel` against a non-existent forward fails noisily. Treat
        // "not found" as success: we only care about the end state being
        // "this port is not forwarded".
        if op == "cancel"
            && (msg.to_lowercase().contains("not found")
                || msg.to_lowercase().contains("no forwarding"))
        {
            return Ok(());
        }
        Err(ForwardError::Ssh(if msg.is_empty() {
            format!("ssh -O {op} failed with no stderr")
        } else {
            msg
        }))
    }
}

/// Per-backend information cached on demand from `BackendManager` so we
/// don't keep crossing the layer boundary during a sync.
#[derive(Debug, Clone)]
struct BackendBinding {
    name: String,
    ssh_alias: String,
    control_path: Option<PathBuf>,
}

/// Provides the list of locally-bound ports. Production uses the existing
/// `scanner` module; tests substitute a fixed list to exercise the
/// collision-detection branch without touching the host's ports.
pub trait LocalPortProbe: Send + Sync + std::fmt::Debug {
    fn list(&self) -> Vec<ActivePort>;
}

#[derive(Debug)]
pub struct SystemLocalProbe;

impl LocalPortProbe for SystemLocalProbe {
    fn list(&self) -> Vec<ActivePort> {
        scanner::scan_active_ports_detailed()
    }
}

/// Owns the per-(backend, port) forward state.
pub struct ForwardManager {
    db: Arc<Database>,
    backends: Arc<BackendManager>,
    controller: Arc<dyn ForwardController>,
    local_probe: Arc<dyn LocalPortProbe>,
    forwards: Mutex<HashMap<(String, i64), ForwardEntry>>,
    /// SSH aliases for which we opened a ControlMaster ourselves. App
    /// shutdown closes these; user-managed masters are never touched.
    managed_masters: Mutex<HashMap<String, PathBuf>>,
}

#[derive(Debug, Clone)]
struct ForwardEntry {
    state: ForwardState,
    #[allow(dead_code)]
    since: Option<Instant>,
}

impl std::fmt::Debug for ForwardManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForwardManager")
            .field("controller", &self.controller)
            .finish_non_exhaustive()
    }
}

impl ForwardManager {
    pub fn new(db: Arc<Database>, backends: Arc<BackendManager>) -> Self {
        Self::with_dependencies(
            db,
            backends,
            Arc::new(SystemSshForwardController),
            Arc::new(SystemLocalProbe),
        )
    }

    pub fn with_controller(
        db: Arc<Database>,
        backends: Arc<BackendManager>,
        controller: Arc<dyn ForwardController>,
    ) -> Self {
        Self::with_dependencies(db, backends, controller, Arc::new(SystemLocalProbe))
    }

    pub fn with_dependencies(
        db: Arc<Database>,
        backends: Arc<BackendManager>,
        controller: Arc<dyn ForwardController>,
        local_probe: Arc<dyn LocalPortProbe>,
    ) -> Self {
        Self {
            db,
            backends,
            controller,
            local_probe,
            forwards: Mutex::new(HashMap::new()),
            managed_masters: Mutex::new(HashMap::new()),
        }
    }

    /// Surface any local process holding `port` so we can warn the caller
    /// before issuing `ssh -O forward` (which would silently fail with a
    /// bind error). Returns the first match in scan order; ports
    /// duplicate-bound across IPv4/IPv6 are deduplicated by the scanner.
    fn local_holder(&self, port: i64) -> Option<ActivePort> {
        self.local_probe.list().into_iter().find(|p| p.port == port)
    }

    /// Reconcile open forwards against the desired set for `backend_name`.
    /// Desired set = registered ports on the remote minus excluded ports.
    ///
    /// Returns the resulting per-port statuses for the backend (in port
    /// order) so callers can refresh the UI without a second round-trip.
    pub fn sync(&self, backend_name: &str) -> Result<Vec<ForwardStatus>, ForwardError> {
        let binding = self.ensure_binding(backend_name)?;
        let backend_row = self
            .db
            .get_remote_backend_by_name(backend_name)
            .map_err(|e| ForwardError::Db(e.to_string()))?
            .ok_or_else(|| ForwardError::UnknownBackend(backend_name.to_string()))?;
        let exclusions: HashSet<i64> = self
            .db
            .list_forward_exclusions(backend_row.id)
            .map_err(|e| ForwardError::Db(e.to_string()))?
            .into_iter()
            .map(|e| e.port)
            .collect();

        // Desired set: registered ports on the remote, except the ones the
        // user explicitly blocked.
        let projects = self
            .backends
            .connect(&BackendTarget::Remote {
                name: backend_name.to_string(),
            })
            .map_err(|e| ForwardError::Backend(e.to_string()))?
            .list_all()
            .map_err(|e| ForwardError::Backend(e.to_string()))?;
        let desired: HashSet<i64> = projects
            .iter()
            .flat_map(|p: &ProjectStatus| p.ports.iter().map(|port| port.port))
            .filter(|p| !exclusions.contains(p))
            .collect();

        // Current set: keys in `self.forwards` for this backend whose state
        // is Active or Pending. Failed/Cancelled entries are treated as
        // "not currently forwarded".
        let current: HashSet<i64> = {
            let map = self.forwards.lock().unwrap_or_else(|p| p.into_inner());
            map.iter()
                .filter(|((b, _), e)| {
                    b == backend_name
                        && matches!(e.state, ForwardState::Active | ForwardState::Pending)
                })
                .map(|((_, port), _)| *port)
                .collect()
        };

        // Open missing. Probe local conflicts ahead of ssh so we mark
        // those ports Failed with a useful reason instead of letting ssh's
        // bind-failure leak out as an opaque error.
        for &port in desired.difference(&current) {
            if let Some(holder) = self.local_holder(port) {
                self.set_state(
                    backend_name,
                    port,
                    ForwardState::Failed {
                        reason: format!(
                            "port {} is in use locally by {} (pid {})",
                            port, holder.process, holder.pid
                        ),
                    },
                );
                continue;
            }
            self.set_state(backend_name, port, ForwardState::Pending);
            let result = self.controller.open_forward(
                &binding.ssh_alias,
                port,
                binding.control_path.as_deref(),
            );
            match result {
                Ok(()) => self.set_state(backend_name, port, ForwardState::Active),
                Err(e) => self.set_state(
                    backend_name,
                    port,
                    ForwardState::Failed {
                        reason: e.to_string(),
                    },
                ),
            }
        }

        // Close extra.
        for &port in current.difference(&desired) {
            let result = self.controller.close_forward(
                &binding.ssh_alias,
                port,
                binding.control_path.as_deref(),
            );
            match result {
                Ok(()) => self.set_state(backend_name, port, ForwardState::Cancelled),
                Err(e) => self.set_state(
                    backend_name,
                    port,
                    ForwardState::Failed {
                        reason: e.to_string(),
                    },
                ),
            }
        }

        Ok(self.statuses_for(backend_name))
    }

    /// Force-open a single forward (UI override, ignores excluded list).
    /// Before issuing the SSH call we probe the local port: if something
    /// else is listening there, ssh would still report success (the master
    /// channel is opened) but the bind would fail with a "bind: Address
    /// already in use" on stderr, leaving the user with a confusing state.
    /// Surface the conflict up-front instead.
    pub fn enable(&self, backend_name: &str, port: i64) -> Result<ForwardStatus, ForwardError> {
        let binding = self.ensure_binding(backend_name)?;
        if let Some(holder) = self.local_holder(port) {
            let reason = format!(
                "port {} is in use locally by {} (pid {})",
                port, holder.process, holder.pid
            );
            self.set_state(
                backend_name,
                port,
                ForwardState::Failed {
                    reason: reason.clone(),
                },
            );
            return Err(ForwardError::Ssh(reason));
        }
        self.set_state(backend_name, port, ForwardState::Pending);
        match self.controller.open_forward(
            &binding.ssh_alias,
            port,
            binding.control_path.as_deref(),
        ) {
            Ok(()) => {
                self.set_state(backend_name, port, ForwardState::Active);
                Ok(self.single_status(backend_name, port))
            }
            Err(e) => {
                let reason = e.to_string();
                self.set_state(
                    backend_name,
                    port,
                    ForwardState::Failed {
                        reason: reason.clone(),
                    },
                );
                Err(ForwardError::Ssh(reason))
            }
        }
    }

    /// Force-close a single forward (UI override).
    pub fn disable(&self, backend_name: &str, port: i64) -> Result<ForwardStatus, ForwardError> {
        let binding = self.ensure_binding(backend_name)?;
        self.controller
            .close_forward(&binding.ssh_alias, port, binding.control_path.as_deref())?;
        self.set_state(backend_name, port, ForwardState::Cancelled);
        Ok(self.single_status(backend_name, port))
    }

    /// Cancel every open forward for `backend_name`. Used on backend
    /// removal / auto-forward toggle-off.
    pub fn cancel_all(&self, backend_name: &str) -> Result<(), ForwardError> {
        let binding = match self.ensure_binding(backend_name) {
            Ok(b) => b,
            // If the backend row is already gone, treat the cancel as
            // already-done; the in-memory forwards just become orphans we
            // forget below.
            Err(ForwardError::UnknownBackend(_)) => {
                self.forget_backend(backend_name);
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        let ports: Vec<i64> = {
            let map = self.forwards.lock().unwrap_or_else(|p| p.into_inner());
            map.iter()
                .filter(|((b, _), _)| b == backend_name)
                .map(|((_, port), _)| *port)
                .collect()
        };
        for port in ports {
            let _ = self.controller.close_forward(
                &binding.ssh_alias,
                port,
                binding.control_path.as_deref(),
            );
            self.set_state(backend_name, port, ForwardState::Cancelled);
        }
        Ok(())
    }

    /// Statuses across every known forward, in (backend_name, port) order.
    pub fn statuses(&self) -> Vec<ForwardStatus> {
        let map = self.forwards.lock().unwrap_or_else(|p| p.into_inner());
        let mut out: Vec<_> = map
            .iter()
            .map(|((backend_name, port), entry)| ForwardStatus {
                backend_name: backend_name.clone(),
                port: *port,
                state: entry.state.clone(),
            })
            .collect();
        out.sort_by(|a, b| {
            (a.backend_name.as_str(), a.port).cmp(&(b.backend_name.as_str(), b.port))
        });
        out
    }

    /// Statuses for a single backend, in port order.
    pub fn statuses_for(&self, backend_name: &str) -> Vec<ForwardStatus> {
        let mut out: Vec<_> = self
            .statuses()
            .into_iter()
            .filter(|s| s.backend_name == backend_name)
            .collect();
        out.sort_by_key(|s| s.port);
        out
    }

    /// Close every Portsage-managed ControlMaster. Called on app quit.
    /// The user's own masters are never touched.
    pub fn shutdown(&self) {
        let managed = {
            let mut g = self
                .managed_masters
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            std::mem::take(&mut *g)
        };
        for (alias, path) in managed {
            let _ = self.controller.close_master(&alias, &path);
        }
    }

    fn single_status(&self, backend_name: &str, port: i64) -> ForwardStatus {
        let map = self.forwards.lock().unwrap_or_else(|p| p.into_inner());
        let state = map
            .get(&(backend_name.to_string(), port))
            .map(|e| e.state.clone())
            .unwrap_or(ForwardState::Cancelled);
        ForwardStatus {
            backend_name: backend_name.to_string(),
            port,
            state,
        }
    }

    fn set_state(&self, backend_name: &str, port: i64, state: ForwardState) {
        let mut map = self.forwards.lock().unwrap_or_else(|p| p.into_inner());
        let key = (backend_name.to_string(), port);
        let since = matches!(state, ForwardState::Active).then(Instant::now);
        map.insert(key, ForwardEntry { state, since });
    }

    fn forget_backend(&self, backend_name: &str) {
        let mut map = self.forwards.lock().unwrap_or_else(|p| p.into_inner());
        map.retain(|(b, _), _| b != backend_name);
    }

    /// Resolve the ssh alias for `backend_name` and ensure a usable
    /// ControlMaster exists. Returns the (alias, control_path) pair used
    /// by subsequent `-O forward` / `-O cancel` ops.
    fn ensure_binding(&self, backend_name: &str) -> Result<BackendBinding, ForwardError> {
        let row = self
            .db
            .get_remote_backend_by_name(backend_name)
            .map_err(|e| ForwardError::Db(e.to_string()))?
            .ok_or_else(|| ForwardError::UnknownBackend(backend_name.to_string()))?;
        let alias = row.ssh_alias.clone();
        let control_path = self.ensure_master(&alias)?;
        Ok(BackendBinding {
            name: backend_name.to_string(),
            ssh_alias: alias,
            control_path,
        })
    }

    /// If the user's `ssh_config` already has a ControlMaster running for
    /// `alias` we use that (no `-S` needed on subsequent calls). Otherwise
    /// we open a Portsage-managed master under
    /// `paths::state_dir()/cm-<alias>.sock` and track it so `shutdown` can
    /// close it cleanly.
    fn ensure_master(&self, alias: &str) -> Result<Option<PathBuf>, ForwardError> {
        // First, check if we already opened one ourselves.
        {
            let managed = self
                .managed_masters
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if let Some(p) = managed.get(alias) {
                if self.controller.check_master(alias, Some(p)) {
                    return Ok(Some(p.clone()));
                }
                // Stale managed master - fall through to re-open.
            }
        }

        // User-managed master?
        if self.controller.check_master(alias, None) {
            return Ok(None);
        }

        // Neither: open our own and track it.
        let state_dir = paths::state_dir();
        std::fs::create_dir_all(&state_dir).map_err(ForwardError::Io)?;
        let path = state_dir.join(format!("cm-{}.sock", alias));
        let _ = std::fs::remove_file(&path);
        self.controller.open_master(alias, &path)?;
        let mut managed = self
            .managed_masters
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        managed.insert(alias.to_string(), path.clone());
        Ok(Some(path))
    }
}

/// Periodic-sync interval. Acts as a safety net for events the Mac doesn't
/// observe directly - e.g. an MCP client on the remote box calling
/// `register_port`. Anything shorter and we waste battery; anything longer
/// and remote-driven port registrations linger un-forwarded.
pub const PERIODIC_SYNC_INTERVAL: Duration = Duration::from_secs(60);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RemoteBackendInput;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Records every controller call without invoking ssh. Tests check the
    /// log to verify the manager issues the correct commands. We model
    /// "master is alive at <path>" with a set so `check_master(.., Some(p))`
    /// returns true after we've opened a master on that path, matching
    /// what real ssh would do.
    #[derive(Debug, Default)]
    struct FakeController {
        opens: Mutex<Vec<(String, i64)>>,
        closes: Mutex<Vec<(String, i64)>>,
        master_opens: AtomicUsize,
        master_closes: AtomicUsize,
        existing_master: bool,
        live_managed: Mutex<HashSet<PathBuf>>,
        open_fails: Mutex<HashSet<i64>>,
    }

    impl FakeController {
        fn new() -> Self {
            Self::default()
        }
        fn with_existing_master() -> Self {
            Self {
                existing_master: true,
                ..Self::default()
            }
        }
        fn open_calls(&self) -> Vec<(String, i64)> {
            self.opens.lock().unwrap().clone()
        }
        fn close_calls(&self) -> Vec<(String, i64)> {
            self.closes.lock().unwrap().clone()
        }
        fn master_open_count(&self) -> usize {
            self.master_opens.load(Ordering::SeqCst)
        }
        fn master_close_count(&self) -> usize {
            self.master_closes.load(Ordering::SeqCst)
        }
        fn fail_open_for(&self, port: i64) {
            self.open_fails.lock().unwrap().insert(port);
        }
    }

    impl ForwardController for FakeController {
        fn check_master(&self, _alias: &str, control_path: Option<&Path>) -> bool {
            match control_path {
                None => self.existing_master,
                Some(p) => self.live_managed.lock().unwrap().contains(p),
            }
        }
        fn open_master(&self, _alias: &str, control_path: &Path) -> Result<(), ForwardError> {
            self.master_opens.fetch_add(1, Ordering::SeqCst);
            self.live_managed
                .lock()
                .unwrap()
                .insert(control_path.to_path_buf());
            Ok(())
        }
        fn close_master(&self, _alias: &str, control_path: &Path) -> Result<(), ForwardError> {
            self.master_closes.fetch_add(1, Ordering::SeqCst);
            self.live_managed.lock().unwrap().remove(control_path);
            Ok(())
        }
        fn open_forward(
            &self,
            alias: &str,
            port: i64,
            _control_path: Option<&Path>,
        ) -> Result<(), ForwardError> {
            if self.open_fails.lock().unwrap().contains(&port) {
                return Err(ForwardError::Ssh(format!(
                    "simulated open failure for {port}"
                )));
            }
            self.opens.lock().unwrap().push((alias.to_string(), port));
            Ok(())
        }
        fn close_forward(
            &self,
            alias: &str,
            port: i64,
            _control_path: Option<&Path>,
        ) -> Result<(), ForwardError> {
            self.closes.lock().unwrap().push((alias.to_string(), port));
            Ok(())
        }
    }

    fn fresh_db() -> Arc<Database> {
        Arc::new(Database::in_memory().unwrap())
    }

    /// Returns a fixed snapshot of "locally-bound ports". Tests use this to
    /// simulate a port collision without having to bind a real socket.
    #[derive(Debug, Default)]
    struct FakeProbe(Mutex<Vec<ActivePort>>);

    impl FakeProbe {
        fn empty() -> Self {
            Self(Mutex::new(Vec::new()))
        }
        fn holding(port: i64, process: &str, pid: i64) -> Self {
            Self(Mutex::new(vec![ActivePort {
                port,
                process: process.into(),
                pid,
            }]))
        }
    }

    impl LocalPortProbe for FakeProbe {
        fn list(&self) -> Vec<ActivePort> {
            self.0.lock().unwrap().clone()
        }
    }

    fn fm(
        db: Arc<Database>,
        ctrl: Arc<FakeController>,
        probe: Arc<dyn LocalPortProbe>,
    ) -> ForwardManager {
        let backends = Arc::new(BackendManager::new(db.clone()));
        ForwardManager::with_dependencies(db, backends, ctrl, probe)
    }

    fn add_backend(db: &Database, name: &str, local: &str, remote: &str) {
        db.create_remote_backend(RemoteBackendInput {
            name,
            ssh_alias: name,
            remote_socket_path: remote,
            local_socket_path: local,
            auto_forward_enabled: false,
        })
        .unwrap();
    }

    // For sync tests we need BackendManager.list_all to return a known set.
    // The real one talks to a remote portsage-server; we use the same
    // FakeLauncher trick the backends module already exercises - but the
    // sync path here also calls list_all on the *protocol* socket. To
    // avoid full integration of both fakes for every test, we test the
    // sync logic indirectly by calling `enable` / `disable` / `cancel_all`,
    // and write one focused integration-style test in `backends`-adjacent
    // territory for the full sync flow.

    #[test]
    fn ensure_master_uses_user_config_when_available() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::with_existing_master());
        let fm = fm(db, ctrl.clone(), Arc::new(FakeProbe::empty()));

        // Trigger ensure_master via enable; the master should not be opened
        // by us because check_master returns true.
        fm.enable("dev", 4000).unwrap();
        assert_eq!(ctrl.master_open_count(), 0);
        assert_eq!(ctrl.open_calls(), vec![("dev".to_string(), 4000)]);
    }

    #[test]
    fn ensure_master_opens_managed_master_when_user_has_none() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::new());
        let fm = fm(db, ctrl.clone(), Arc::new(FakeProbe::empty()));

        fm.enable("dev", 4000).unwrap();
        assert_eq!(ctrl.master_open_count(), 1);
        // Subsequent ops reuse the master.
        fm.enable("dev", 4001).unwrap();
        assert_eq!(ctrl.master_open_count(), 1);
    }

    #[test]
    fn shutdown_closes_only_managed_masters() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");

        // Case 1: user-managed master - we should NOT close it.
        let ctrl = Arc::new(FakeController::with_existing_master());
        let fm1 = fm(db.clone(), ctrl.clone(), Arc::new(FakeProbe::empty()));
        fm1.enable("dev", 4000).unwrap();
        fm1.shutdown();
        assert_eq!(ctrl.master_close_count(), 0);

        // Case 2: Portsage-managed master - we MUST close it.
        let ctrl2 = Arc::new(FakeController::new());
        let fm2 = fm(db, ctrl2.clone(), Arc::new(FakeProbe::empty()));
        fm2.enable("dev", 4000).unwrap();
        assert_eq!(ctrl2.master_open_count(), 1);
        fm2.shutdown();
        assert_eq!(ctrl2.master_close_count(), 1);
    }

    #[test]
    fn enable_records_active_state() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::with_existing_master());
        let fm = fm(db, ctrl, Arc::new(FakeProbe::empty()));

        let status = fm.enable("dev", 4000).unwrap();
        assert_eq!(status.state, ForwardState::Active);
        assert_eq!(status.port, 4000);
        assert_eq!(status.backend_name, "dev");
    }

    #[test]
    fn enable_failure_records_failed_state_and_returns_err() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::with_existing_master());
        ctrl.fail_open_for(4000);
        let fm = fm(db, ctrl, Arc::new(FakeProbe::empty()));

        let err = fm.enable("dev", 4000).unwrap_err();
        assert!(matches!(err, ForwardError::Ssh(_)));
        // The recorded state must reflect the failure so the UI can show
        // the reason rather than dropping the port silently.
        let statuses = fm.statuses_for("dev");
        assert_eq!(statuses.len(), 1);
        assert!(matches!(statuses[0].state, ForwardState::Failed { .. }));
    }

    #[test]
    fn disable_records_cancelled() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::with_existing_master());
        let fm = fm(db, ctrl, Arc::new(FakeProbe::empty()));

        fm.enable("dev", 4000).unwrap();
        let status = fm.disable("dev", 4000).unwrap();
        assert_eq!(status.state, ForwardState::Cancelled);
    }

    #[test]
    fn cancel_all_iterates_all_open_forwards() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::with_existing_master());
        let fm = fm(db, ctrl.clone(), Arc::new(FakeProbe::empty()));

        fm.enable("dev", 4000).unwrap();
        fm.enable("dev", 4001).unwrap();
        fm.enable("dev", 4002).unwrap();
        fm.cancel_all("dev").unwrap();

        let closed_ports: Vec<i64> = ctrl.close_calls().into_iter().map(|(_, p)| p).collect();
        let closed_set: HashSet<i64> = closed_ports.into_iter().collect();
        assert_eq!(closed_set, [4000, 4001, 4002].iter().copied().collect());

        // States should all be Cancelled.
        let states: Vec<ForwardState> = fm
            .statuses_for("dev")
            .into_iter()
            .map(|s| s.state)
            .collect();
        assert!(states.iter().all(|s| matches!(s, ForwardState::Cancelled)));
    }

    #[test]
    fn cancel_all_unknown_backend_clears_in_memory_state_and_does_not_err() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::with_existing_master());
        let fm = fm(db.clone(), ctrl, Arc::new(FakeProbe::empty()));
        fm.enable("dev", 4000).unwrap();
        db.delete_remote_backend(1).unwrap();
        fm.cancel_all("dev").unwrap();
        assert!(fm.statuses_for("dev").is_empty());
    }

    #[test]
    fn statuses_are_sorted_by_backend_then_port() {
        let db = fresh_db();
        add_backend(&db, "alpha", "/tmp/a.sock", "/run/portsage/portsage.sock");
        add_backend(&db, "bravo", "/tmp/b.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::with_existing_master());
        let fm = fm(db, ctrl, Arc::new(FakeProbe::empty()));

        fm.enable("bravo", 5000).unwrap();
        fm.enable("alpha", 4001).unwrap();
        fm.enable("alpha", 4000).unwrap();

        let s = fm.statuses();
        let keys: Vec<(String, i64)> = s.into_iter().map(|x| (x.backend_name, x.port)).collect();
        assert_eq!(
            keys,
            vec![
                ("alpha".to_string(), 4000),
                ("alpha".to_string(), 4001),
                ("bravo".to_string(), 5000),
            ]
        );
    }

    #[test]
    fn enable_refuses_when_local_port_is_already_bound() {
        let db = fresh_db();
        add_backend(&db, "dev", "/tmp/dev.sock", "/run/portsage/portsage.sock");
        let ctrl = Arc::new(FakeController::with_existing_master());
        let probe = Arc::new(FakeProbe::holding(4060, "node", 12345));
        let fm = fm(db, ctrl.clone(), probe);

        let err = fm.enable("dev", 4060).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("port 4060 is in use locally")
                && msg.contains("node")
                && msg.contains("12345"),
            "got: {msg}"
        );
        // The SSH controller must NOT have been invoked - we want to short-
        // circuit before paying the round-trip cost.
        assert!(ctrl.open_calls().is_empty());

        let statuses = fm.statuses_for("dev");
        assert_eq!(statuses.len(), 1);
        assert!(matches!(statuses[0].state, ForwardState::Failed { .. }));
    }

    #[test]
    fn forward_state_serialization_is_tagged() {
        assert_eq!(
            serde_json::to_string(&ForwardState::Active).unwrap(),
            r#"{"state":"active"}"#
        );
        assert_eq!(
            serde_json::to_string(&ForwardState::Failed {
                reason: "boom".into()
            })
            .unwrap(),
            r#"{"state":"failed","reason":"boom"}"#
        );
    }
}
