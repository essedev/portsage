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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KillOutcome {
    Terminated,
    Killed,
    NotActive,
    PermissionDenied,
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

/// Current global configuration snapshot. Values are returned as strings to
/// match the SQLite column type (the backend stores everything as TEXT).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigSnapshot {
    pub base_port: String,
    pub range_size: String,
}
