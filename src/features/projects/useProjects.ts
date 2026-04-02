import { useState, useEffect, useCallback } from "react";
import type { ProjectStatus, UnmanagedPort } from "@/lib/types";
import * as cmd from "@/lib/commands";

export function useProjects() {
  const [projects, setProjects] = useState<ProjectStatus[]>([]);
  const [unmanagedPorts, setUnmanagedPorts] = useState<UnmanagedPort[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [data, unmanaged] = await Promise.all([
        cmd.listProjects(),
        cmd.listUnmanagedPorts(),
      ]);
      setProjects(data);
      setUnmanagedPorts(unmanaged);
    } catch (err) {
      console.error("Failed to load projects:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  }, [refresh]);

  const create = async (name: string, path?: string) => {
    await cmd.createProject(name, path);
    await refresh();
  };

  const remove = async (id: number) => {
    await cmd.deleteProject(id);
    await refresh();
  };

  const addPort = async (
    projectId: number,
    service: string,
    port: number,
  ) => {
    await cmd.addPort(projectId, service, port);
    await refresh();
  };

  const removePort = async (id: number) => {
    await cmd.removePort(id);
    await refresh();
  };

  return {
    projects,
    unmanagedPorts,
    loading,
    refresh,
    create,
    remove,
    addPort,
    removePort,
  };
}
