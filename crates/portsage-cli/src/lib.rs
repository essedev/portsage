pub mod cli;
pub mod output;

use crate::cli::{Cli, Command, ConfigAction, GlobalOpts};
use crate::output::OutputMode;
use portsage_client::{
    AutoSpawn, Client, ClientError, PortStatus,
};
use std::io::IsTerminal;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CliError {
    Client(ClientError),
    NoProjectAtCwd(PathBuf),
    NoTargetSpecified(&'static str),
    AbortedByUser,
    ServiceNotInProject(String, String),
    Io(std::io::Error),
}

impl From<ClientError> for CliError {
    fn from(e: ClientError) -> Self {
        CliError::Client(e)
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Io(e)
    }
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Client(ClientError::AppNotRunning) => 3,
            CliError::Client(ClientError::SpawnTimeout(_)) => 3,
            CliError::Client(ClientError::Server(msg)) => server_error_code(msg),
            CliError::Client(_) => 1,
            CliError::NoProjectAtCwd(_) => 4,
            CliError::ServiceNotInProject(_, _) => 4,
            CliError::NoTargetSpecified(_) => 2,
            CliError::AbortedByUser => 1,
            CliError::Io(_) => 1,
        }
    }

    pub fn message(&self) -> String {
        match self {
            CliError::Client(ClientError::AppNotRunning) => {
                "Portsage backend not reachable. Launch the menubar app, or run `portsage --headless` in another terminal.".into()
            }
            CliError::Client(e) => e.to_string(),
            CliError::NoProjectAtCwd(p) => {
                format!("no project registered for path {}. Run `portsage reserve --here` first.", p.display())
            }
            CliError::ServiceNotInProject(svc, name) => {
                format!("service {svc} is not registered in project {name}")
            }
            CliError::NoTargetSpecified(what) => {
                format!("must specify {what} or pass --here")
            }
            CliError::AbortedByUser => "aborted".into(),
            CliError::Io(e) => format!("io: {e}"),
        }
    }
}

pub fn server_error_code(msg: &str) -> u8 {
    let lower = msg.to_lowercase();
    if lower.contains("not found") {
        4
    } else if lower.contains("outside")
        || lower.contains("unique")
        || lower.contains("constraint")
        || lower.contains("duplicate")
    {
        5
    } else {
        1
    }
}

pub fn make_client(opts: &GlobalOpts) -> Client {
    let socket_path = opts
        .socket
        .clone()
        .unwrap_or_else(portsage_client::default_socket_path);
    let autospawn = if opts.no_autospawn {
        AutoSpawn::Disabled
    } else {
        AutoSpawn::Enabled {
            app_path: opts.app.clone(),
        }
    };
    Client::new(socket_path).with_autospawn(autospawn)
}

fn pwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolve a project name from an explicit argument or by looking up the
/// current working directory. Used by every cwd-aware subcommand.
fn resolve_project_name(
    client: &Client,
    name: Option<String>,
    here: bool,
    what: &'static str,
) -> Result<String, CliError> {
    if let Some(n) = name {
        return Ok(n);
    }
    if here {
        let cwd = pwd();
        let cwd_str = cwd.to_string_lossy().to_string();
        match client.find_project_by_path(&cwd_str)? {
            Some(p) => Ok(p.name),
            None => Err(CliError::NoProjectAtCwd(cwd)),
        }
    } else {
        Err(CliError::NoTargetSpecified(what))
    }
}

/// Prompt the user for a yes/no confirmation. If stdin is not a TTY and
/// `yes` is false, we refuse rather than silently auto-accept, so piped
/// invocations of destructive commands never act without `-y`.
fn confirm(prompt: &str, yes: bool) -> Result<(), CliError> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        output::print_error(
            "destructive command requires --yes when stdin is not a terminal",
        )?;
        return Err(CliError::AbortedByUser);
    }
    use std::io::Write;
    eprint!("{} [y/N] ", prompt);
    std::io::stderr().flush().ok();
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf)?;
    let answer = buf.trim().to_ascii_lowercase();
    if matches!(answer.as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(CliError::AbortedByUser)
    }
}

// === Subcommand handlers ===

