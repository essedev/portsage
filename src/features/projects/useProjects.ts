import { useState, useEffect, useCallback } from "react";
import type { ProjectStatus, UnmanagedPort, KillOutcome } from "@/lib/types";
import * as cmd from "@/lib/commands";
import type { KillEntry } from "@/lib/commands";
import { humanizeError } from "@/lib/errors";
import { useToast } from "@/lib/toast";

export function useProjects() {
  const [projects, setProjects] = useState<ProjectStatus[]>([]);
  const [unmanagedPorts, setUnmanagedPorts] = useState<UnmanagedPort[]>([]);
  const [loading, setLoading] = useState(true);
  const { showError } = useToast();

  const refresh = useCallback(async () => {
    try {
      const [data, unmanaged] = await Promise.all([
        cmd.listProjects(),
        cmd.listUnmanagedPorts(),
      ]);
      setProjects(data);
      setUnmanagedPorts(unmanaged);
    } catch (err) {
      // Background refresh failures: surface to the user but keep the
      // previous good data on screen. Polling will retry on the next tick.
      showError(humanizeError(err));
    } finally {
      setLoading(false);
    }
  }, [showError]);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  }, [refresh]);

  // Wraps a mutating action so failures surface as a toast and the caller
  // can decide whether to react. Returns true on success, false on failure;
  // never re-throws so callers don't need their own try/catch.
  const run = async (fn: () => Promise<unknown>): Promise<boolean> => {
    try {
      await fn();
      await refresh();
      return true;
    } catch (err) {
      showError(humanizeError(err));
      return false;
    }
  };

  const create = (name: string, path?: string) =>
    run(() => cmd.createProject(name, path));

  const remove = (name: string) => run(() => cmd.deleteProject(name));

  const update = (currentName: string, newName?: string, newPath?: string) =>
    run(() => cmd.updateProject(currentName, newName, newPath));

  const addPort = (projectName: string, service: string, port: number) =>
    run(() => cmd.addPort(projectName, service, port));

  const removePort = (projectName: string, service: string) =>
    run(() => cmd.removePort(projectName, service));

  // Kill helpers don't go through `run`: callers need the KillOutcome to
  // decide whether to surface "permission denied" or "process already gone"
  // hints. Technical failures (channel errors) still produce an error toast.
  const killPort = async (port: number): Promise<KillOutcome | null> => {
    try {
      const outcome = await cmd.killPort(port);
      await refresh();
      return outcome;
    } catch (err) {
      showError(humanizeError(err));
      return null;
    }
  };

  const killProject = async (
    projectName: string,
  ): Promise<KillEntry[] | null> => {
    try {
      const result = await cmd.killProject(projectName);
      await refresh();
      return result;
    } catch (err) {
      showError(humanizeError(err));
      return null;
    }
  };

  return {
    projects,
    unmanagedPorts,
    loading,
    refresh,
    create,
    remove,
    update,
    addPort,
    removePort,
    killPort,
    killProject,
  };
}
