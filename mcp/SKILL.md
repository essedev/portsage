---
name: portsage
description: >
  Manages port allocation across development projects.
  Use it when you need to assign ports to a new project, register services,
  or check which ports are in use.
---

# Portsage - Port Allocation Manager

Access to the local port database via the Portsage app.

## Available tools

### list_all
Shows all registered projects with their port range, services, and active state.
Use this as the first step to understand the current situation.

### reserve_range
Reserves the next free port range for a new project.
- `project_name`: project name (e.g. "my-app")
- `path`: optional path to the project directory

### register_port
Registers a specific port for a service inside a project's range.
- `project_name`: project name
- `service`: service name (e.g. "vite", "postgres", "redis", "minio")
- `port`: port number (must be inside the project's range)

### release_project
Releases a project's port range.
- `project_name`: name of the project to release

### scan_active
Scans all active TCP ports on the machine.

## Recommended workflow

When assigning ports to a new project:
1. Call `list_all` to see which ranges are taken
2. Call `reserve_range` with the project name
3. Use the assigned range's ports in docker-compose.yml and vite.config
4. Call `register_port` for each configured service

When checking conflicts:
1. Call `scan_active` to see active ports
2. Call `list_all` to cross-reference with registered ports
