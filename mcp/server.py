"""Grimport MCP server - thin client that forwards requests to the Tauri backend via Unix socket."""

import json
import socket
from pathlib import Path

from mcp.server.fastmcp import FastMCP

SOCKET_PATH = Path.home() / ".config" / "grimport" / "grimport.sock"

mcp = FastMCP("grimport", json_response=True)


def _send(method: str, params: dict | None = None) -> dict:
    """Send a JSON request to the Grimport Unix socket and return the response."""
    if not SOCKET_PATH.exists():
        return {"error": "Grimport app is not running. Start it first."}

    request = {"method": method}
    if params:
        request["params"] = params

    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
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
        return {"error": "Cannot connect to Grimport. Is the app running?"}
    except Exception as e:
        return {"error": str(e)}


@mcp.tool()
def list_all() -> str:
    """List all registered projects with their port ranges, assigned ports, and active status.
    Use this to get an overview of all port allocations."""
    result = _send("list_all")
    return json.dumps(result, indent=2)


@mcp.tool()
def reserve_range(project_name: str, path: str | None = None) -> str:
    """Reserve the next available port range for a new project.

    Args:
        project_name: Name of the project (e.g. 'my-app')
        path: Optional filesystem path to the project directory
    """
    params = {"name": project_name}
    if path:
        params["path"] = path
    result = _send("reserve_range", params)
    return json.dumps(result, indent=2)


@mcp.tool()
def register_port(project_name: str, service: str, port: int) -> str:
    """Register a specific port for a service within a project's range.

    Args:
        project_name: Name of the project
        service: Name of the service (e.g. 'vite', 'postgres', 'redis')
        port: Port number to register (must be within the project's range)
    """
    result = _send("register_port", {
        "project": project_name,
        "service": service,
        "port": port,
    })
    return json.dumps(result, indent=2)


@mcp.tool()
def release_project(project_name: str) -> str:
    """Release a project's port range, freeing all its ports.

    Args:
        project_name: Name of the project to release
    """
    result = _send("release_project", {"name": project_name})
    return json.dumps(result, indent=2)


@mcp.tool()
def scan_active() -> str:
    """Scan for all currently active (listening) TCP ports on the machine."""
    result = _send("scan_active")
    return json.dumps(result, indent=2)


def main():
    """Entry point for local MCP server via stdio."""
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
