use crate::types::{
    ActivePort, ConfigSnapshot, KillEntry, KillOutcome, PortStatus, ProjectStatus, RangeBounds,
    RemoteBackend,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Default connect timeout. The socket is local; failure beyond this almost
/// certainly means the backend is not running rather than congested.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

/// Default read timeout for one request/response cycle. The longest synchronous
/// call is `kill_port`/`kill_project`, which can sit for ~2 seconds of grace
/// period. We add headroom for slow IO so legitimate kills don't time out.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the autospawn flow polls for the socket file after launching the
/// backend. Warm spawns bind the socket in a few hundred ms, but a cold first
/// launch after install can take several seconds on macOS (Gatekeeper xattr
/// scan, dyld cache priming, etc.). 8 s covers the slow path without making
/// real failures (binary missing, permission denied) feel hung.
pub const AUTOSPAWN_TIMEOUT: Duration = Duration::from_secs(8);

/// Interval between socket-existence polls during autospawn.
pub const AUTOSPAWN_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The socket file does not exist and the backend cannot be auto-started
    /// (either AutoSpawn was disabled or the binary could not be located).
    #[error(
        "Portsage backend is not running. Launch the Portsage app, or pass `--app` / set $PORTSAGE_APP."
    )]
    AppNotRunning,

    /// Autospawn was attempted but the socket did not appear within the timeout.
    #[error("Portsage backend was spawned but did not start within {0:?}")]
    SpawnTimeout(Duration),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("timeout reading response")]
    ReadTimeout,

    /// The backend responded with `{"error": "..."}` to a method call.
    #[error("backend: {0}")]
    Server(String),

    /// The response could not be parsed as the expected shape.
    #[error("could not parse backend response: {0}")]
    Parse(String),
}

/// Whether the client should launch the backend if the socket is missing.
#[derive(Debug, Clone)]
#[derive(Default)]
pub enum AutoSpawn {
    /// Do not attempt to spawn. Connection failures surface as `AppNotRunning`.
    #[default]
    Disabled,
    /// Attempt to spawn the backend in `--headless` mode. The binary is located
    /// via (in order): the supplied `app_path`, the `$PORTSAGE_APP` env var,
    /// and platform-specific defaults (e.g. `/Applications/Portsage.app/...`
    /// on macOS).
    Enabled { app_path: Option<PathBuf> },
}


/// Synchronous client for the Portsage Unix socket. Every method opens a
/// fresh connection; the backend's 60 s idle timeout makes long-lived
/// connections more trouble than they're worth for the typical short-lived
/// CLI invocation.
#[derive(Debug, Clone)]
pub struct Client {
    socket_path: PathBuf,
    autospawn: AutoSpawn,
    read_timeout: Duration,
}

