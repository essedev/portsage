import { useState, useEffect, useCallback } from "react";
import type { ProjectStatus, UnmanagedPort } from "@/lib/types";
import * as cmd from "@/lib/commands";
import { humanizeError } from "@/lib/errors";

export function useProjects() {
  const [projects, setProjects] = useState<ProjectStatus[]>([]);
  const [unmanagedPorts, setUnmanagedPorts] = useState<UnmanagedPort[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const clearError = useCallback(() => setError(null), []);

  const refresh = useCallback(async () => {
    try {
      const [data, unmanaged] = await Promise.all([
        cmd.listProjects(),
        cmd.listUnmanagedPorts(),
      ]);
      setProjects(data);
      setUnmanagedPorts(unmanaged);
    } catch (err) {
      // Background refresh errors are surfaced to the UI but not allowed to
      // mask the previous good data. Polling will retry on the next tick.
      setError(humanizeError(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  }, [refresh]);

  // Wraps a mutating action so failures surface as a UI error and the caller
  // can decide whether to react. Returns true on success, false on failure;
  // never re-throws so callers don't need their own try/catch.
  const run = async (fn: () => Promise<unknown>): Promise<boolean> => {
    try {
      await fn();
      setError(null);
      await refresh();
      return true;
    } catch (err) {
      setError(humanizeError(err));
      return false;
    }
  };

  const create = (name: string, path?: string) =>
    run(() => cmd.createProject(name, path));

  const remove = (id: number) => run(() => cmd.deleteProject(id));

  const addPort = (projectId: number, service: string, port: number) =>
    run(() => cmd.addPort(projectId, service, port));

  const removePort = (id: number) => run(() => cmd.removePort(id));

  return {
    projects,
    unmanagedPorts,
    loading,
    error,
    clearError,
    refresh,
    create,
    remove,
    addPort,
    removePort,
  };
}
