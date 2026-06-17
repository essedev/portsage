use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "portsage",
    bin_name = "portsage",
    about = "Manage port allocation across development projects",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOpts,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Args, Debug, Default, Clone)]
pub struct GlobalOpts {
    /// Output machine-readable JSON (raw protocol payloads).
    #[arg(long, global = true)]
    pub json: bool,

    /// Pipe-friendly output: one value per line, no headers.
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Skip the auto-spawn flow; fail immediately if the backend is not running.
    #[arg(long, global = true)]
    pub no_autospawn: bool,

    /// Path to the Portsage app binary (used by auto-spawn instead of the default).
    #[arg(long, global = true, env = "PORTSAGE_APP")]
    pub app: Option<std::path::PathBuf>,

    /// Override the Unix socket path (mainly for tests).
    #[arg(long, global = true, env = "PORTSAGE_SOCKET")]
    pub socket: Option<std::path::PathBuf>,

    /// Target a configured remote backend by name (Mac only). The CLI looks
    /// up the backend's local-side forwarded socket via the local Portsage
    /// app and connects to that path. Requires the Portsage app to be
    /// running with the tunnel open (use the Settings UI to add and Test the
    /// backend first).
    #[arg(long, global = true, env = "PORTSAGE_BACKEND")]
    pub backend: Option<String>,

    /// Skip the confirmation prompt on destructive commands (`release`, `kill`,
    /// `kill-project`). Global so it works in either position:
    /// `portsage -y release foo` and `portsage release foo -y` are equivalent.
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List registered projects and their ports.
    List {
        /// Restrict to the project whose path matches the current working directory.
        #[arg(long)]
        here: bool,
        /// Restrict to a single named project.
        #[arg(long)]
        project: Option<String>,
        /// Show only ports that are currently active (listening).
        #[arg(long)]
        active: bool,
    },

    /// Show details for the project the current working directory belongs to.
    Status,

    /// Reserve the next free port range for a new project.
    Reserve {
        /// Project name. Defaults to the basename of `--path` / current dir when omitted.
        name: Option<String>,
        /// Filesystem path of the project. Implies `--here` defaults to it.
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        /// Use the current working directory as the project path.
        #[arg(long)]
        here: bool,
    },

    /// Register a specific port for a service inside a project's range.
    Register {
        /// Service name (e.g. vite, postgres).
        service: String,
        /// Port number (must be inside the project's range).
        port: i64,
        /// Project name. Defaults to the project resolved by `--here`.
        #[arg(long)]
        project: Option<String>,
        /// Resolve the project from the current working directory.
        #[arg(long)]
        here: bool,
    },

    /// Remove a single port from a project.
    Remove {
        service: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        here: bool,
    },

    /// Release a project's range, deleting all its ports.
    Release {
        /// Project name. Falls back to `--here` if omitted.
        name: Option<String>,
        #[arg(long)]
        here: bool,
    },

    /// Rename a project and/or change its path. The range and every registered
    /// port are preserved. Provide a new name, `--path`, or both.
    Rename {
        /// Current project name.
        current: String,
        /// New project name. Omit to change only the path.
        new_name: Option<String>,
        /// New filesystem path for the project. Pass an empty string to clear it.
        #[arg(long)]
        path: Option<String>,
    },

    /// Scan the machine for active TCP ports.
    Scan {
        /// Show only active ports that are not registered to any project and
        /// are above the dev-port threshold.
        #[arg(long)]
        unmanaged: bool,
    },

    /// Kill the process listening on a port (SIGTERM, grace, SIGKILL).
    Kill { port: i64 },

    /// Kill every active port registered to a project, in parallel.
    KillProject {
        name: Option<String>,
        #[arg(long)]
        here: bool,
    },

    /// Open a port (or a service in the current/named project) in the browser.
    Open {
        /// Either a port number or a service name. When a service name is given,
        /// it is resolved within the project from `--project` / `--here`.
        target: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        here: bool,
    },

    /// Read or change configuration (`base_port`, `range_size`).
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Diagnose the local install: socket reachable, app located, etc.
    Doctor,

    /// Manage the Claude Code MCP integration (server files, registration,
    /// skill, allowlist).
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },

    /// Check for a newer Portsage release and, when possible, upgrade in place.
    SelfUpdate {
        /// Only check for a newer version, don't run an upgrade.
        #[arg(long)]
        check: bool,
        /// Skip the confirmation prompt before running the upgrade.
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show the current config.
    Get,
    /// Set a config value.
    Set { key: String, value: String },
}

#[derive(Subcommand, Debug)]
pub enum McpAction {
    /// Lay down the MCP server files, register them with Claude Code, install
    /// the skill, and add tool permissions.
    Install {
        /// Register in `./.mcp.json` next to the cwd instead of `~/.claude.json`.
        #[arg(long)]
        project: bool,
        /// Skip `uv sync` (useful in CI / when uv is not available; you can run
        /// it manually later from the install dir).
        #[arg(long)]
        skip_uv: bool,
    },
    /// Reverse the install. By default only de-registers; pass `--wipe` to
    /// also delete the install dir.
    Uninstall {
        /// Also delete the install dir (`<data_dir>/portsage/mcp`).
        #[arg(long)]
        wipe: bool,
    },
    /// Report the current state of every install artifact.
    Status,
}
