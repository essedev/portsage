//! Cross-platform port scanner.
//!
//! The public API (`scan_active_ports`, `scan_active_ports_detailed`,
//! `scan_unmanaged_ports`) is OS-neutral. The implementation is selected at
//! build time via `#[cfg(target_os = "...")]`; there is no runtime dispatch.
//!
//! - macOS: shells out to `lsof -iTCP -sTCP:LISTEN -nP` (the historical
//!   approach), then resolves PIDs to executable names via `ps -p <pid> -o comm=`.
//! - Linux: parses `/proc/net/tcp` and `/proc/net/tcp6` for sockets in
//!   `TCP_LISTEN` state, then maps the socket inode back to a PID by walking
//!   `/proc/<pid>/fd/`. Falls back to `ss -ltnpH` when /proc reading fails
//!   (e.g. inside restricted containers / namespaces).

use std::collections::HashSet;

// Wire type shared with portsage-client. Keep the definition there.
pub use portsage_client::ActivePort;

/// Processes whose listening sockets are filtered out of the "unmanaged"
/// list. These are well-known kernel / system services that the user does
/// not care about and cannot easily kill anyway.
#[cfg(target_os = "macos")]
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

#[cfg(target_os = "linux")]
const BLOCKED_PROCESSES: &[&str] = &[
    "systemd-resolved",
    "systemd-resolve",
    "cups-browsed",
    "cupsd",
    "avahi-daemon",
    "rpcbind",
    "rpc.statd",
    "rpc.idmapd",
    "sshd",
    "chronyd",
    "ntpd",
];

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const BLOCKED_PROCESSES: &[&str] = &[];

/// Minimum port number treated as "could be a dev server". Anything below
/// this (privileged ports, system services) is hidden from the unmanaged
/// list even if it happens to be listening.
const MIN_DEV_PORT: i64 = 3000;

#[allow(dead_code)]
pub fn scan_active_ports() -> HashSet<i64> {
    scan_active_ports_detailed()
        .into_iter()
        .map(|p| p.port)
        .collect()
}

pub fn scan_active_ports_detailed() -> Vec<ActivePort> {
    let mut ports = scan_impl();
    // Deduplicate by port (keep first occurrence). Same port can show up
    // multiple times if a process binds both IPv4 and IPv6 (or has multiple
    // fds pointing at the same socket).
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
                && !BLOCKED_PROCESSES
                    .iter()
                    .any(|bp| p.process.eq_ignore_ascii_case(bp))
        })
        .collect()
}

// --- Per-OS implementations ---

#[cfg(target_os = "macos")]
fn scan_impl() -> Vec<ActivePort> {
    macos::scan_via_lsof()
}

