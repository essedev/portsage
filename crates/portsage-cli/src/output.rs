use anstyle::{AnsiColor, Color, Style};
use portsage_client::{
    ActivePort, KillEntry, KillOutcome, PortStatus, ProjectStatus, RestoreOutcome, StaleProject,
    StaleReason, TrashEntry, TrashKind,
};
use std::io::{self, Write};

/// Output mode driven by the global `--json` / `--quiet` flags. The default
/// human view is colored when stdout is a TTY; `AutoStream` strips colors when
/// it isn't (e.g. piped to a file).
#[derive(Debug, Clone, Copy)]
pub enum OutputMode {
    Human,
    Json,
    Quiet,
}

impl OutputMode {
    pub fn from_flags(json: bool, quiet: bool) -> Self {
        if json {
            OutputMode::Json
        } else if quiet {
            OutputMode::Quiet
        } else {
            OutputMode::Human
        }
    }
}

fn green() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)))
}
fn red() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)))
}
fn yellow() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)))
}
fn dim() -> Style {
    Style::new().dimmed()
}
fn bold() -> Style {
    Style::new().bold()
}

pub fn print_projects(mode: OutputMode, projects: &[ProjectStatus]) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(projects),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            for p in projects {
                writeln!(out, "{}\t{}\t{}", p.name, p.range_start, p.range_end)?;
            }
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            if projects.is_empty() {
                writeln!(out, "(no projects registered)")?;
                return Ok(());
            }
            // Width-padded columns. Computing widths upfront keeps the output aligned
            // even when names are long.
            let name_w = projects
                .iter()
                .map(|p| p.name.len())
                .max()
                .unwrap_or(4)
                .max(4);
            let path_w = projects
                .iter()
                .map(|p| p.path.as_deref().unwrap_or("-").len())
                .max()
                .unwrap_or(4)
                .max(4);
            let b = bold();
            writeln!(
                out,
                "{b}{:<4} {:<name_w$}  {:<11}  {:<path_w$}  {:<6}{b:#}",
                "ID",
                "NAME",
                "RANGE",
                "PATH",
                "PORTS",
                name_w = name_w,
                path_w = path_w,
            )?;
            let d = dim();
            for p in projects {
                let active = p.ports.iter().filter(|x| x.active).count();
                let total = p.ports.len();
                let count = format!("{}/{}", active, total);
                // Archived rows only show up when explicitly asked for, so
                // they have to be recognisable once they do.
                let shelved = if p.archived_at.is_some() {
                    format!("  {d}archived{d:#}")
                } else {
                    String::new()
                };
                writeln!(
                    out,
                    "{:<4} {:<name_w$}  {:>5}-{:<5}  {:<path_w$}  {}{}",
                    p.id,
                    p.name,
                    p.range_start,
                    p.range_end,
                    p.path.as_deref().unwrap_or("-"),
                    count,
                    shelved,
                    name_w = name_w,
                    path_w = path_w,
                )?;
            }
            Ok(())
        }
    }
}

pub fn print_project_detail(mode: OutputMode, p: &ProjectStatus) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(p),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            for port in &p.ports {
                writeln!(out, "{}\t{}\t{}", port.service, port.port, port.active)?;
            }
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            let b = bold();
            let d = dim();
            writeln!(out, "{b}{}{b:#}  {d}#{}{d:#}", p.name, p.id)?;
            writeln!(
                out,
                "  {d}path:{d:#}   {}",
                p.path.as_deref().unwrap_or("-")
            )?;
            writeln!(out, "  {d}range:{d:#}  {}-{}", p.range_start, p.range_end)?;
            if p.ports.is_empty() {
                writeln!(out, "  {d}(no ports registered yet){d:#}")?;
            } else {
                writeln!(out, "  {d}ports:{d:#}")?;
                let svc_w = p.ports.iter().map(|x| x.service.len()).max().unwrap_or(4);
                for port in &p.ports {
                    let badge = if port.active {
                        format!("{}ACTIVE{:#}", green(), green())
                    } else {
                        format!("{}     -{:#}", dim(), dim())
                    };
                    let proc = match (&port.process, port.pid) {
                        (Some(name), Some(pid)) => format!(" {d}({} pid {}){d:#}", name, pid),
                        _ => String::new(),
                    };
                    writeln!(
                        out,
                        "    {:<svc_w$}  {:>5}  {}{}",
                        port.service,
                        port.port,
                        badge,
                        proc,
                        svc_w = svc_w,
                    )?;
                }
            }
            Ok(())
        }
    }
}