fn cmd_list(
    client: &Client,
    mode: OutputMode,
    here: bool,
    project: Option<String>,
    active_only: bool,
) -> Result<(), CliError> {
    let mut projects = client.list_all()?;

    if here {
        let cwd = pwd();
        match client.find_project_by_path(&cwd.to_string_lossy())? {
            Some(p) => projects.retain(|x| x.name == p.name),
            None => return Err(CliError::NoProjectAtCwd(cwd)),
        }
    }
    if let Some(name) = project {
        projects.retain(|p| p.name == name);
        if projects.is_empty() {
            return Err(CliError::Client(ClientError::Server(format!(
                "project '{name}' not found"
            ))));
        }
    }
    if active_only {
        for p in projects.iter_mut() {
            p.ports.retain(|port| port.active);
        }
    }
    output::print_projects(mode, &projects)?;
    Ok(())
}

fn cmd_status(client: &Client, mode: OutputMode) -> Result<(), CliError> {
    let cwd = pwd();
    let cwd_str = cwd.to_string_lossy().to_string();
    let project = client
        .find_project_by_path(&cwd_str)?
        .ok_or(CliError::NoProjectAtCwd(cwd))?;
    output::print_project_detail(mode, &project)?;
    Ok(())
}

fn cmd_reserve(
    client: &Client,
    mode: OutputMode,
    name: Option<String>,
    path: Option<PathBuf>,
    here: bool,
) -> Result<(), CliError> {
    let path = match (path, here) {
        (Some(p), _) => Some(p),
        (None, true) => Some(pwd()),
        (None, false) => None,
    };
    let name = match name {
        Some(n) => n,
        None => match path.as_ref().and_then(|p| p.file_name()) {
            Some(base) => base.to_string_lossy().to_string(),
            None => return Err(CliError::NoTargetSpecified("a project name")),
        },
    };
    let path_str = path.as_ref().map(|p| p.to_string_lossy().to_string());
    let project = client.reserve_range(&name, path_str.as_deref())?;
    output::print_project_detail(mode, &project)?;
    Ok(())
}

fn cmd_register(
    client: &Client,
    mode: OutputMode,
    service: String,
    port: i64,
    project: Option<String>,
    here: bool,
) -> Result<(), CliError> {
    let name = resolve_project_name(client, project, here, "a project (use --project or --here)")?;
    let p = client.register_port(&name, &service, port)?;
    output::print_port(mode, &p)?;
    Ok(())
}

fn cmd_remove(
    client: &Client,
    mode: OutputMode,
    service: String,
    project: Option<String>,
    here: bool,
) -> Result<(), CliError> {
    let name = resolve_project_name(client, project, here, "a project (use --project or --here)")?;
    client.remove_port(&name, &service)?;
    output::print_message(mode, &format!("removed {service} from {name}"))?;
    Ok(())
}

fn cmd_release(
    client: &Client,
    mode: OutputMode,
    name: Option<String>,
    here: bool,
    yes: bool,
) -> Result<(), CliError> {
    let name =
        resolve_project_name(client, name, here, "a project name (or use --here)")?;
    confirm(&format!("release project {name} and all its ports?"), yes)?;
    client.release_project(&name)?;
    output::print_message(mode, &format!("released {name}"))?;
    Ok(())
}

fn cmd_scan(client: &Client, mode: OutputMode, unmanaged: bool) -> Result<(), CliError> {
    let ports = if unmanaged {
        client.list_unmanaged()?
    } else {
        client.scan_active()?
    };
    output::print_active_ports(mode, &ports)?;
    Ok(())
}

fn cmd_kill(client: &Client, mode: OutputMode, port: i64, yes: bool) -> Result<(), CliError> {
    confirm(&format!("kill process on port {port}?"), yes)?;
    let outcome = client.kill_port(port)?;
    output::print_kill_outcome(mode, port, outcome)?;
    Ok(())
}

fn cmd_kill_project(
    client: &Client,
    mode: OutputMode,
    name: Option<String>,
    here: bool,
    yes: bool,
) -> Result<(), CliError> {
    let name =
        resolve_project_name(client, name, here, "a project name (or use --here)")?;
    confirm(&format!("kill all active processes in project {name}?"), yes)?;
    let entries = client.kill_project(&name)?;
    output::print_kill_entries(mode, &entries)?;
    Ok(())
}

