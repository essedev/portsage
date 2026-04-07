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

/// Parse lsof fields without side effects. Returns (process_name, pid, port).
fn parse_lsof_fields(line: &str) -> Option<(String, i64, i64)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let lsof_name = parts.first()?.to_string();
    let pid: i64 = parts.get(1)?.parse().ok()?;
    let name = parts.get(8)?;
    let port: i64 = name.rsplit(':').next()?.parse().ok()?;
    Some((lsof_name, pid, port))
}

fn parse_lsof_line(line: &str) -> Option<ActivePort> {
    let (lsof_name, pid, port) = parse_lsof_fields(line)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    // Real lsof output lines from macOS
    const LSOF_NODE: &str =
        "node      12345 user   23u  IPv4 0x1234  0t0  TCP 127.0.0.1:3000 (LISTEN)";
    const LSOF_POSTGRES: &str =
        "postgres  6789  user   5u   IPv4 0xabcd  0t0  TCP *:5432 (LISTEN)";
    const LSOF_IPV6: &str =
        "node      11111 user   24u  IPv6 0x5678  0t0  TCP [::1]:8080 (LISTEN)";

    #[test]
    fn parse_standard_lsof_line() {
        let (name, pid, port) = parse_lsof_fields(LSOF_NODE).unwrap();
        assert_eq!(name, "node");
        assert_eq!(pid, 12345);
        assert_eq!(port, 3000);
    }

    #[test]
    fn parse_wildcard_address() {
        let (name, pid, port) = parse_lsof_fields(LSOF_POSTGRES).unwrap();
        assert_eq!(name, "postgres");
        assert_eq!(pid, 6789);
        assert_eq!(port, 5432);
    }

    #[test]
    fn parse_ipv6_address() {
        let (name, pid, port) = parse_lsof_fields(LSOF_IPV6).unwrap();
        assert_eq!(name, "node");
        assert_eq!(pid, 11111);
        assert_eq!(port, 8080);
    }

    #[test]
    fn parse_empty_line_returns_none() {
        assert!(parse_lsof_fields("").is_none());
    }

    #[test]
    fn parse_header_line_returns_none() {
        let header = "COMMAND   PID  USER   FD   TYPE   DEVICE SIZE/OFF NODE NAME";
        assert!(parse_lsof_fields(header).is_none());
    }

    #[test]
    fn parse_truncated_line_returns_none() {
        assert!(parse_lsof_fields("node 123").is_none());
    }

    #[test]
    fn blocked_processes_filters_system() {
        let registered: HashSet<i64> = HashSet::new();
        let active = vec![
            ActivePort { port: 5000, process: "node".into(), pid: 1 },
            ActivePort { port: 5001, process: "rapportd".into(), pid: 2 },
            ActivePort { port: 5002, process: "mDNSResponder".into(), pid: 3 },
        ];
        let filtered: Vec<_> = active
            .into_iter()
            .filter(|p| {
                p.port >= MIN_DEV_PORT
                    && !registered.contains(&p.port)
                    && !BLOCKED_PROCESSES.iter().any(|bp| p.process.eq_ignore_ascii_case(bp))
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].process, "node");
    }

    #[test]
    fn registered_ports_excluded_from_unmanaged() {
        let mut registered = HashSet::new();
        registered.insert(5000);
        let active = vec![
            ActivePort { port: 5000, process: "node".into(), pid: 1 },
            ActivePort { port: 5001, process: "node".into(), pid: 2 },
        ];
        let filtered: Vec<_> = active
            .into_iter()
            .filter(|p| {
                p.port >= MIN_DEV_PORT
                    && !registered.contains(&p.port)
                    && !BLOCKED_PROCESSES.iter().any(|bp| p.process.eq_ignore_ascii_case(bp))
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].port, 5001);
    }

    #[test]
    fn ports_below_min_excluded() {
        let registered: HashSet<i64> = HashSet::new();
        let active = vec![
            ActivePort { port: 80, process: "nginx".into(), pid: 1 },
            ActivePort { port: 443, process: "nginx".into(), pid: 2 },
            ActivePort { port: 3000, process: "node".into(), pid: 3 },
        ];
        let filtered: Vec<_> = active
            .into_iter()
            .filter(|p| {
                p.port >= MIN_DEV_PORT
                    && !registered.contains(&p.port)
                    && !BLOCKED_PROCESSES.iter().any(|bp| p.process.eq_ignore_ascii_case(bp))
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].port, 3000);
    }
}
