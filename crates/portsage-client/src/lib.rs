//! Wire protocol types and a synchronous Unix-socket client for the Portsage
//! backend. This crate is the single source of truth for the protocol: the
//! Tauri app re-uses these types when serializing socket responses, and the
//! CLI uses the `Client` to call them.

mod client;
mod types;

pub use client::{
    AutoSpawn, Client, ClientError, AUTOSPAWN_POLL_INTERVAL, AUTOSPAWN_TIMEOUT,
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_READ_TIMEOUT,
};
pub use types::{
    ActivePort, ConfigSnapshot, KillEntry, KillOutcome, PortStatus, ProjectStatus, RangeBounds,
    RemoteBackend, RestoreOutcome, TrashEntry, TrashKind,
};

use std::path::PathBuf;

/// Default location of the Portsage Unix socket, matching what the Rust
/// backend creates.
///
/// Precedence:
///   1. `PORTSAGE_SOCKET` env var (used for the system-wide systemd socket
///      at `/run/portsage/portsage.sock` and any custom setup).
///   2. macOS: `~/Library/Application Support/portsage/portsage.sock`.
///   3. Linux: `$XDG_RUNTIME_DIR/portsage/portsage.sock`, falling back to
///      `/tmp/portsage-<uid>.sock` (uid via `$UID`, or `portsage.sock` if
///      unknown) when the runtime dir is unset.
///   4. Windows: `%APPDATA%/portsage/portsage.sock`.
///
/// `dirs` is a heavy dep for this single use; the lookup is reimplemented
/// inline so the client crate stays minimal.
pub fn default_socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("PORTSAGE_SOCKET") {
        return PathBuf::from(p);
    }

    if cfg!(target_os = "macos") {
        let base = std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"));
        return base
            .unwrap_or_else(|| PathBuf::from("."))
            .join("portsage")
            .join("portsage.sock");
    }

    if cfg!(target_os = "windows") {
        let base = std::env::var_os("APPDATA").map(PathBuf::from);
        return base
            .unwrap_or_else(|| PathBuf::from("."))
            .join("portsage")
            .join("portsage.sock");
    }

    // Linux and other unix: prefer XDG_RUNTIME_DIR (the systemd-user default),
    // fall back to /tmp keyed by uid so two users on the same box can each run
    // their own Portsage.
    if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(rt).join("portsage").join("portsage.sock");
    }
    let uid = std::env::var("UID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok());
    match uid {
        Some(u) => PathBuf::from(format!("/tmp/portsage-{}.sock", u)),
        None => PathBuf::from("/tmp/portsage.sock"),
    }
}

/// Default per-OS data directory for Portsage, where the SQLite DB lives and
/// where the CLI extracts the embedded MCP server files. Mirrors
/// `paths::data_dir` from the app crate so the CLI can resolve it without
/// pulling in `dirs` or the Tauri toolchain.
///
///   - macOS:   `~/Library/Application Support/portsage`
///   - Linux:   `$XDG_DATA_HOME/portsage`, default `~/.local/share/portsage`
///   - Windows: `%APPDATA%/portsage`
pub fn default_data_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        return std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("portsage");
    }

    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("portsage");
    }

    // Linux and other unix.
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("portsage")
}

#[cfg(test)]
mod data_dir_tests {
    use super::default_data_dir;

    #[test]
    fn default_data_dir_ends_in_portsage() {
        let p = default_data_dir();
        assert_eq!(p.file_name().and_then(|s| s.to_str()), Some("portsage"));
    }
}
