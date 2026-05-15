# Multi-host evolution plan

Detailed plan to evolve Portsage from a single-host macOS app into a multi-host port allocation manager. The goal: the same menubar UI that today shows the local Mac state can also show, and control, the state of one or more remote dev servers, including automatic SSH port forwarding so the user's browser keeps reaching `localhost:<port>`.

This document is the source of truth for the evolution roadmap. It supersedes the vague entries in `ROADMAP.md` (v0.7 "cross-platform") and refines them into shippable phases.

> **Status (2026-05-15)**: Phase 1, Phase 2, and Phase 3 are shipped and validated end-to-end against the `forge` dev box on 2026-05-12. Phase 4 (polish) is not yet started. This document is kept for the design rationale and the residual Phase 4 plan; the shipped scope and known deferrals are mirrored in [`ROADMAP.md`](ROADMAP.md) (v0.8 / v0.11) and the "Active evolution" section of [`CLAUDE.md`](../CLAUDE.md).

## Context

Portsage today assumes a single host: the user's Mac. With AI agents (Claude Code) running on a remote dev server (Hetzner dedicated, Linux), the assumption breaks in three places:

1. **The agents need a port registry on the host where they run.** Agents on the remote box must call portsage MCP on the remote box to reserve and register ports. A registry on the Mac alone is useless to them.
2. **The user wants a single UI.** Switching between "local portsage on Mac" and "ssh + portsage CLI on server" is friction. The menubar should show both contexts.
3. **The browser on the Mac must still reach the remote services on `localhost:<port>` to keep CORS-free.** Port forwarding needs to be aware of which ports are in use on the remote host, and follow them automatically.

The chosen architecture is layered, not monolithic. Each phase ships value independently:

- **Phase 1** unblocks the remote agents (Linux server build).
- **Phase 2** unifies the UI (remote backend visible in the menubar).
- **Phase 3** eliminates manual SSH tunnel management (auto-forward).
- **Phase 4** polishes the edges.

## Target architecture

```
Mac (Tauri UI)
├─ Local backend         ──► /Users/.../portsage.sock  → SQLite (local)
└─ Remote backend "dev"  ──► /tmp/portsage-dev.sock    ↓
                                  ↑ SSH unix socket forward via ControlMaster
                                  │  (auto-managed by Portsage Mac)
                                  │
                           SSH tunnel inside Tailscale (WireGuard)
                                  │
                                  ▼
Hetzner Linux server
├─ portsage-server (headless Rust binary, systemd service)
│  └─ /run/portsage/portsage.sock → SQLite (remote, independent DB)
├─ portsage CLI (talks to the local socket)
└─ MCP server (Python, talks to the local socket)
   └─ used by Claude Code agents running on the server
```

Key principle: **each host runs its own backend with its own database**. There is no DB synchronization, no distributed state, no consensus protocol. The Mac UI is a thin client that can speak the existing socket protocol against either the local socket or a remote socket forwarded by SSH. Both backends are independent.

This is the simplest model that solves the problem. It avoids the complexity of clustering or replicated SQLite (Litestream, rqlite, etc.), which would be overkill for "show me what's running on box N".

## Phase 1 - Cross-platform server (Linux backend)

**Goal**: ship a `portsage-server` binary that runs on Linux x86_64, exposes the same Unix socket protocol as the macOS app's headless mode, and stores its state in a Linux-friendly location.

### What changes

#### 1.1 Scanner abstraction

Today `scanner.rs` shells out to `lsof -iTCP -sTCP:LISTEN -nP` and `ps -p <pid> -o comm=`. Neither is guaranteed on Linux (`lsof` is optional on minimal Ubuntu, `ps -o comm=` works but the lsof output format is different).

Introduce a trait in `src-tauri/src/scanner.rs`:

```rust
pub trait PortScanner: Send + Sync {
    fn scan(&self) -> Result<Vec<ActivePort>>;
}
```

Two implementations:

- `MacOsScanner` - current code, kept as-is.
- `LinuxScanner` - reads `/proc/net/tcp` and `/proc/net/tcp6` for LISTEN sockets, maps inodes to PIDs via `/proc/<pid>/fd/*`, reads process name from `/proc/<pid>/comm`. Pure file I/O, no external binaries. Fallback to `ss -ltnpH` if `/proc` parsing fails (containers, special namespaces).