impl Client {
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            autospawn: AutoSpawn::Disabled,
            read_timeout: DEFAULT_READ_TIMEOUT,
        }
    }

    pub fn with_autospawn(mut self, autospawn: AutoSpawn) -> Self {
        self.autospawn = autospawn;
        self
    }

    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    // --- low-level send/recv ---

    fn open_stream(&self) -> Result<UnixStream, ClientError> {
        match UnixStream::connect(&self.socket_path) {
            Ok(s) => Ok(s),
            Err(e) => match self.autospawn {
                AutoSpawn::Disabled => Err(map_connect_error(e)),
                AutoSpawn::Enabled { ref app_path } => {
                    spawn_headless(app_path.as_deref())?;
                    wait_for_socket(&self.socket_path, AUTOSPAWN_TIMEOUT)?;
                    UnixStream::connect(&self.socket_path).map_err(map_connect_error)
                }
            },
        }
    }

    fn call_raw(&self, request: Value) -> Result<Value, ClientError> {
        let mut stream = self.open_stream()?;
        stream.set_read_timeout(Some(self.read_timeout))?;
        stream.set_write_timeout(Some(self.read_timeout))?;

        let mut payload =
            serde_json::to_string(&request).map_err(|e| ClientError::Parse(e.to_string()))?;
        payload.push('\n');
        stream.write_all(payload.as_bytes())?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let read = reader.read_line(&mut line);
        match read {
            Ok(0) => return Err(ClientError::Parse("backend closed connection".into())),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(ClientError::ReadTimeout);
            }
            Err(e) => return Err(ClientError::Io(e)),
        }

        let response: Value =
            serde_json::from_str(line.trim()).map_err(|e| ClientError::Parse(e.to_string()))?;

        if let Some(err) = response.get("error").and_then(|v| v.as_str()) {
            return Err(ClientError::Server(err.to_string()));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    fn call<R: DeserializeOwned>(&self, request: Value) -> Result<R, ClientError> {
        let result = self.call_raw(request)?;
        serde_json::from_value(result).map_err(|e| ClientError::Parse(e.to_string()))
    }

    // --- methods, in the same order as the socket dispatcher ---

    pub fn list_all(&self) -> Result<Vec<ProjectStatus>, ClientError> {
        self.call(json!({ "method": "list_all" }))
    }

    pub fn reserve_range(
        &self,
        name: &str,
        path: Option<&str>,
    ) -> Result<ProjectStatus, ClientError> {
        let mut params = json!({ "name": name });
        if let Some(p) = path {
            params["path"] = json!(p);
        }
        self.call(json!({ "method": "reserve_range", "params": params }))
    }

    pub fn register_port(
        &self,
        project: &str,
        service: &str,
        port: i64,
    ) -> Result<PortStatus, ClientError> {
        // The server emits a full PortStatus with active/process/pid set to
        // inactive defaults right after insertion; if the port is actually
        // listening, the caller can re-fetch via `list_all`.
        self.call(json!({
            "method": "register_port",
            "params": { "project": project, "service": service, "port": port },
        }))
    }

    pub fn remove_port(&self, project: &str, service: &str) -> Result<(), ClientError> {
        let _: serde_json::Value = self.call(json!({
            "method": "remove_port",
            "params": { "project": project, "service": service },
        }))?;
        Ok(())
    }

    pub fn release_project(&self, name: &str) -> Result<(), ClientError> {
        let _: serde_json::Value = self.call(json!({
            "method": "release_project",
            "params": { "name": name },
        }))?;
        Ok(())
    }

    pub fn scan_active(&self) -> Result<Vec<ActivePort>, ClientError> {
        self.call(json!({ "method": "scan_active" }))
    }

    pub fn list_unmanaged(&self) -> Result<Vec<ActivePort>, ClientError> {
        self.call(json!({ "method": "list_unmanaged" }))
    }

    pub fn next_range(&self) -> Result<RangeBounds, ClientError> {
        self.call(json!({ "method": "next_range" }))
    }

    pub fn get_config(&self) -> Result<ConfigSnapshot, ClientError> {
        self.call(json!({ "method": "get_config" }))
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<(), ClientError> {
        let _: serde_json::Value = self.call(json!({
            "method": "set_config",
            "params": { "key": key, "value": value },
        }))?;
        Ok(())
    }

    pub fn kill_port(&self, port: i64) -> Result<KillOutcome, ClientError> {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            outcome: KillOutcome,
        }
        let wrapper: Wrapper = self.call(json!({
            "method": "kill_port",
            "params": { "port": port },
        }))?;
        Ok(wrapper.outcome)
    }

    pub fn kill_project(&self, name: &str) -> Result<Vec<KillEntry>, ClientError> {
        self.call(json!({
            "method": "kill_project",
            "params": { "name": name },
        }))
    }

    pub fn open_in_browser(&self, port: i64) -> Result<(), ClientError> {
        let _: serde_json::Value = self.call(json!({
            "method": "open_in_browser",
            "params": { "port": port },
        }))?;
        Ok(())
    }

    pub fn find_project_by_path(&self, path: &str) -> Result<Option<ProjectStatus>, ClientError> {
        self.call(json!({
            "method": "find_project_by_path",
            "params": { "path": path },
        }))
    }

    /// Look up a remote-backend row by name on the (Mac) socket. Used by the
    /// CLI's `--backend <name>` flag to discover the local-side forwarded
    /// socket path without poking the Mac's SQLite file directly.
    pub fn get_remote_backend(&self, name: &str) -> Result<Option<RemoteBackend>, ClientError> {
        self.call(json!({
            "method": "get_remote_backend",
            "params": { "name": name },
        }))
    }
}

