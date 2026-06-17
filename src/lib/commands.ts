import { invoke } from "@tauri-apps/api/core";
import type {
  ProjectStatus,
  PortStatus,
  UnmanagedPort,
  KillOutcome,
  RemoteBackend,
  RemoteBackendForm,
  BackendTarget,
  TunnelStatus,
  ForwardStatus,
  ForwardExclusion,
} from "./types";

export function listProjects(): Promise<ProjectStatus[]> {
  return invoke("list_projects");
}

export function createProject(
  name: string,
  path?: string,
): Promise<ProjectStatus> {
  return invoke("create_project", { name, path });
}

export function deleteProject(name: string): Promise<void> {
  return invoke("delete_project", { name });
}

/**
 * Rename a project and/or change its path, keeping its range and registered
 * ports. Only the provided fields change; `newPath` set to an empty string
 * clears the path. At least one of `newName`/`newPath` must be passed.
 */
export function updateProject(
  currentName: string,
  newName?: string,
  newPath?: string,
): Promise<ProjectStatus> {
  return invoke("update_project", { currentName, newName, newPath });
}

export function addPort(
  projectName: string,
  service: string,
  port: number,
): Promise<PortStatus> {
  return invoke("add_port", { projectName, service, port });
}

export function removePort(
  projectName: string,
  service: string,
): Promise<void> {
  return invoke("remove_port", { projectName, service });
}

export function scanPorts(): Promise<number[]> {
  return invoke("scan_ports");
}

export function getNextRange(): Promise<{ range_start: number; range_end: number }> {
  return invoke("get_next_range");
}

export function listUnmanagedPorts(): Promise<UnmanagedPort[]> {
  return invoke("list_unmanaged_ports");
}

export function openInFinder(path: string): Promise<void> {
  return invoke("open_in_finder", { path });
}

export function openInTerminal(path: string): Promise<void> {
  return invoke("open_in_terminal", { path });
}

export function openInBrowser(port: number): Promise<void> {
  return invoke("open_in_browser", { port });
}

export function killPort(port: number): Promise<KillOutcome> {
  return invoke("kill_port", { port });
}

export interface KillEntry {
  port: number;
  outcome: KillOutcome;
}

export function killProject(projectName: string): Promise<KillEntry[]> {
  return invoke("kill_project", { projectName });
}

export function getConfig(): Promise<{ base_port: string; range_size: string }> {
  return invoke("get_config");
}

export function setConfig(key: string, value: string): Promise<void> {
  return invoke("set_config", { key, value });
}

export function exportData(destPath: string): Promise<void> {
  return invoke("export_data", { destPath });
}

export function importData(sourcePath: string): Promise<void> {
  return invoke("import_data", { sourcePath });
}

export function getMcpDir(): Promise<string> {
  return invoke("get_mcp_dir");
}

export function checkMcpInstalled(): Promise<boolean> {
  return invoke("check_mcp_installed");
}

export function installMcp(mcpDir: string): Promise<void> {
  return invoke("install_mcp", { mcpDir });
}

export function uninstallMcp(): Promise<void> {
  return invoke("uninstall_mcp");
}

// --- Multi-host backends (Phase 2) ---

export function listRemoteBackends(): Promise<RemoteBackend[]> {
  return invoke("list_remote_backends");
}

export function addRemoteBackend(form: RemoteBackendForm): Promise<RemoteBackend> {
  return invoke("add_remote_backend", { form });
}

export function updateRemoteBackend(
  id: number,
  form: RemoteBackendForm,
): Promise<RemoteBackend> {
  return invoke("update_remote_backend", { id, form });
}

export function removeRemoteBackend(id: number): Promise<void> {
  return invoke("remove_remote_backend", { id });
}

export function setRemoteBackendAutoForward(
  id: number,
  enabled: boolean,
): Promise<void> {
  return invoke("set_remote_backend_auto_forward", { id, enabled });
}

/**
 * Probe the remote backend by opening the tunnel and fetching its project
 * count. The returned number is the count of projects on the remote; errors
 * are thrown verbatim from the backend (so they can be shown in the UI).
 */
export function testRemoteBackend(name: string): Promise<number> {
  return invoke("test_remote_backend", { name });
}

export function getCurrentBackend(): Promise<BackendTarget> {
  return invoke("get_current_backend");
}

export function setCurrentBackend(target: BackendTarget): Promise<void> {
  return invoke("set_current_backend", { target });
}

export function getTunnelStatuses(): Promise<TunnelStatus[]> {
  return invoke("get_tunnel_statuses");
}

export function closeTunnel(name: string): Promise<void> {
  return invoke("close_tunnel", { name });
}

/**
 * Tauri event name emitted whenever a remote backend's tunnel state changes.
 * The payload is a {@link TunnelStatus}.
 */
export const TUNNEL_EVENT = "tunnel://state-changed" as const;

// --- Forwards (Phase 3) ---

export function listForwardStatuses(backend: string): Promise<ForwardStatus[]> {
  return invoke("list_forward_statuses", { backend });
}

export function enableForward(
  backend: string,
  port: number,
): Promise<ForwardStatus> {
  return invoke("enable_forward", { backend, port });
}

export function disableForward(
  backend: string,
  port: number,
): Promise<ForwardStatus> {
  return invoke("disable_forward", { backend, port });
}

export function syncForwards(backend: string): Promise<ForwardStatus[]> {
  return invoke("sync_forwards", { backend });
}

export function listForwardExclusions(
  backendId: number,
): Promise<ForwardExclusion[]> {
  return invoke("list_forward_exclusions", { backendId });
}

export function addForwardExclusion(
  backendId: number,
  port: number,
): Promise<ForwardExclusion> {
  return invoke("add_forward_exclusion", { backendId, port });
}

export function removeForwardExclusion(id: number): Promise<void> {
  return invoke("remove_forward_exclusion", { id });
}

/**
 * Tauri event name emitted whenever one or more forward states change. The
 * payload is `ForwardStatus[]` (delta snapshot for the affected backend).
 */
export const FORWARD_EVENT = "forward://state-changed" as const;