Build-time selection via `#[cfg(target_os = "macos")]` / `#[cfg(target_os = "linux")]`. No runtime dispatch needed.

Tests: parse a captured sample of `/proc/net/tcp` from a Linux box, verify the parser handles IPv4, IPv6, multiple sockets per process, defunct entries (`tcp` line with `st != 0a`).

#### 1.2 Config directories

Today the macOS app uses `~/Library/Application Support/portsage/` for the DB and socket. On Linux follow XDG Base Directory:

- DB: `$XDG_DATA_HOME/portsage/portsage.db` (default `~/.local/share/portsage/`)
- Socket: `$XDG_RUNTIME_DIR/portsage/portsage.sock` (default `/run/user/<uid>/portsage/`) - automatically cleaned on logout. Fall back to `/tmp/portsage-<uid>.sock` if `XDG_RUNTIME_DIR` is unset (servers in non-systemd-user setups).
- Logs: `$XDG_STATE_HOME/portsage/server.log`

Centralize path resolution in `paths.rs` so the rest of the code is OS-agnostic.

For our dev server use case the socket should also be available system-wide for the systemd service: support an explicit `--socket /run/portsage/portsage.sock` argument that overrides the XDG default. The systemd unit will use this.

#### 1.3 Process blocklist for unmanaged ports

The current `scanner.rs` filters macOS system processes (AirPlay, CUPS, mDNS, Spotlight, etc.) from the "unmanaged ports" list. The Linux blocklist is different: avoid showing `systemd-resolved` (53), `cups-browsed` (631), `avahi-daemon` (5353), `docker-proxy` if you want (debatable - probably keep these visible, they are user-owned services).

Refactor the blocklist into a per-OS list. Keep it short and conservative: only block well-known kernel/system services that nobody wants to see.

#### 1.4 Linux build pipeline

Add to the existing release workflow (or a new `.github/workflows/server-build.yml`):

- Trigger: tag `v*` and manual dispatch.
- Job 1: build `portsage-server` (alias for the headless binary) on `ubuntu-latest`, statically linked via `musl` for portability. Target `x86_64-unknown-linux-musl`.
- Job 2: build `portsage-cli` on the same target (already cross-platform, just needs the target added).
- Job 3 (optional, low priority): `aarch64-unknown-linux-musl` for ARM servers (Raspberry Pi, Hetzner CAX cloud).
- Artifacts: tarball `portsage-server-vX.Y.Z-linux-x86_64.tar.gz` containing the binary + CLI + a sample systemd unit.

Distribution channels:

- GitHub Release attachments (immediate, no extra infra).
- Future: deb/rpm packages, an APT repo. Out of scope for Phase 1.
- Future: Homebrew on Linux (`brew install essedev/portsage/portsage-server`). Low priority.

#### 1.5 Server packaging

Provide a sample systemd unit in `packaging/linux/portsage-server.service`:

```ini
[Unit]
Description=Portsage port allocation server
After=network.target

[Service]
Type=simple
User=portsage
Group=portsage
ExecStart=/usr/local/bin/portsage-server --socket /run/portsage/portsage.sock
Restart=on-failure
RestartSec=2
RuntimeDirectory=portsage
RuntimeDirectoryMode=0750
StateDirectory=portsage
StateDirectoryMode=0750

[Install]
WantedBy=multi-user.target
```

Plus an install script `packaging/linux/install.sh` that:

1. Creates the `portsage` system user/group.
2. Copies the binary to `/usr/local/bin/`.
3. Drops the systemd unit into `/etc/systemd/system/`.
4. Adds the target user (e.g. `simone`) to the `portsage` group so they can talk to the socket without sudo.
5. `systemctl enable --now portsage-server`.

Idempotent. Re-run on upgrade.