// --- autospawn helpers ---

fn map_connect_error(err: std::io::Error) -> ClientError {
    match err.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            ClientError::AppNotRunning
        }
        _ => ClientError::Io(err),
    }
}

fn spawn_headless(hint: Option<&Path>) -> Result<(), ClientError> {
    let bin = locate_app_binary(hint).ok_or(ClientError::AppNotRunning)?;
    Command::new(&bin)
        .arg("--headless")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(ClientError::Io)?;
    Ok(())
}

fn locate_app_binary(hint: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = hint {
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    if let Some(env) = std::env::var_os("PORTSAGE_APP") {
        let p = PathBuf::from(env);
        if p.exists() {
            return Some(p);
        }
    }
    if cfg!(target_os = "macos") {
        let sys = PathBuf::from("/Applications/Portsage.app/Contents/MacOS/portsage");
        if sys.exists() {
            return Some(sys);
        }
        if let Some(home) = std::env::var_os("HOME") {
            let user =
                PathBuf::from(home).join("Applications/Portsage.app/Contents/MacOS/portsage");
            if user.exists() {
                return Some(user);
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        // The Linux build is headless-only and ships as `portsage-server`. The
        // systemd unit is the expected production install path, but autospawn
        // still lets `portsage <cmd>` work for users who installed by hand.
        for candidate in ["/usr/local/bin/portsage-server", "/usr/bin/portsage-server"] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Some(p);
            }
        }
        if let Some(home) = std::env::var_os("HOME") {
            let user = PathBuf::from(home).join(".local/bin/portsage-server");
            if user.exists() {
                return Some(user);
            }
        }
    }
    None
}

fn wait_for_socket(path: &Path, timeout: Duration) -> Result<(), ClientError> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return Ok(());
        }
        std::thread::sleep(AUTOSPAWN_POLL_INTERVAL);
    }
    Err(ClientError::SpawnTimeout(timeout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader as StdBufReader;
    #[allow(unused_imports)]
    use std::io::{BufRead, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;
    use std::thread;

    /// Spin up a minimal echo-style mock server on a temp socket path. The
    /// `handler` closure receives the request line and returns the response
    /// line (without trailing newline). The server handles a single connection
    /// and then exits.
    fn spawn_mock_server<F>(handler: F) -> (PathBuf, tempfile::TempDir)
    where
        F: Fn(String) -> String + Send + 'static,
    {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("portsage.sock");
        let listener = UnixListener::bind(&path).expect("bind socket");

        let (ready_tx, ready_rx) = mpsc::channel();
        let server_path = path.clone();
        thread::spawn(move || {
            ready_tx.send(()).ok();
            for stream in listener.incoming().flatten() {
                let mut reader = StdBufReader::new(stream.try_clone().expect("clone"));
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let mut response = handler(line.trim().to_string());
                response.push('\n');
                let mut writer = stream;
                if writer.write_all(response.as_bytes()).is_err() {
                    continue;
                }
            }
            drop(server_path);
        });
        ready_rx.recv().ok();
        (path, dir)
    }

    #[test]
    fn list_all_round_trips_through_mock_server() {
        let (path, _dir) = spawn_mock_server(|req| {
            assert!(
                req.contains("\"method\":\"list_all\""),
                "request was: {req}"
            );
            r#"{"result":[{"id":1,"name":"alpha","path":null,"range_start":4000,"range_end":4009,"created_at":"t","ports":[]}]}"#.into()
        });
        let client = Client::new(path);
        let projects = client.list_all().expect("list_all");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "alpha");
        assert_eq!(projects[0].range_start, 4000);
    }

    #[test]
    fn server_error_surfaces_as_client_error_server() {
        let (path, _dir) =
            spawn_mock_server(|_req| r#"{"error":"project 'ghost' not found"}"#.into());
        let client = Client::new(path);
        let err = client.release_project("ghost").unwrap_err();
        match err {
            ClientError::Server(msg) => assert!(msg.contains("not found"), "got: {msg}"),
            other => panic!("expected Server error, got {other:?}"),
        }
    }

    #[test]
    fn missing_socket_yields_app_not_running() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.sock");
        let client = Client::new(path);
        let err = client.list_all().unwrap_err();
        assert!(matches!(err, ClientError::AppNotRunning), "got: {err:?}");
    }

    #[test]
    fn reserve_range_sends_path_when_provided() {
        let (path, _dir) = spawn_mock_server(|req| {
            assert!(req.contains("\"name\":\"alpha\""));
            assert!(req.contains("\"path\":\"/tmp/alpha\""));
            r#"{"result":{"id":1,"name":"alpha","path":"/tmp/alpha","range_start":4000,"range_end":4009,"created_at":"t","ports":[]}}"#.into()
        });
        let client = Client::new(path);
        let ps = client.reserve_range("alpha", Some("/tmp/alpha")).unwrap();
        assert_eq!(ps.path.as_deref(), Some("/tmp/alpha"));
    }

    #[test]
    fn register_port_deserializes_full_port_status() {
        // The server emits a fully-shaped PortStatus (with inactive defaults
        // immediately after insertion); the client deserializes it directly.
        let (path, _dir) = spawn_mock_server(|_req| {
            r#"{"result":{"id":10,"project_id":1,"service":"vite","port":4000,"active":false,"process":null,"pid":null,"created_at":"t"}}"#.into()
        });
        let client = Client::new(path);
        let p = client.register_port("alpha", "vite", 4000).unwrap();
        assert_eq!(p.id, 10);
        assert!(!p.active);
        assert!(p.process.is_none());
        assert!(p.pid.is_none());
    }

    #[test]
    fn kill_port_unwraps_outcome_wrapper() {
        let (path, _dir) =
            spawn_mock_server(|_req| r#"{"result":{"outcome":"terminated"}}"#.into());
        let client = Client::new(path);
        let outcome = client.kill_port(4000).unwrap();
        assert_eq!(outcome, KillOutcome::Terminated);
    }

    #[test]
    fn find_project_by_path_returns_none_when_null() {
        let (path, _dir) = spawn_mock_server(|_req| r#"{"result":null}"#.into());
        let client = Client::new(path);
        let res = client.find_project_by_path("/var/log").unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn next_range_deserializes_bounds() {
        let (path, _dir) =
            spawn_mock_server(|_req| r#"{"result":{"range_start":4010,"range_end":4019}}"#.into());
        let client = Client::new(path);
        let r = client.next_range().unwrap();
        assert_eq!(r.range_start, 4010);
        assert_eq!(r.range_end, 4019);
    }

    #[test]
    fn config_snapshot_round_trip() {
        let (path, _dir) =
            spawn_mock_server(|_req| r#"{"result":{"base_port":"4000","range_size":"10"}}"#.into());
        let client = Client::new(path);
        let cfg = client.get_config().unwrap();
        assert_eq!(cfg.base_port, "4000");
        assert_eq!(cfg.range_size, "10");
    }

    #[test]
    fn kill_project_returns_entries() {
        let (path, _dir) = spawn_mock_server(|_req| {
            r#"{"result":[{"port":4000,"outcome":"terminated"},{"port":4001,"outcome":"killed"}]}"#
                .into()
        });
        let client = Client::new(path);
        let entries = client.kill_project("alpha").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].outcome, KillOutcome::Terminated);
        assert_eq!(entries[1].outcome, KillOutcome::Killed);
    }
}
