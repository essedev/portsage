"""Portsage MCP server - thin client that forwards requests to the Tauri backend via Unix socket."""

import json
import os
import socket
import sys
from pathlib import Path

from mcp.server.fastmcp import FastMCP


def _socket_path() -> Path:
    """Return the portsage socket path matching what the Rust app creates.

    Mirrors `dirs::config_dir()` from the Rust side:
      - macOS:   ~/Library/Application Support/portsage/portsage.sock
      - Linux:   ~/.config/portsage/portsage.sock
      - Windows: %APPDATA%\\portsage\\portsage.sock
    """
    home = Path.home()
    if sys.platform == "darwin":
        base = home / "Library" / "Application Support"
    elif sys.platform == "win32":
        base = Path(os.environ.get("APPDATA", str(home)))
    else:
        base = home / ".config"
    return base / "portsage" / "portsage.sock"


SOCKET_PATH = _socket_path()

mcp = FastMCP("portsage", json_response=True)


def _send(method: str, params: dict | None = None) -> dict:
    """Send a JSON request to the Portsage Unix socket and return the response."""
    if not SOCKET_PATH.exists():
        return {"error": "Portsage app is not running. Start it first."}

    request = {"method": method}
    if params:
        request["params"] = params

    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        # Bound every blocking call. Without this, a hung Rust backend would
        # freeze the MCP tool call indefinitely and hang Claude Code.
        sock.settimeout(5.0)
        sock.connect(str(SOCKET_PATH))
        sock.sendall((json.dumps(request) + "\n").encode())

        data = b""
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            data += chunk
            if b"\n" in data:
                break
        sock.close()

        response = json.loads(data.decode().strip())
        if "error" in response:
            return {"error": response["error"]}
        return response.get("result", response)
    except ConnectionRefusedError:
        return {"error": "Cannot connect to Portsage. Is the app running?"}
    except socket.timeout:
        return {"error": "Portsage backend did not respond within 5s."}
    except Exception as e:
        return {"error": str(e)}


@mcp.tool()
def list_all() -> str:
    """List all registered projects with their port ranges, assigned ports, and active state.
    Each port includes pid and process name when active. Use this as the first step
    to understand the current allocation before reserving or registering anything."""
    return json.dumps(_send("list_all"), indent=2)


@mcp.tool()
def reserve_range(project_name: str, path: str | None = None) -> str:
    """Reserve the next available port range for a new project.

    Returns the created project including its id and assigned range_start/range_end.

    Args:
        project_name: Name of the project (e.g. 'my-app')
        path: Optional filesystem path to the project directory. Pass this so that
              `find_project_by_path` can later resolve the project from a working
              directory inside it.
    """
    params: dict = {"name": project_name}
    if path:
        params["path"] = path
    return json.dumps(_send("reserve_range", params), indent=2)


@mcp.tool()
def register_port(project_name: str, service: str, port: int) -> str:
    """Register a specific port for a service within a project's range.

    Returns the created port row including its id.

    Args:
        project_name: Name of the project
        service: Name of the service (e.g. 'vite', 'postgres', 'redis')
        port: Port number to register (must be within the project's range)
    """
    return json.dumps(_send("register_port", {
        "project": project_name,
        "service": service,
        "port": port,
    }), indent=2)


@mcp.tool()
def remove_port(project_name: str, service: str) -> str:
    """Remove a single port from a project, identified by project and service name.

    Args:
        project_name: Name of the project
        service: Name of the service to remove
    """
    return json.dumps(_send("remove_port", {
        "project": project_name,
        "service": service,
    }), indent=2)


@mcp.tool()
def release_project(project_name: str) -> str:
    """Release a project's port range, freeing all its ports.

    Args:
        project_name: Name of the project to release
    """
    return json.dumps(_send("release_project", {"name": project_name}), indent=2)


@mcp.tool()
def scan_active() -> str:
    """Scan all active (LISTEN) TCP ports on the machine.

    Returns a list of objects with port, process name, and pid - useful when
    cross-referencing with registered ports to identify zombies or collisions."""
    return json.dumps(_send("scan_active"), indent=2)


@mcp.tool()
def list_unmanaged() -> str:
    """List active TCP ports above 3000 that are not registered to any project
    and not on the system-services blocklist. Useful to spot rogue processes
    that should either be killed or assigned to a project."""
    return json.dumps(_send("list_unmanaged"), indent=2)


@mcp.tool()
def next_range() -> str:
    """Peek at the next free port range without reserving it. Returns
    {range_start, range_end}."""
    return json.dumps(_send("next_range"), indent=2)


@mcp.tool()
def get_config() -> str:
    """Get current global configuration: base_port and range_size."""
    return json.dumps(_send("get_config"), indent=2)


@mcp.tool()
def set_config(key: str, value: str) -> str:
    """Set a configuration value. Only `base_port` and `range_size` are accepted.

    Args:
        key: 'base_port' or 'range_size'
        value: the new value, as a string (e.g. "5000")
    """
    return json.dumps(_send("set_config", {"key": key, "value": value}), indent=2)


@mcp.tool()
def kill_port(port: int) -> str:
    """Kill the process listening on the given port. Sends SIGTERM, waits 2
    seconds, then escalates to SIGKILL if the process is still alive.

    Returns {outcome: terminated | killed | not_active | permission_denied}.

    Args:
        port: Port number to free
    """
    return json.dumps(_send("kill_port", {"port": port}), indent=2)


@mcp.tool()
def kill_project(project_name: str) -> str:
    """Kill every active port registered to a project, in parallel. Returns a
    list of {port, outcome} entries (only ports that were actually active).

    Args:
        project_name: Name of the project whose ports should be freed
    """
    return json.dumps(_send("kill_project", {"name": project_name}), indent=2)


@mcp.tool()
def open_in_browser(port: int) -> str:
    """Open http://localhost:<port> in the user's default browser. Useful to
    surface a dev server quickly after registering it.

    Args:
        port: Port to open
    """
    return json.dumps(_send("open_in_browser", {"port": port}), indent=2)


@mcp.tool()
def find_project_by_path(path: str) -> str:
    """Reverse lookup: given an absolute filesystem path, return the project
    whose registered path equals it or is an ancestor of it. Returns null when
    no project matches.

    Args:
        path: An absolute path (typically the current working directory)
    """
    return json.dumps(_send("find_project_by_path", {"path": path}), indent=2)


def main():
    """Entry point for local MCP server via stdio."""
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
