use serde::{Deserialize, Serialize};

/// A port row inside a project, enriched with live status from the host scanner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortStatus {
    pub id: i64,
    pub project_id: i64,
    pub service: String,
    pub port: i64,
    pub active: bool,
    pub process: Option<String>,
    pub pid: Option<i64>,
    pub created_at: String,
}

/// A project with its assigned range and the live status of every port.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectStatus {
    pub id: i64,
    pub name: String,
    pub path: Option<String>,
    pub range_start: i64,
    pub range_end: i64,
    pub created_at: String,
    /// Set when the project is shelved: it keeps its range, ports and name,
    /// and only drops out of the default listing. `None` for live projects.
    #[serde(default)]
    pub archived_at: Option<String>,
    pub ports: Vec<PortStatus>,
}

/// A TCP port currently in LISTEN on the host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivePort {
    pub port: i64,
    pub process: String,
    pub pid: i64,
}

/// Result of a kill attempt against a single PID.
///
/// The `Docker*` variants are emitted when the listening PID belongs to a
/// Docker port-forwarding proxy (`com.docker.backend`, `vpnkit`,
/// `docker-proxy`): we cannot kill the proxy without nuking every other
/// container's published port, so the action resolves the host port to its
/// container and calls `docker stop` instead. The failure cases stay
/// separate because each one asks something different of the user.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KillOutcome {
    Terminated,
    Killed,
    NotActive,
    PermissionDenied,
    /// `docker stop` succeeded on at least one container.
    DockerStopped,
    /// No docker CLI found in `PATH`, in the known install locations, or via
    /// `PORTSAGE_DOCKER_BIN`.
    DockerCliMissing,
    /// The CLI ran but could not reach the daemon (Docker Desktop stopped).
    DockerDaemonDown,
    /// Docker is reachable but no running container publishes that host port.
    DockerNoContainer,
    /// Anything else: `docker stop` refused, or an unexpected CLI failure.
    DockerError,
}

/// One entry returned by `kill_project`: the registered port that was active
/// and the outcome of attempting to kill its process.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct KillEntry {
    pub port: i64,
    pub outcome: KillOutcome,
}

/// Inclusive range bounds returned by `next_range`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RangeBounds {
    pub range_start: i64,
    pub range_end: i64,
}

/// Why a project turned up in `list_stale`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaleReason {
    /// The recorded path no longer exists. Usually a deleted project, but
    /// sometimes a moved one, in which case the fix is `update_project`
    /// with a new path rather than archiving.
    PathMissing,
    /// Nothing has touched the project directory or its git reflog for at
    /// least the requested number of days.
    Inactive,
}

/// A project that looks abandoned. Projects with a port listening right now
/// are never included, whatever their age: something is using that range.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaleProject {
    pub name: String,
    pub path: Option<String>,
    pub range_start: i64,
    pub range_end: i64,
    pub reason: StaleReason,
    /// Days since the last filesystem activity. `None` when the path is gone.
    pub inactive_days: Option<i64>,
    /// How many ports the project has registered, all of them idle.
    pub registered_ports: i64,
}

/// What a trash row holds: a whole project with its ports, or a single port.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrashKind {
    Project,
    Port,
}

/// One archived deletion, as shown in the trash view. The snapshot itself
/// stays server-side: clients only need enough to recognise what they are
/// about to restore, so `label` and `detail` are pre-rendered by the backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrashEntry {
    pub id: i64,
    pub kind: TrashKind,
    /// Project name, or `"<project> / <service>"` for a single port.
    pub label: String,
    /// One-line summary: `"range 4060-4069, 6 ports"` or `"port 4332"`.
    pub detail: String,
    pub deleted_at: String,
}

/// Result of restoring a trash entry. Ports already taken by a live project
/// are reported in `skipped_ports` rather than failing the whole restore: a
/// project that comes back missing one port is more useful than one that
/// cannot come back at all.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestoreOutcome {
    pub kind: TrashKind,
    pub project: String,
    pub restored_ports: Vec<i64>,
    pub skipped_ports: Vec<i64>,
}

/// Current global configuration snapshot. Values are returned as strings to
/// match the SQLite column type (the backend stores everything as TEXT).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigSnapshot {
    pub base_port: String,
    pub range_size: String,
}

/// A remote-backend catalogue row, returned by `get_remote_backend`. Exists
/// in the wire types so the CLI can ask the Mac socket for a backend's
/// `local_socket_path` and then point its own `Client` at that path. The
/// CLI does not open tunnels itself; that stays with the Mac UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteBackend {
    pub id: i64,
    pub name: String,
    pub ssh_alias: String,
    pub remote_socket_path: String,
    pub local_socket_path: String,
    pub auto_forward_enabled: bool,
    pub created_at: String,
}