> **As shipped (divergence discovered during live smoke test, 2026-05-12)**: the `User=portsage Group=portsage` configuration above blocks process-name resolution in the Mac UI. Background: the scanner maps listening ports to processes by reading `/proc/<pid>/fd/*` magic links. The kernel gates that readlink behind `__ptrace_may_access(PTRACE_MODE_FSCREDS)`, which requires the caller's `fsuid` AND `fsgid` to match the target process's creds (not just uid). With `Group=portsage` the service has `fsgid=987`, while the dev user's processes (Vite/Node/Python) have `gid=1000` - mismatch, kernel returns `EACCES`, every port renders as `?` in the UI. `install.sh` therefore `sed`-rewrites `User=` and `Group=` to the target dev user before installing the unit; the template above is left as-is so a multi-tenant operator can manually opt back in (and accept the `?` for cross-user ports). See `packaging/linux/README.md` for the user-facing explanation.

#### 1.6 Bundled CLI parity

The CLI is already cross-platform. Verify: nothing in `crates/portsage-cli` calls macOS-only syscalls or paths. Add a CI job that runs `cargo test -p portsage-cli` on `ubuntu-latest` to catch regressions.

#### 1.7 MCP server on Linux

The Python MCP server (`mcp/server.py`) is already platform-neutral - it speaks the wire protocol over a Unix socket. Just verify:

- `install.sh` works on Linux (it currently has macOS-only paths for the Claude Code config). Add a Linux branch that writes the MCP config to `~/.claude/settings.json` and the skill to `~/.claude/skills/portsage/`.
- Document the Linux install path.

### Acceptance criteria

- [ ] `cargo build --release --target x86_64-unknown-linux-musl -p portsage` produces a working binary.
- [ ] CI runs the full Rust test suite on Linux and macOS in parallel.
- [ ] `portsage-server --socket /tmp/foo.sock` on Linux exposes the socket and accepts all 14 protocol methods.
- [ ] `portsage list` from the CLI on the same Linux box returns the registered projects.
- [ ] An MCP client (e.g. Claude Code) configured against the Linux MCP server can reserve and register ports.
- [ ] systemd unit starts on boot, restarts on crash, cleans up the socket on shutdown.

### Effort estimate

3-5 days of focused work. Scanner abstraction is the heaviest piece (~2 days including tests on real Linux samples). Everything else is wiring, paths, and CI.

### Out of scope for Phase 1

- Windows support (covered by ROADMAP v0.7 as a separate future epic).
- ARM Linux (optional add-on, no blocker).
- Linux GUI / tray icon. Linux gets only the headless server. The Mac remains the only UI host.

---

## Phase 2 - Remote backend in the UI

**Goal**: the macOS app can connect to a remote `portsage-server` over SSH and show its state in the menubar / main window. The user can switch between "Local" and "Remote: <name>" backends. All existing UI works against the remote backend with no functional regression.

### What changes

#### 2.1 Remote backend configuration

Add a new SQLite table on the **Mac side** for known remote backends:

```sql
CREATE TABLE remote_backends (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,                -- "dev", "staging", etc.
    ssh_alias TEXT NOT NULL,                  -- matches Host entry in ~/.ssh/config
    remote_socket_path TEXT NOT NULL,         -- typically /run/portsage/portsage.sock
    local_socket_path TEXT NOT NULL,          -- where SSH forwards to, e.g. /tmp/portsage-dev.sock
    auto_forward_enabled INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Migration: add the table, no data backfill needed. The user configures backends explicitly.

#### 2.2 Client abstraction

Today `portsage-client::Client::connect()` takes an optional path and opens a `UnixStream`. Keep the API but make the resolution of the path go through a "backend selector":

```rust
pub enum BackendTarget {
    Local,
    Remote { name: String },
}

pub struct BackendManager {
    db: Arc<Database>,         // for reading remote_backends rows
    ssh_tunnels: Mutex<HashMap<String, SshTunnel>>,  // active tunnels by backend name
}

