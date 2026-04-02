import { invoke } from "@tauri-apps/api/core";
import type { ProjectStatus, PortStatus, UnmanagedPort } from "./types";

export function listProjects(): Promise<ProjectStatus[]> {
  return invoke("list_projects");
}

export function createProject(
  name: string,
  path?: string,
): Promise<ProjectStatus> {
  return invoke("create_project", { name, path });
}

export function deleteProject(id: number): Promise<void> {
  return invoke("delete_project", { id });
}

export function addPort(
  projectId: number,
  service: string,
  port: number,
): Promise<PortStatus> {
  return invoke("add_port", { projectId, service, port });
}

export function removePort(id: number): Promise<void> {
  return invoke("remove_port", { id });
}

export function scanPorts(): Promise<number[]> {
  return invoke("scan_ports");
}

export function getNextRange(): Promise<[number, number]> {
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

export function checkMcpInstalled(): Promise<boolean> {
  return invoke("check_mcp_installed");
}

export function installMcp(mcpDir: string): Promise<void> {
  return invoke("install_mcp", { mcpDir });
}

export function uninstallMcp(): Promise<void> {
  return invoke("uninstall_mcp");
}