fn cmd_open(
    client: &Client,
    mode: OutputMode,
    target: String,
    project: Option<String>,
    here: bool,
) -> Result<(), CliError> {
    if let Ok(port) = target.parse::<i64>() {
        client.open_in_browser(port)?;
        output::print_message(mode, &format!("opened http://localhost:{port}"))?;
        return Ok(());
    }
    let name = resolve_project_name(
        client,
        project,
        here,
        "a project (use --project or --here) to resolve the service",
    )?;
    let projects = client.list_all()?;
    let project = projects
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            CliError::Client(ClientError::Server(format!("project '{name}' not found")))
        })?;
    let port_row: PortStatus = project
        .ports
        .into_iter()
        .find(|p| p.service == target)
        .ok_or_else(|| CliError::ServiceNotInProject(target.clone(), name.clone()))?;
    client.open_in_browser(port_row.port)?;
    output::print_message(
        mode,
        &format!("opened http://localhost:{} ({})", port_row.port, port_row.service),
    )?;
    Ok(())
}

fn cmd_config(
    client: &Client,
    mode: OutputMode,
    action: ConfigAction,
) -> Result<(), CliError> {
    match action {
        ConfigAction::Get => {
            let cfg = client.get_config()?;
            output::print_json(&cfg)?;
            let _ = mode;
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            client.set_config(&key, &value)?;
            output::print_message(mode, &format!("set {key} = {value}"))?;
            Ok(())
        }
    }
}

fn cmd_doctor(opts: &GlobalOpts, mode: OutputMode) -> Result<(), CliError> {
    use std::io::Write;
    let socket_path = opts
        .socket
        .clone()
        .unwrap_or_else(portsage_client::default_socket_path);
    let mut out = anstream::stdout().lock();
    if !matches!(mode, OutputMode::Json) {
        writeln!(out, "socket: {}", socket_path.display())?;
        writeln!(
            out,
            "  exists: {}",
            if socket_path.exists() { "yes" } else { "no" }
        )?;
    }

    let probe = Client::new(socket_path.clone())
        .with_read_timeout(std::time::Duration::from_millis(500))
        .list_all();
    let reachable = probe.is_ok();
    let mut report = serde_json::json!({
        "socket_path": socket_path.to_string_lossy(),
        "socket_file_exists": socket_path.exists(),
        "backend_reachable": reachable,
    });
    match &probe {
        Ok(projects) => {
            report["projects"] = serde_json::json!(projects.len());
            if !matches!(mode, OutputMode::Json) {
                writeln!(out, "  reachable: yes ({} projects)", projects.len())?;
            }
        }
        Err(e) => {
            report["backend_error"] = serde_json::json!(e.to_string());
            if !matches!(mode, OutputMode::Json) {
                writeln!(out, "  reachable: no ({})", e)?;
            }
        }
    }

    if matches!(mode, OutputMode::Json) {
        output::print_json(&report)?;
    }
    Ok(())
}