impl BackendManager {
    pub fn connect(&self, target: BackendTarget) -> Result<Client> {
        match target {
            BackendTarget::Local => Client::connect_local(),
            BackendTarget::Remote { name } => {
                let backend = self.db.get_remote_backend(&name)?;
                self.ensure_tunnel(&backend)?;
                Client::connect_path(&backend.local_socket_path)
            }
        }
    }
}
```

`ensure_tunnel` checks if the SSH tunnel is alive (test connection to local socket); if not, opens one via `ssh -fN -L unix:<local_socket>:<remote_socket> <ssh_alias>`. Relies on the user's existing `~/.ssh/config` for auth and ControlMaster settings.

#### 2.3 SSH tunnel management

The Mac app shells out to `ssh` rather than embedding `libssh2`/`russh`. Reasons:

- The user already has `~/.ssh/config` configured with `ControlMaster auto`, `ControlPath`, `ControlPersist`. Portsage should not bypass it.
- All authentication (keys, agent, hardware tokens) is delegated to the system SSH client.
- Tailscale routing, MagicDNS, and ProxyJump are all handled by the SSH client transparently.

`SshTunnel` struct:

```rust
pub struct SshTunnel {
    backend_name: String,
    ssh_alias: String,
    remote_socket: PathBuf,
    local_socket: PathBuf,
    pid: Option<u32>,
    state: TunnelState,  // Disconnected | Connecting | Connected | Failed(String)
}
```

Lifecycle:

- **Open**: spawn `ssh -fN -L unix:<local>:<remote> <alias>`. Wait until `<local>` becomes connectable (poll up to 5s). On failure, capture stderr for the UI.
- **Probe**: `connect(local_socket)` periodically (every 30s) to detect dead tunnels. If dead, mark as `Disconnected` and attempt reconnect.
- **Close**: send `ssh -O exit <alias>` for clean shutdown, or kill the spawned PID.

Emit Tauri events (`tunnel://state-changed`) so the UI updates the indicator without polling.

#### 2.4 UI changes

**Sidebar header**: add a backend switcher. A dropdown above the search box showing:

- "Local" (always present, default)
- "Remote: dev" (and any other configured remote backends)
- "+ Add remote backend..." (opens the config modal)

Below the dropdown, a small connection status dot: green (connected), amber (connecting), red (failed - hover for error).

**Settings panel**: new section "Remote backends" with:

- Table of configured backends: name, ssh_alias, remote socket path, connection status, last seen.
- "Add" button: form with the 4 fields (name, ssh_alias, remote_socket_path, local_socket_path). The local_socket_path has a sensible default `/tmp/portsage-<name>.sock` that the user rarely changes.
- "Test" button per row: opens the tunnel, sends a `list_all` request, reports success/failure.
- "Remove" button per row.
- A toggle "Auto-forward ports for this backend" per row, off by default (covered fully in Phase 3).

**Project list**: when a remote backend is selected, the list shows the **remote** projects. The "Add project" button creates the project on the remote backend. The "Open in Finder" button is hidden for remote projects (no equivalent action makes sense). The "Open in Terminal" button is replaced by "SSH into project dir" which opens a terminal running `ssh <alias> -t 'cd <path> && exec $SHELL'`.

**Project detail**: identical to local. Port status, kill buttons, add service, all route through the remote backend client. The "Open in browser" link still opens `http://localhost:<port>` - which works if the port is forwarded (Phase 3) or returns "connection refused" cleanly otherwise. Phase 3 closes this gap.

**Popover (menubar)**: stays simple. Default to showing the local backend. A small label `[dev]` next to the title indicates if a remote backend is selected. Click the label to cycle through backends, persisted across restarts.

#### 2.5 MCP and CLI scope

- **MCP server stays single-backend.** Each `portsage-server` instance has its own MCP server that talks to its own local socket. The agents on the Mac use the Mac MCP; the agents on the server use the server MCP. There is no "cross-backend" MCP call. This is deliberate: agents reason about the host they live on, not about remote machines.
- **CLI grows a `--backend <name>` flag**. `portsage list --backend dev` from the Mac shell shows the remote state. Default: local backend.

> **As shipped (divergence from the original plan)**: the CLI does *not* open its own tunnel. It asks the Mac app's socket for the configured backend's `local_socket_path` and connects to that. Reason: cross-process tunnel ownership would mean two `BackendManager` instances racing to bind the same local socket, plus per-process tracking of an SSH child. With the Mac app as the single tunnel owner the model stays simple - the cost is that `--backend dev` requires the Portsage app to be running and the tunnel for `dev` to be open (which the user does once from the Settings UI). A standalone CLI tunnel mode can be added later if demand appears.

