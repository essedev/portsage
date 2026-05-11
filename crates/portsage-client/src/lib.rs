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
};

use std::path::PathBuf;

/// Default location of the Portsage Unix socket, matching what the Rust
/// backend creates via `dirs::config_dir()` (~/Library/Application Support
/// on macOS, ~/.config on Linux, %APPDATA% on Windows).
pub fn default_socket_path() -> PathBuf {
    // dirs is a heavy dep for this single use; reimplement the lookup with
    // env vars so the client crate stays minimal.
    let base = if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("portsage")
        .join("portsage.sock")
}