pub fn run(cli: Cli) -> Result<(), CliError> {
    let mode = OutputMode::from_flags(cli.global.json, cli.global.quiet);
    let client = make_client(&cli.global);

    match cli.command {
        Command::List { here, project, active } => cmd_list(&client, mode, here, project, active),
        Command::Status => cmd_status(&client, mode),
        Command::Reserve { name, path, here } => cmd_reserve(&client, mode, name, path, here),
        Command::Register { service, port, project, here } => {
            cmd_register(&client, mode, service, port, project, here)
        }
        Command::Remove { service, project, here } => {
            cmd_remove(&client, mode, service, project, here)
        }
        Command::Release { name, here, yes } => cmd_release(&client, mode, name, here, yes),
        Command::Scan { unmanaged } => cmd_scan(&client, mode, unmanaged),
        Command::Kill { port, yes } => cmd_kill(&client, mode, port, yes),
        Command::KillProject { name, here, yes } => {
            cmd_kill_project(&client, mode, name, here, yes)
        }
        Command::Open { target, project, here } => cmd_open(&client, mode, target, project, here),
        Command::Config { action } => cmd_config(&client, mode, action),
        Command::Doctor => cmd_doctor(&cli.global, mode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_error_code_maps_not_found_to_4() {
        assert_eq!(server_error_code("project 'x' not found"), 4);
        assert_eq!(server_error_code("Not Found"), 4);
    }

    #[test]
    fn server_error_code_maps_constraint_to_5() {
        assert_eq!(server_error_code("port 9999 is outside project range 4000-4009"), 5);
        assert_eq!(server_error_code("UNIQUE constraint failed: projects.name"), 5);
        assert_eq!(server_error_code("duplicate value"), 5);
    }

    #[test]
    fn server_error_code_falls_back_to_1() {
        assert_eq!(server_error_code("something unexpected"), 1);
    }

    #[test]
    fn cli_error_exit_codes_match_table() {
        assert_eq!(CliError::Client(ClientError::AppNotRunning).exit_code(), 3);
        assert_eq!(
            CliError::Client(ClientError::SpawnTimeout(std::time::Duration::from_secs(1)))
                .exit_code(),
            3
        );
        assert_eq!(
            CliError::Client(ClientError::Server("not found".into())).exit_code(),
            4
        );
        assert_eq!(
            CliError::Client(ClientError::Server("UNIQUE failed".into())).exit_code(),
            5
        );
        assert_eq!(CliError::AbortedByUser.exit_code(), 1);
        assert_eq!(CliError::NoTargetSpecified("foo").exit_code(), 2);
        assert_eq!(CliError::NoProjectAtCwd(PathBuf::from("/tmp")).exit_code(), 4);
    }

    #[test]
    fn output_mode_picks_json_first_then_quiet() {
        assert!(matches!(OutputMode::from_flags(true, false), OutputMode::Json));
        assert!(matches!(OutputMode::from_flags(false, true), OutputMode::Quiet));
        assert!(matches!(OutputMode::from_flags(true, true), OutputMode::Json));
        assert!(matches!(OutputMode::from_flags(false, false), OutputMode::Human));
    }

    // End-to-end: spin up a minimal mock socket server in a thread, point the
    // CLI at it via --socket, and verify a real subcommand round-trips. This
    // catches drift between the CLI's expectations and the actual wire shapes.
    fn spawn_canned_server<F: Fn(String) -> String + Send + 'static>(
        handler: F,
    ) -> (PathBuf, tempfile::TempDir) {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("portsage.sock");
        let listener = UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let mut resp = handler(line.trim().to_string());
                resp.push('\n');
                let mut writer = stream;
                let _ = writer.write_all(resp.as_bytes());
            }
        });
        (path, dir)
    }

    fn opts_with_socket(path: PathBuf) -> GlobalOpts {
        GlobalOpts {
            json: false,
            quiet: false,
            no_autospawn: true,
            app: None,
            socket: Some(path),
        }
    }

    #[test]
    fn run_list_subcommand_against_mock_server_succeeds() {
        let (path, _dir) = spawn_canned_server(|_req| {
            r#"{"result":[{"id":1,"name":"alpha","path":null,"range_start":4000,"range_end":4009,"created_at":"t","ports":[]}]}"#
                .into()
        });
        let cli = Cli {
            global: opts_with_socket(path),
            command: Command::List {
                here: false,
                project: None,
                active: false,
            },
        };
        let result = run(cli);
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[test]
    fn run_release_aborts_when_not_yes_and_not_tty() {
        // Tests + cargo run their stdin from a pipe, never a TTY. Without
        // --yes, the CLI must refuse to act destructively.
        let (path, _dir) = spawn_canned_server(|_req| r#"{"result":"ok"}"#.into());
        let cli = Cli {
            global: opts_with_socket(path),
            command: Command::Release {
                name: Some("alpha".into()),
                here: false,
                yes: false,
            },
        };
        let result = run(cli);
        match result {
            Err(CliError::AbortedByUser) => {}
            other => panic!("expected AbortedByUser, got {other:?}"),
        }
    }

    #[test]
    fn run_release_with_yes_proceeds() {
        let (path, _dir) = spawn_canned_server(|req| {
            assert!(req.contains("\"method\":\"release_project\""));
            r#"{"result":"ok"}"#.into()
        });
        let cli = Cli {
            global: opts_with_socket(path),
            command: Command::Release {
                name: Some("alpha".into()),
                here: false,
                yes: true,
            },
        };
        let result = run(cli);
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[test]
    fn run_with_unreachable_socket_returns_app_not_running() {
        let dir = tempfile::tempdir().unwrap();
        let dead = dir.path().join("dead.sock");
        let cli = Cli {
            global: opts_with_socket(dead),
            command: Command::List {
                here: false,
                project: None,
                active: false,
            },
        };
        let err = run(cli).unwrap_err();
        assert_eq!(err.exit_code(), 3);
    }
}