pub fn print_active_ports(mode: OutputMode, ports: &[ActivePort]) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(ports),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            for p in ports {
                writeln!(out, "{}\t{}\t{}", p.port, p.pid, p.process)?;
            }
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            if ports.is_empty() {
                writeln!(out, "(nothing listening)")?;
                return Ok(());
            }
            let b = bold();
            writeln!(out, "{b}{:<7} {:<8} PROCESS{b:#}", "PORT", "PID")?;
            for p in ports {
                writeln!(out, "{:<7} {:<8} {}", p.port, p.pid, p.process)?;
            }
            Ok(())
        }
    }
}

pub fn print_kill_outcome(mode: OutputMode, port: i64, outcome: KillOutcome) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(serde_json::json!({"port": port, "outcome": outcome})),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            writeln!(out, "{}\t{}", port, outcome_label(outcome))?;
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            let (style, msg) = outcome_human(outcome);
            writeln!(out, "port {}: {style}{}{style:#}", port, msg)?;
            Ok(())
        }
    }
}

pub fn print_kill_entries(mode: OutputMode, entries: &[KillEntry]) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(entries),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            for e in entries {
                writeln!(out, "{}\t{}", e.port, outcome_label(e.outcome))?;
            }
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            if entries.is_empty() {
                writeln!(out, "(nothing was active to kill)")?;
                return Ok(());
            }
            for e in entries {
                let (style, msg) = outcome_human(e.outcome);
                writeln!(out, "  {:>5}  {style}{}{style:#}", e.port, msg)?;
            }
            Ok(())
        }
    }
}

pub fn print_port(mode: OutputMode, p: &PortStatus) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(p),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            writeln!(out, "{}\t{}", p.service, p.port)?;
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            let g = green();
            writeln!(
                out,
                "{g}registered{g:#} {} = {} ({} #{})",
                p.service, p.port, p.service, p.id
            )?;
            Ok(())
        }
    }
}

pub fn print_message(mode: OutputMode, message: &str) -> io::Result<()> {
    match mode {
        OutputMode::Quiet => Ok(()),
        OutputMode::Json => print_json(serde_json::json!({ "ok": message })),
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            writeln!(out, "{}", message)?;
            Ok(())
        }
    }
}

pub fn print_error(message: &str) -> io::Result<()> {
    // Errors always go to stderr so they don't pollute piped stdout.
    let mut err = anstream::stderr().lock();
    let r = red();
    writeln!(err, "{r}error:{r:#} {}", message)?;
    Ok(())
}

pub fn print_json<T: serde::Serialize>(value: T) -> io::Result<()> {
    let mut out = anstream::stdout().lock();
    let s = serde_json::to_string_pretty(&value)
        .unwrap_or_else(|e| format!(r#"{{"error":"serialize failed: {e}"}}"#));
    writeln!(out, "{}", s)?;
    Ok(())
}

/// `dry_run` gates the closing hint: after `--apply` the "nothing changed"
/// line would be a lie.
pub fn print_stale(
    mode: OutputMode,
    entries: &[StaleProject],
    days: i64,
    dry_run: bool,
) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(entries),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            for e in entries {
                writeln!(
                    out,
                    "{}\t{}\t{}",
                    e.name,
                    stale_reason_label(e.reason),
                    e.inactive_days.map(|d| d.to_string()).unwrap_or_default()
                )?;
            }
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            let d = dim();
            if entries.is_empty() {
                writeln!(
                    out,
                    "(nothing idle for {days}+ days; projects with a listening port are never listed)"
                )?;
                return Ok(());
            }
            let name_w = entries
                .iter()
                .map(|e| e.name.len())
                .max()
                .unwrap_or(4)
                .max(4);
            let b = bold();
            writeln!(
                out,
                "{b}{:<name_w$}  {:<11}  {:<6}  WHY{b:#}",
                "NAME",
                "RANGE",
                "PORTS",
                name_w = name_w,
            )?;
            let mut missing = 0;
            for e in entries {
                let why = match e.reason {
                    StaleReason::PathMissing => {
                        missing += 1;
                        format!("path is gone: {}", e.path.as_deref().unwrap_or("-"))
                    }
                    StaleReason::Inactive => {
                        format!("untouched for {} days", e.inactive_days.unwrap_or(0))
                    }
                };
                writeln!(
                    out,
                    "{:<name_w$}  {:>5}-{:<5}  {:<6}  {}",
                    e.name,
                    e.range_start,
                    e.range_end,
                    e.registered_ports,
                    why,
                    name_w = name_w,
                )?;
            }
            if missing > 0 {
                // A gone directory is often a move, not a deletion, and
                // archiving would hide a project that is merely mislabelled.
                writeln!(
                    out,
                    "{d}if one of those {missing} moved rather than went away, fix it with \
                     `portsage rename <name> --path <new-path>`{d:#}"
                )?;
            }
            if dry_run {
                writeln!(
                    out,
                    "{d}nothing changed. `portsage prune --apply` archives these; they keep \
                     their range and come back with `portsage unarchive <name>`{d:#}"
                )?;
            }
            Ok(())
        }
    }
}