#### 2.6 Error handling and UX edge cases

- **SSH alias not in ~/.ssh/config**: detect early (`ssh -G <alias>` returns config; if it returns the literal alias as hostname, it's not configured). Show a clear error: "Host 'dev' not found in your SSH config. Add it with Host/HostName entries first."
- **ControlMaster socket stale**: not our problem strictly, but if `ssh -O check` reports stale, surface a button "Refresh SSH connection" that calls `ssh -O exit` then reconnects.
- **Tailscale not running on the Mac**: the SSH call will fail with the standard `Could not resolve hostname` error. Surface the verbatim error in the UI - we don't try to detect Tailscale status; that's the user's responsibility.
- **Remote socket missing**: `portsage-server` is down on the remote box. SSH connects but `connect(/run/portsage/...)` fails. Show: "Connected to <alias>, but portsage-server is not running on the remote host."
- **Race on first connect**: two tabs trying to open a tunnel simultaneously. Lock the tunnel map per backend name.

### Acceptance criteria

- [ ] Adding a remote backend takes < 30 seconds (4 fields + Test).
- [ ] Switching between local and remote in the sidebar is instant after the tunnel is open (< 200ms).
- [ ] The full project CRUD works against a remote backend with no UI regression.
- [ ] Tunnel failure surfaces a clear actionable error.
- [ ] Restarting the Mac app preserves the configured backends and reconnects on demand.

### Effort estimate

4-6 days. Most of it is UI work and tunnel lifecycle. Client abstraction is mechanical once the BackendManager skeleton is in place.

---

## Phase 3 - SSH port forwarding integration

**Goal**: when a project on a remote backend has registered ports, Portsage automatically opens SSH local forwards on the Mac so the user's browser hits `localhost:<port>` and reaches the remote service transparently. No `~/.ssh/config` editing, no script generation.

### What changes

#### 3.1 ForwardManager

A new component on the Mac side. Tracks all active SSH local forwards per remote backend.

```rust
pub struct ForwardManager {
    backends: Arc<BackendManager>,
    forwards: Mutex<HashMap<(String, u16), ForwardState>>,
    config: Mutex<HashMap<String, BackendForwardConfig>>,
}

pub enum ForwardState {
    Pending,
    Active { since: Instant },
    Failed { reason: String },
    Cancelled,
}

pub struct BackendForwardConfig {
    auto_forward: bool,
    excluded_ports: HashSet<u16>,    // user-blocklisted ports
}
```

Operations:

- `enable_forward(backend, port)`: `ssh -O forward -L <port>:localhost:<port> <alias>` via the ControlMaster socket. Records state.
- `disable_forward(backend, port)`: `ssh -O cancel -L <port>:localhost:<port> <alias>`. Records state.
- `sync(backend)`: reconcile. Read the registered ports of the backend, ensure forwards match the desired set. Open missing, close extra.

The sync runs:

- On backend connection.
- On every project change event from the backend (after register_port, remove_port, release_project).
- On a periodic timer (60s) as safety net.

The backend dispatcher (`socket.rs`) gains an "events" channel: when state changes, it pushes a `StateChanged` event on the socket. The Mac client subscribes and triggers a sync. **This is a new wire protocol message** - additive, backward compatible (older clients ignore unknown messages).

#### 3.2 Local port collision detection

Before opening a forward, check that the local port is free. If not:

- Resolve which local process holds it (we already have a scanner for the local backend - reuse it).
- Surface the conflict in the UI: "Port 4060 is in use locally by `vite (pid 12345)`. The remote forward for project X cannot be opened."
- Provide two actions: "Kill local process" (with confirmation) or "Skip this port" (adds the port to the per-backend `excluded_ports` blocklist).

#### 3.3 ControlMaster requirement

For `ssh -O forward` / `ssh -O cancel` to work, a ControlMaster session must be open. Portsage's behavior:

- Detect: run `ssh -O check <alias>`. If it succeeds, we have a master. If not, no master exists.
- If no master and `auto_forward` is enabled, **portsage opens its own master**: `ssh -fNM -S <portsage_managed_path> <alias>`. Stores the path under `~/.local/state/portsage/cm-<alias>.sock`. Sets `ControlPersist` via `-o ControlPersist=...` so the master closes after configured idle.
- All subsequent `ssh -O forward` calls use the same control path explicitly.

Document a recommended `~/.ssh/config` snippet (in the UI add-backend modal):

```
Host dev
  ControlMaster auto
  ControlPath ~/.ssh/cm-%r@%h:%p.sock
  ControlPersist 4h
```

If present, Portsage uses the user's config. If absent, Portsage falls back to its own managed control socket.

#### 3.4 UI changes

**Sidebar dropdown for remote backend**: when "Auto-forward" is on, show a small forward icon next to each project that has at least one forwarded port.

**Port row** (project detail): the port number gets a third state besides "active" and "inactive":

- Active and forwarded (amber dot + small arrow icon)
- Active and not forwarded (amber dot, no arrow)
- Inactive (grey dot)

Hover on the arrow shows a tooltip: "Forwarded as localhost:4060". Click the arrow toggles the forward.

**Settings > Remote backends**: per backend, a sub-table of "Excluded ports" - the per-backend blocklist of ports that should never be forwarded (e.g. port already used by a local service the user runs in parallel).

**Notification on conflict**: macOS notification when a forward fails due to local port conflict. Includes the conflicting process name. Click opens the affected project panel.

#### 3.5 Lifecycle

- App start: for each remote backend with `auto_forward = true`, open the tunnel + sync forwards.
- Backend goes down: mark all its forwards as Failed, keep the entries (will reopen on reconnect).
- Backend toggle off: cancel all its forwards, remove from the table.
- App quit: cancel all forwards cleanly via `ssh -O cancel`. Send the final ControlMaster exit if Portsage opened it. Don't tear down user-managed ControlMaster sessions.

### Acceptance criteria

- [ ] Toggling "Auto-forward" on a backend opens forwards for all its currently-registered ports within 2 seconds.
- [ ] Registering a new port on the remote backend (via MCP, CLI, or UI) automatically opens its forward.
- [ ] Releasing or removing a port cancels the corresponding forward.
- [ ] A local port conflict produces a clear, dismissible UI notification with both actions available.
- [ ] Opening `http://localhost:<port>` in the browser reaches the remote service end-to-end (smoke test with `python -m http.server` running on the remote).
- [ ] Quitting the Mac app cleanly closes all SSH forwards (verified via `ss -ltn` on the Mac before/after).

### Effort estimate

3-5 days. ForwardManager and protocol event extension are the meat. UI changes are incremental on top of Phase 2.

---

## Phase 4 - Quality of life

These are independent improvements, ship in any order after Phase 3.

### 4.1 Project migration between backends

UI action: "Move project to backend X". Behavior:

- On the source backend: read project + ports + range.
- On the target backend: reserve a new range, register each service. **Service ports may change** if the target backend's range doesn't match.
- Print a report at the end: old ports vs new ports per service.
- The user is responsible for updating `.env` files / config in the project. Portsage does not modify project files - too risky.

Alternative considered: "preserve ports across backends" by registering the source ports verbatim on the target. Rejected: target backend's `base_port`/`range_size` config would need to be honored, and collisions with existing target projects are possible.

### 4.2 Health dashboard

Add a "Status" tab to settings:

- Per backend: connection state, last seen timestamp, version of remote `portsage-server`, count of registered projects/ports/forwards.
- Per active forward: latency (ping the local socket end-to-end), throughput placeholder for future.

Useful when something silently breaks.

### 4.3 CLI integration enhancements

- `portsage backends list / add / remove` to manage remote backends from the CLI (parity with the Settings UI).
- `portsage list --backend dev` already covered by Phase 2 - polish edge cases.
- `portsage forward open|close <port> --backend dev` for explicit one-off forwards bypassing auto-forward.

### 4.4 Multi-user remote server

Today the Linux `portsage-server` is single-user (one socket, one DB, owned by user `portsage` or the dev user). For shared servers, gate by Unix group permissions: members of the `portsage` group can read; only the owner can mutate. Out of scope unless we get a real multi-user need - the dev server today is single-user (only `simone` connects).

### 4.5 Auto-discover Tailscale hosts

Tailscale has a CLI (`tailscale status --json`) that lists hosts in the tailnet. Add a "Discover backends" button that lists tailnet hosts and offers to pre-fill the ssh_alias. Convenience only; the underlying config still goes through `~/.ssh/config`.

---

## Open questions

1. **Should Portsage embed an SSH client (`russh`, `libssh2`)?** Current plan: no, shell out to `ssh`. Trade-offs:
   - Embedding: more control, more code, harder distribution (no system OpenSSH dependency), would need to reimplement ControlMaster.
   - Shelling out: relies on system OpenSSH (universally available on macOS), respects user's `~/.ssh/config` automatically, supports any auth method the system supports including hardware tokens, plays nicely with Tailscale and ProxyJump.
   - Verdict: shell out. Revisit only if portability becomes painful (it won't, OpenSSH is on every Mac).

2. **What about port forwarding over Tailscale Funnel / Serve instead of SSH?** Tailscale Serve exposes a service on the tailnet over HTTPS. Pros: zero SSH involvement, MagicDNS hostnames. Cons:
   - Different origin (not `localhost`), reintroduces CORS issues.
   - Requires Tailscale on both ends, not strictly worse but locks into Tailscale.
   - No `connect(socket)` for the Mac client to portsage-server (Tailscale doesn't forward Unix sockets, only TCP). We'd need to expose portsage-server on a TCP port on the tailnet IP.
   - Verdict: SSH wins for our use case. Tailscale stays as the underlying transport for the SSH connection (defense in depth: SSH over WireGuard).

3. **Database migration when project paths differ between Mac and server?** A user might have `~/Development/Projects/foo` on the Mac and `/home/simone/Development/Projects/foo` on the server. `find_project_by_path` is path-sensitive. Not a Portsage problem to solve; the user picks the path on each backend. We can show both paths side by side in the UI when migrating (Phase 4.1).

4. **Concurrent writes from multiple Macs to the same remote backend?** Two Macs both running Portsage, both attached to the same `dev` server. Today: the wire protocol is request/response over Unix socket; SQLite handles concurrent transactions. The "events" channel (Phase 3.1) needs to broadcast to all connected clients, not just one - straightforward broadcast model on the server side.

5. **Tray icon on Linux for users who want it?** Out of scope. The Linux build is server-only. If demand grows, build a separate `portsage-linux-tray` artifact later. Not on the roadmap.

## Suggested order

1. **Phase 1** (3-5 days) - unblocks Claude Code agents on the remote server. Highest priority. Sufficient to use the Hetzner box productively even without UI integration.
2. **Phase 2** (4-6 days) - the UI win. Daily-use UX goes from "two tools" to "one tool, two tabs".
3. **Phase 3** (3-5 days) - removes the last manual step (SSH tunnel management). After this the experience is "click a project on the remote tab, browser works on localhost".
4. **Phase 4** (open) - polish, ship items individually as needed.

Total to reach a complete remote-host story: ~2-3 weeks of focused work, shippable in increments.

## Compatibility and rollback

- All phases are additive. The local-only macOS workflow continues working identically.
- New SQLite tables on the Mac (`remote_backends`) are isolated; existing schema is untouched.
- New wire protocol messages (StateChanged event in Phase 3) are additive; older clients ignore unknown lines.
- The Linux server uses the same protocol; if it ever diverges, version negotiation can be added.

If a phase is rolled back (revert), the user loses the remote functionality but local state is unaffected.

## Relationship with the existing roadmap

- `ROADMAP.md` v0.7 "cross-platform" is **subsumed** by this document. The Linux scanner work, build pipeline, and config dirs are concretized in Phase 1.
- `ROADMAP.md` v0.7 "Windows support" is **not in scope** here. Separate epic.
- This document does not change any v0.4-v0.6 items (settings, distribution, kill/browser/CLI, tags, notifications, i18n).

When this plan starts execution, update `ROADMAP.md` to reference `docs/multi-host-evolution.md` for the v0.7 entries and remove the duplicate bullets.