#[cfg(target_os = "linux")]
fn scan_impl() -> Vec<ActivePort> {
    linux::scan_via_proc().unwrap_or_else(|_| linux::scan_via_ss())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn scan_impl() -> Vec<ActivePort> {
    Vec::new()
}

#[cfg(target_os = "macos")]
mod macos {
    use super::ActivePort;
    use crate::toolpath;

    /// `lsof` lives in `/usr/sbin`, which is in launchd's PATH but missing
    /// from plenty of interactive shells. Resolving it explicitly keeps a
    /// `pnpm tauri dev` run from silently reporting an empty machine.
    const LSOF_CANDIDATES: &[&str] = &["/usr/sbin/lsof", "/usr/bin/lsof", "/opt/homebrew/bin/lsof"];
    const PS_CANDIDATES: &[&str] = &["/bin/ps", "/usr/bin/ps"];

    pub fn scan_via_lsof() -> Vec<ActivePort> {
        let output = toolpath::command(toolpath::resolve_or_bare("lsof", LSOF_CANDIDATES))
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
        ports
    }

    /// Parse lsof fields without side effects. Returns (process_name, pid, port).
    pub(super) fn parse_lsof_fields(line: &str) -> Option<(String, i64, i64)> {
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
        let output = toolpath::command(toolpath::resolve_or_bare("ps", PS_CANDIDATES))
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output()
            .ok()?;
        let full = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if full.is_empty() {
            return None;
        }
        // ps returns full path like /usr/bin/node, take just the filename
        Some(full.rsplit('/').next().unwrap_or(&full).to_string())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::ActivePort;
    use crate::toolpath;
    use std::collections::HashMap;
    use std::fs;
    use std::io;

    /// `ss` is in `/usr/sbin` or `/sbin` on most distros, neither of which is
    /// guaranteed to be on a service unit's PATH.
    const SS_CANDIDATES: &[&str] = &["/usr/sbin/ss", "/sbin/ss", "/usr/bin/ss", "/bin/ss"];

    /// TCP_LISTEN state in `/proc/net/tcp` (kernel constant `TCP_LISTEN = 10`,
    /// always rendered as zero-padded hex).
    const TCP_LISTEN_HEX: &str = "0A";

    /// Primary scanner: parse /proc/net/tcp{,6} and walk /proc/<pid>/fd to map
    /// socket inodes back to processes. Pure file I/O, no external binaries.
    pub fn scan_via_proc() -> io::Result<Vec<ActivePort>> {
        let mut entries: Vec<(i64, u64)> = parse_proc_net_tcp("/proc/net/tcp")?;
        // tcp6 is optional: some minimal containers disable IPv6 entirely.
        if let Ok(v6) = parse_proc_net_tcp("/proc/net/tcp6") {
            entries.extend(v6);
        }
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        let inode_to_pid = scan_proc_fd_inodes();
        let mut result: Vec<ActivePort> = entries
            .into_iter()
            .map(|(port, inode)| {
                let pid = inode_to_pid.get(&inode).copied().unwrap_or(0);
                let process = if pid > 0 {
                    read_proc_comm(pid).unwrap_or_else(|| "?".to_string())
                } else {
                    "?".to_string()
                };
                ActivePort { port, process, pid }
            })
            .collect();
        result.sort_by_key(|p| p.port);
        Ok(result)
    }

    /// Fallback scanner: shell out to `ss -ltnpH`. Used when /proc parsing
    /// fails (containers without /proc/net, missing fd directories, etc.).
    /// Note that `ss -p` only shows PID/process info for sockets the caller
    /// has permission to inspect; without root this is limited to the
    /// caller's own processes - same constraint as scan_via_proc.
    pub fn scan_via_ss() -> Vec<ActivePort> {
        let output = match toolpath::command(toolpath::resolve_or_bare("ss", SS_CANDIDATES))
            .args(["-ltnpH"])
            .output()
        {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut out: Vec<ActivePort> = stdout.lines().filter_map(parse_ss_line).collect();
        out.sort_by_key(|p| p.port);
        out
    }

    /// Parse `/proc/net/tcp` or `/proc/net/tcp6`. Returns `(port, inode)` for
    /// every entry in TCP_LISTEN state. The address itself is ignored - any
    /// listening socket (loopback or wildcard) counts as a listening port.
    pub(super) fn parse_proc_net_tcp(path: &str) -> io::Result<Vec<(i64, u64)>> {
        let contents = fs::read_to_string(path)?;
        let mut out = Vec::new();
        for line in contents.lines().skip(1) {
            if let Some(entry) = parse_proc_net_tcp_line(line) {
                out.push(entry);
            }
        }
        Ok(out)
    }

    /// Parse a single /proc/net/tcp line. Returns Some((port, inode)) if the
    /// entry is a listening socket, None otherwise (including malformed
    /// lines, which are simply skipped rather than erroring).
    pub(super) fn parse_proc_net_tcp_line(line: &str) -> Option<(i64, u64)> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Format: sl local_addr rem_addr st tx:rx tr:tm retrnsmt uid timeout inode ...
        let local_addr = parts.get(1)?;
        let st = parts.get(3)?;
        if !st.eq_ignore_ascii_case(TCP_LISTEN_HEX) {
            return None;
        }
        let inode: u64 = parts.get(9)?.parse().ok()?;
        // local_addr is "<hex_ip>:<hex_port>". For our purposes we only need
        // the port; the IP can be IPv4 (8 hex chars) or IPv6 (32 hex chars).
        let port_hex = local_addr.rsplit(':').next()?;
        let port = i64::from_str_radix(port_hex, 16).ok()?;
        Some((port, inode))
    }

    /// Walk `/proc/<pid>/fd/*` and build a map from socket inode -> pid.
    /// Errors are swallowed silently per-entry: missing pids and permission
    /// denials are expected.
    fn scan_proc_fd_inodes() -> HashMap<u64, i64> {
        let mut map = HashMap::new();
        let proc_dir = match fs::read_dir("/proc") {
            Ok(d) => d,
            Err(_) => return map,
        };
        for entry in proc_dir.flatten() {
            let name = entry.file_name();
            let name_str = match name.to_str() {
                Some(s) => s,
                None => continue,
            };
            let pid: i64 = match name_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let fd_dir = entry.path().join("fd");
            let fds = match fs::read_dir(&fd_dir) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for fd in fds.flatten() {
                let target = match fs::read_link(fd.path()) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if let Some(inode) = parse_socket_link(&target.to_string_lossy()) {
                    map.entry(inode).or_insert(pid);
                }
            }
        }
        map
    }

    /// Parse `socket:[<inode>]` link targets, returning the inode. Other fd
    /// kinds (regular files, pipes, anon_inodes) return None.
    pub(super) fn parse_socket_link(link: &str) -> Option<u64> {
        let rest = link.strip_prefix("socket:[")?;
        let rest = rest.strip_suffix(']')?;
        rest.parse().ok()
    }

    fn read_proc_comm(pid: i64) -> Option<String> {
        let path = format!("/proc/{}/comm", pid);
        let s = fs::read_to_string(path).ok()?;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Parse one line of `ss -ltnpH` output, e.g.
    /// `LISTEN 0  511  127.0.0.1:3000  0.0.0.0:*  users:(("node",pid=12345,fd=23))`
    /// Returns None for malformed lines (which are skipped silently).
    pub(super) fn parse_ss_line(line: &str) -> Option<ActivePort> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Need at least: state, recv-q, send-q, local-addr:port, peer-addr:port
        let local = parts.get(3)?;
        let port_str = local.rsplit(':').next()?;
        // ss can wrap IPv6 addrs in brackets: [::1]:4060 - the rsplit handles
        // that correctly since the port comes after the last ':'.
        let port: i64 = port_str.parse().ok()?;

        // Find the users:((...)) field. With -p it always lives at the end.
        let users_field = parts.iter().find(|p| p.starts_with("users:"));
        let (process, pid) = users_field
            .and_then(|s| parse_ss_users_field(s))
            .unwrap_or_else(|| ("?".to_string(), 0));
        Some(ActivePort { port, process, pid })
    }

    /// `users:(("node",pid=12345,fd=23))` -> ("node", 12345). Multiple
    /// users (`,("python",pid=...)`) get the first one, matching what we'd
    /// pick from /proc walking.
    pub(super) fn parse_ss_users_field(field: &str) -> Option<(String, i64)> {
        let inner = field.strip_prefix("users:")?;
        let inner = inner.strip_prefix("((")?.trim_end_matches("))");
        // Now `"node",pid=12345,fd=23` or several joined by `),(`
        let first = inner.split("),(").next()?;
        let mut iter = first.splitn(3, ',');
        let name_q = iter.next()?;
        let pid_kv = iter.next()?;
        let name = name_q.trim_matches('"').to_string();
        let pid: i64 = pid_kv.strip_prefix("pid=")?.parse().ok()?;
        Some((name, pid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ap(port: i64, process: &str, pid: i64) -> ActivePort {
        ActivePort {
            port,
            process: process.into(),
            pid,
        }
    }

    #[test]
    fn blocked_processes_filters_system() {
        let registered: HashSet<i64> = HashSet::new();
        let active = vec![
            ap(5000, "node", 1),
            ap(
                5001,
                BLOCKED_PROCESSES.first().copied().unwrap_or("node"),
                2,
            ),
        ];
        let filtered: Vec<_> = active
            .into_iter()
            .filter(|p| {
                p.port >= MIN_DEV_PORT
                    && !registered.contains(&p.port)
                    && !BLOCKED_PROCESSES
                        .iter()
                        .any(|bp| p.process.eq_ignore_ascii_case(bp))
            })
            .collect();
        // node survives, the blocked process is filtered out (unless the OS
        // has no blocklist, in which case both survive - asserted below).
        if BLOCKED_PROCESSES.is_empty() {
            assert_eq!(filtered.len(), 2);
        } else {
            assert_eq!(filtered.len(), 1);
            assert_eq!(filtered[0].process, "node");
        }
    }

    #[test]
    fn registered_ports_excluded_from_unmanaged() {
        let mut registered = HashSet::new();
        registered.insert(5000);
        let active = vec![ap(5000, "node", 1), ap(5001, "node", 2)];
        let filtered: Vec<_> = active
            .into_iter()
            .filter(|p| {
                p.port >= MIN_DEV_PORT
                    && !registered.contains(&p.port)
                    && !BLOCKED_PROCESSES
                        .iter()
                        .any(|bp| p.process.eq_ignore_ascii_case(bp))
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].port, 5001);
    }

    #[test]
    fn ports_below_min_excluded() {
        let registered: HashSet<i64> = HashSet::new();
        let active = vec![ap(80, "nginx", 1), ap(443, "nginx", 2), ap(3000, "node", 3)];
        let filtered: Vec<_> = active
            .into_iter()
            .filter(|p| {
                p.port >= MIN_DEV_PORT
                    && !registered.contains(&p.port)
                    && !BLOCKED_PROCESSES
                        .iter()
                        .any(|bp| p.process.eq_ignore_ascii_case(bp))
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].port, 3000);
    }

    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::super::macos::parse_lsof_fields;

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
    }

    #[cfg(target_os = "linux")]
    mod linux_tests {
        use super::super::linux::{
            parse_proc_net_tcp_line, parse_socket_link, parse_ss_line, parse_ss_users_field,
        };

        // Captured /proc/net/tcp samples. Header is skipped by the caller, so
        // these are body lines only.
        const TCP_LOOPBACK_3000_LISTEN: &str = concat!(
            "   0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  ",
            "1000        0 1234567 1 0000000000000000 100 0 0 10 0",
        );
        const TCP_WILDCARD_5432_LISTEN: &str = concat!(
            "   1: 00000000:1538 00000000:0000 0A 00000000:00000000 00:00000000 00000000  ",
            "1000        0 9876543 1 0000000000000000 100 0 0 10 0",
        );
        const TCP_ESTABLISHED: &str = concat!(
            "   2: 0100007F:0BB8 0100007F:9C40 01 00000000:00000000 00:00000000 00000000  ",
            "1000        0 1112223 1 0000000000000000 100 0 0 10 0",
        );
        const TCP6_LOOPBACK_4060_LISTEN: &str = concat!(
            "   0: 00000000000000000000000000000000:0FDC 00000000000000000000000000000000:0000 ",
            "0A 00000000:00000000 00:00000000 00000000  1000        0 4040404 1 0000000000000000 100 0 0 10 0",
        );

        #[test]
        fn parse_listen_ipv4_loopback() {
            let (port, inode) = parse_proc_net_tcp_line(TCP_LOOPBACK_3000_LISTEN).unwrap();
            assert_eq!(port, 3000);
            assert_eq!(inode, 1234567);
        }

        #[test]
        fn parse_listen_ipv4_wildcard() {
            let (port, inode) = parse_proc_net_tcp_line(TCP_WILDCARD_5432_LISTEN).unwrap();
            assert_eq!(port, 5432);
            assert_eq!(inode, 9876543);
        }

        #[test]
        fn parse_skips_non_listen_sockets() {
            assert!(parse_proc_net_tcp_line(TCP_ESTABLISHED).is_none());
        }

        #[test]
        fn parse_listen_ipv6() {
            let (port, inode) = parse_proc_net_tcp_line(TCP6_LOOPBACK_4060_LISTEN).unwrap();
            assert_eq!(port, 4060);
            assert_eq!(inode, 4040404);
        }

        #[test]
        fn parse_malformed_returns_none() {
            assert!(parse_proc_net_tcp_line("").is_none());
            assert!(parse_proc_net_tcp_line("not a real line").is_none());
            assert!(parse_proc_net_tcp_line("   0: short").is_none());
        }

        #[test]
        fn socket_link_extracts_inode() {
            assert_eq!(parse_socket_link("socket:[12345]"), Some(12345));
            assert_eq!(parse_socket_link("socket:[0]"), Some(0));
        }

        #[test]
        fn socket_link_returns_none_for_non_sockets() {
            assert!(parse_socket_link("/dev/null").is_none());
            assert!(parse_socket_link("pipe:[12345]").is_none());
            assert!(parse_socket_link("anon_inode:[eventfd]").is_none());
        }

        #[test]
        fn ss_users_field_extracts_first_process() {
            let (name, pid) = parse_ss_users_field(r#"users:(("node",pid=12345,fd=23))"#).unwrap();
            assert_eq!(name, "node");
            assert_eq!(pid, 12345);
        }

        #[test]
        fn ss_users_field_first_of_many() {
            // `ss` can list multiple holders for a single socket (e.g. forked
            // workers). We take the first; the user can still kill the parent.
            let (name, pid) =
                parse_ss_users_field(r#"users:(("nginx",pid=10,fd=6),("nginx",pid=11,fd=6))"#)
                    .unwrap();
            assert_eq!(name, "nginx");
            assert_eq!(pid, 10);
        }

        #[test]
        fn ss_line_parses_full_row() {
            let line =
                "LISTEN 0  511  127.0.0.1:3000  0.0.0.0:*  users:((\"node\",pid=12345,fd=23))";
            let ap = parse_ss_line(line).unwrap();
            assert_eq!(ap.port, 3000);
            assert_eq!(ap.process, "node");
            assert_eq!(ap.pid, 12345);
        }

        #[test]
        fn ss_line_without_users_field_yields_unknown_process() {
            let line = "LISTEN 0  511  0.0.0.0:443  0.0.0.0:*";
            let ap = parse_ss_line(line).unwrap();
            assert_eq!(ap.port, 443);
            assert_eq!(ap.process, "?");
            assert_eq!(ap.pid, 0);
        }

        #[test]
        fn ss_line_handles_ipv6_brackets() {
            let line = "LISTEN 0  511  [::1]:8080  [::]:*  users:((\"node\",pid=99,fd=3))";
            let ap = parse_ss_line(line).unwrap();
            assert_eq!(ap.port, 8080);
            assert_eq!(ap.pid, 99);
        }
    }
}