fn stale_reason_label(reason: StaleReason) -> &'static str {
    match reason {
        StaleReason::PathMissing => "path_missing",
        StaleReason::Inactive => "inactive",
    }
}

pub fn print_trash(mode: OutputMode, entries: &[TrashEntry]) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(entries),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            for e in entries {
                writeln!(out, "{}\t{}\t{}", e.id, trash_kind_label(e.kind), e.label)?;
            }
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            if entries.is_empty() {
                writeln!(out, "(trash is empty)")?;
                return Ok(());
            }
            let label_w = entries
                .iter()
                .map(|e| e.label.len())
                .max()
                .unwrap_or(4)
                .max(4);
            let b = bold();
            writeln!(
                out,
                "{b}{:<4} {:<7} {:<label_w$}  {:<24}  DELETED{b:#}",
                "ID",
                "KIND",
                "NAME",
                "DETAIL",
                label_w = label_w,
            )?;
            for e in entries {
                writeln!(
                    out,
                    "{:<4} {:<7} {:<label_w$}  {:<24}  {}",
                    e.id,
                    trash_kind_label(e.kind),
                    e.label,
                    e.detail,
                    e.deleted_at,
                    label_w = label_w,
                )?;
            }
            let d = dim();
            writeln!(out, "{d}restore with `portsage trash restore <id>`{d:#}")?;
            Ok(())
        }
    }
}

pub fn print_restore_outcome(mode: OutputMode, outcome: &RestoreOutcome) -> io::Result<()> {
    match mode {
        OutputMode::Json => print_json(outcome),
        OutputMode::Quiet => {
            let mut out = anstream::stdout().lock();
            for port in &outcome.restored_ports {
                writeln!(out, "{}\t{}", outcome.project, port)?;
            }
            Ok(())
        }
        OutputMode::Human => {
            let mut out = anstream::stdout().lock();
            let g = green();
            writeln!(
                out,
                "{g}restored{g:#} {} ({} port{})",
                outcome.project,
                outcome.restored_ports.len(),
                if outcome.restored_ports.len() == 1 {
                    ""
                } else {
                    "s"
                },
            )?;
            // Skipped ports are the one thing a caller must not miss: the
            // project is back but incomplete.
            if !outcome.skipped_ports.is_empty() {
                let y = yellow();
                let ports: Vec<String> = outcome
                    .skipped_ports
                    .iter()
                    .map(|p| p.to_string())
                    .collect();
                writeln!(
                    out,
                    "{y}skipped{y:#} {} (registered to another project since)",
                    ports.join(", ")
                )?;
            }
            Ok(())
        }
    }
}

fn trash_kind_label(kind: TrashKind) -> &'static str {
    match kind {
        TrashKind::Project => "project",
        TrashKind::Port => "port",
    }
}

fn outcome_label(outcome: KillOutcome) -> &'static str {
    match outcome {
        KillOutcome::Terminated => "terminated",
        KillOutcome::Killed => "killed",
        KillOutcome::NotActive => "not_active",
        KillOutcome::PermissionDenied => "permission_denied",
        KillOutcome::DockerStopped => "docker_stopped",
        KillOutcome::DockerCliMissing => "docker_cli_missing",
        KillOutcome::DockerDaemonDown => "docker_daemon_down",
        KillOutcome::DockerNoContainer => "docker_no_container",
        KillOutcome::DockerError => "docker_error",
    }
}

fn outcome_human(outcome: KillOutcome) -> (Style, &'static str) {
    match outcome {
        KillOutcome::Terminated => (green(), "terminated"),
        KillOutcome::Killed => (yellow(), "killed (forced after grace)"),
        KillOutcome::NotActive => (dim(), "nothing was listening"),
        KillOutcome::PermissionDenied => (red(), "permission denied"),
        KillOutcome::DockerStopped => (green(), "container stopped (docker)"),
        KillOutcome::DockerCliMissing => (
            red(),
            "docker CLI not found (set PORTSAGE_DOCKER_BIN to its path)",
        ),
        KillOutcome::DockerDaemonDown => {
            (red(), "docker daemon not reachable (is Docker running?)")
        }
        KillOutcome::DockerNoContainer => (red(), "no running container publishes this port"),
        KillOutcome::DockerError => (red(), "docker stop failed"),
    }
}
