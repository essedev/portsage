use serde::Serialize;
use std::collections::HashSet;
use std::process::Command;

#[derive(Debug, Serialize, Clone)]
pub struct ActivePort {
    pub port: i64,
    pub process: String,
    pub pid: i64,
}

const BLOCKED_PROCESSES: &[&str] = &[
    "rapportd",
    "sharingd",
    "mDNSResponder",
    "AirPlayXPCHelper",
    "ControlCenter",
    "WiFiAgent",
    "cupsd",
    "launchd",
    "SystemUIServer",
    "Spotlight",
    "bluetoothd",
    "configd",
    "identityservicesd",
    "locationd",
    "loginwindow",
    "remoted",
    "UserEventAgent",
    "symptomsd",
    "trustd",
    "AMPDevicesAgent",
    "AMPLibraryAgent",
    "coreautha",
    "findmydeviced",
];

const MIN_DEV_PORT: i64 = 3000;

pub fn scan_active_ports() -> HashSet<i64> {
    scan_active_ports_detailed()
        .into_iter()
        .map(|p| p.port)
        .collect()
}

pub fn scan_active_ports_detailed() -> Vec<ActivePort> {
    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-nP"])
        .output();

    let mut ports = Vec::new();
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            if let Some(ap) = parse_lsof_line(line) {
                ports.push(ap);
            }
        }
    }
    // Deduplicate by port (keep first occurrence)
    let mut seen = HashSet::new();
    ports.retain(|p| seen.insert(p.port));
    ports
}

pub fn scan_unmanaged_ports(registered: &HashSet<i64>) -> Vec<ActivePort> {
    scan_active_ports_detailed()
        .into_iter()
        .filter(|p| {
            p.port >= MIN_DEV_PORT
                && !registered.contains(&p.port)
                && !BLOCKED_PROCESSES.iter().any(|bp| p.process.eq_ignore_ascii_case(bp))
        })
        .collect()
}

fn parse_lsof_line(line: &str) -> Option<ActivePort> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let lsof_name = parts.first()?.to_string();
    let pid: i64 = parts.get(1)?.parse().ok()?;
    let name = parts.get(8)?;
    let port: i64 = name.rsplit(':').next()?.parse().ok()?;

    // Resolve full process name via ps if lsof truncated it
    let process = resolve_process_name(pid).unwrap_or(lsof_name);

    Some(ActivePort { port, process, pid })
}

fn resolve_process_name(pid: i64) -> Option<String> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    let full = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if full.is_empty() {
        return None;
    }
    // ps returns full path like /usr/bin/node, take just the filename
    Some(
        full.rsplit('/')
            .next()
            .unwrap_or(&full)
            .to_string(),
    )
}
