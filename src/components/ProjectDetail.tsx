import { useState } from "react";
import { Trash2, FolderOpen, Terminal, Plus, Power, Pencil } from "lucide-react";
import { useConfirm } from "@/lib/dialog";
import { useToast } from "@/lib/toast";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIBadge } from "@/components/ui/UIBadge";
import { PortRow } from "@/components/PortRow";
import { AddPortForm } from "@/components/AddPortForm";
import { EditProjectForm } from "@/components/EditProjectForm";
import * as cmd from "@/lib/commands";
import type { KillEntry } from "@/lib/commands";
import { useForwards } from "@/features/backends/useForwards";
import type {
  BackendTarget,
  ProjectStatus,
  PortStatus,
  KillOutcome,
} from "@/lib/types";

interface ProjectDetailProps {
  project: ProjectStatus;
  onDelete: (name: string) => void;
  /**
   * Rename a project and/or change its path. Returns true when the backend
   * accepted the change (range + ports are preserved server-side). Only the
   * provided fields change; an empty `newPath` clears the stored path.
   */
  onUpdate: (
    currentName: string,
    newName?: string,
    newPath?: string,
  ) => Promise<boolean>;
  onAddPort: (projectName: string, service: string, port: number) => void;
  onRemovePort: (projectName: string, service: string) => void;
  onKillPort: (port: number) => Promise<KillOutcome | null>;
  onKillProject: (projectName: string) => Promise<KillEntry[] | null>;
  /**
   * Active backend target. Drives display-only choices: when targeting a
   * Remote backend the project's filesystem path lives on the remote host,
   * so the "Open in Finder" / "Open in Terminal" buttons are hidden (they'd
   * open the Mac's local view of that path, which is almost never what the
   * user wants).
   */
  backendTarget?: BackendTarget | null;
}

export function ProjectDetail({
  project,
  onDelete,
  onUpdate,
  onAddPort,
  onRemovePort,
  onKillPort,
  onKillProject,
  backendTarget,
}: ProjectDetailProps) {
  const isRemote = backendTarget?.kind === "remote";
  const forwards = useForwards(backendTarget ?? null);

  const handleToggleForward = async (target: PortStatus) => {
    const current = forwards.byPort[target.port]?.state;
    if (current === "active" || current === "pending") {
      await forwards.disable(target.port);
    } else {
      await forwards.enable(target.port);
    }
  };
  const [showAddPort, setShowAddPort] = useState(false);
  const [editing, setEditing] = useState(false);
  const confirm = useConfirm();
  const { showError, showSuccess } = useToast();

  const handleUpdate = async (newName?: string, newPath?: string) => {
    const ok = await onUpdate(project.name, newName, newPath);
    if (ok) {
      setEditing(false);
      showSuccess("Project updated");
    }
  };
  const activePorts = project.ports.filter((p) => p.active).length;

  const handleDelete = async () => {
    const portsCount = project.ports.length;
    const message =
      portsCount > 0
        ? `Delete project "${project.name}" and its ${portsCount} registered port${portsCount === 1 ? "" : "s"}? This cannot be undone.`
        : `Delete project "${project.name}"? This cannot be undone.`;
    const ok = await confirm({
      title: "Delete project",
      message,
      kind: "warning",
      okLabel: "Delete",
      cancelLabel: "Cancel",
    });
    if (ok) onDelete(project.name);
  };

  const handleKillSingle = async (target: PortStatus) => {
    const procLine =
      target.process !== null && target.pid !== null
        ? `${target.service} (${target.process}, PID ${target.pid})`
        : `${target.service} on port ${target.port}`;
    const ok = await confirm({
      title: `Stop port ${target.port}?`,
      message: `${procLine}\n\nSIGTERM will be sent. If the process does not exit within 2s, SIGKILL is sent.`,
      kind: "warning",
      okLabel: "Stop",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    const outcome = await onKillPort(target.port);
    if (outcome) reportSingleOutcome(target.port, outcome);
  };

  const handleKillAll = async () => {
    const activeList = project.ports.filter((p) => p.active);
    if (activeList.length === 0) return;
    const lines = activeList
      .map((p) => {
        const proc =
          p.process !== null && p.pid !== null
            ? `${p.process}, PID ${p.pid}`
            : "unknown process";
        return `  ${p.port}  ${p.service.padEnd(12)} (${proc})`;
      })
      .join("\n");
    const ok = await confirm({
      title: `Stop ${activeList.length} active port${activeList.length === 1 ? "" : "s"} in "${project.name}"?`,
      message: `${lines}\n\nSIGTERM to each, escalating to SIGKILL after 2s if needed.`,
      kind: "warning",
      okLabel: "Stop",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    const results = await onKillProject(project.name);
    if (results) reportProjectOutcomes(results);
  };

  const reportSingleOutcome = (port: number, outcome: KillOutcome) => {
    switch (outcome) {
      case "terminated":
        showSuccess(`Port ${port} stopped`);
        break;
      case "killed":
        showSuccess(`Port ${port} force-killed (SIGKILL)`);
        break;
      case "not_active":
        showSuccess(`Port ${port} was already free`);
        break;
      case "permission_denied":
        showError(`Cannot stop port ${port}: permission denied (different user?)`);
        break;
      case "docker_stopped":
        showSuccess(`Port ${port} container stopped (docker)`);
        break;
      case "docker_error":
        showError(
          `Cannot stop port ${port}: docker container not found or daemon unavailable`,
        );
        break;
    }
  };

  const reportProjectOutcomes = (results: KillEntry[]) => {
    if (results.length === 0) {
      showSuccess("No active ports to stop");
      return;
    }
    const denied = results.filter((e) => e.outcome === "permission_denied");
    if (denied.length > 0) {
      const ports = denied.map((e) => e.port).join(", ");
      showError(`Permission denied for port${denied.length === 1 ? "" : "s"} ${ports}`);
      return;
    }
    const dockerFailed = results.filter((e) => e.outcome === "docker_error");
    if (dockerFailed.length > 0) {
      const ports = dockerFailed.map((e) => e.port).join(", ");
      showError(
        `Docker stop failed for port${dockerFailed.length === 1 ? "" : "s"} ${ports}`,
      );
      return;
    }
    const killed = results.filter((e) => e.outcome === "killed").length;
    const terminated = results.filter((e) => e.outcome === "terminated").length;
    const dockerStopped = results.filter((e) => e.outcome === "docker_stopped").length;
    const parts: string[] = [];
    if (terminated > 0) parts.push(`${terminated} stopped`);
    if (killed > 0) parts.push(`${killed} force-killed`);
    if (dockerStopped > 0)
      parts.push(`${dockerStopped} container${dockerStopped === 1 ? "" : "s"} stopped`);
    showSuccess(parts.length > 0 ? parts.join(", ") : "Done");
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-4)] p-[var(--spacing-5)]">
      {editing ? (
        <EditProjectForm
          initialName={project.name}
          initialPath={project.path}
          onSubmit={handleUpdate}
          onCancel={() => setEditing(false)}
        />
      ) : (
      <div className="flex items-start justify-between">
        <div className="flex flex-col gap-[var(--spacing-1)]">
          <UIText variant="title" as="h2">
            {project.name}
          </UIText>
          {project.path && (
            <UIText variant="mono" className="text-text-secondary text-[11px]">
              {project.path}
            </UIText>
          )}
        </div>
        {/* Toolbar split into two groups so navigation actions (edit, open
            path) don't sit next to destructive actions (stop processes, delete
            project). A subtle vertical divider reinforces the separation. */}
        <div className="flex items-center gap-[var(--spacing-3)]">
          <div className="flex items-center gap-[var(--spacing-1)]">
            <UIButton
              variant="ghost"
              size="icon"
              title="Rename or change path"
              aria-label="Edit project name or path"
              onClick={() => setEditing(true)}
            >
              <Pencil size={16} aria-hidden="true" />
            </UIButton>
            {project.path && !isRemote && (
              <>
                <UIButton
                  variant="ghost"
                  size="icon"
                  title="Open in Finder"
                  aria-label="Open project folder in Finder"
                  onClick={() => cmd.openInFinder(project.path!)}
                >
                  <FolderOpen size={16} aria-hidden="true" />
                </UIButton>
                <UIButton
                  variant="ghost"
                  size="icon"
                  title="Open in Terminal"
                  aria-label="Open project folder in Terminal"
                  onClick={() => cmd.openInTerminal(project.path!)}
                >
                  <Terminal size={16} aria-hidden="true" />
                </UIButton>
              </>
            )}
          </div>
          <div
            aria-hidden="true"
            className="h-5 w-px bg-border-subtle"
          />
          <div className="flex items-center gap-[var(--spacing-1)]">
            <UIButton
              variant="warning"
              size="icon"
              onClick={handleKillAll}
              disabled={activePorts === 0}
              title={
                activePorts === 0
                  ? "No active ports to stop"
                  : `Stop all ${activePorts} active port${activePorts === 1 ? "" : "s"}`
              }
              aria-label={
                activePorts === 0
                  ? "No active ports to stop"
                  : `Stop all ${activePorts} active port${activePorts === 1 ? "" : "s"}`
              }
            >
              <Power size={16} aria-hidden="true" />
            </UIButton>
            <UIButton
              variant="danger"
              size="icon"
              onClick={handleDelete}
              title="Remove project"
              aria-label={`Delete project ${project.name}`}
            >
              <Trash2 size={16} aria-hidden="true" />
            </UIButton>
          </div>
        </div>
      </div>
      )}

      <div className="flex items-center gap-[var(--spacing-3)]">
        <UIText variant="mono">
          Range: {project.range_start}-{project.range_end}
        </UIText>
        <UIBadge variant={activePorts > 0 ? "active" : "inactive"}>
          {activePorts} active of {project.ports.length}
        </UIBadge>
      </div>

      <UIDivider />

      <div className="flex items-center justify-between">
        <UIText variant="label" as="h3">PORTS</UIText>
        <UIButton
          variant="ghost"
          onClick={() => setShowAddPort(!showAddPort)}
          aria-expanded={showAddPort}
          aria-label={showAddPort ? "Close add port form" : "Add a new port to this project"}
        >
          <Plus size={16} aria-hidden="true" />
          Add
        </UIButton>
      </div>

      {showAddPort && (
        <AddPortForm
          rangeStart={project.range_start}
          rangeEnd={project.range_end}
          usedPorts={project.ports.map((p) => p.port)}
          onSubmit={(service, port) => {
            onAddPort(project.name, service, port);
            setShowAddPort(false);
          }}
          onCancel={() => setShowAddPort(false)}
        />
      )}

      {project.ports.length === 0 ? (
        <p className="text-text-muted text-[13px]">
          No ports registered
        </p>
      ) : (
        <div className="flex flex-col">
          {/* Column header - widths must match PortRow exactly (same padding,
              same gap, same per-column widths) so the labels align over their
              respective columns. */}
          <div className="flex items-center gap-[var(--spacing-2)] h-7 px-[var(--spacing-1)] pb-[var(--spacing-2)] mb-[var(--spacing-1)] border-b border-border-subtle">
            <div className="w-5 shrink-0" />
            <UIText variant="label" className="flex-1 min-w-0">Service</UIText>
            <UIText variant="label" className="w-32">Process</UIText>
            <UIText variant="label" className="w-16 text-right">PID</UIText>
            <UIText variant="label" className="w-14 text-right">Port</UIText>
            <div className="w-6 shrink-0" />
            <div className="w-6 shrink-0" />
            <div className="w-6 shrink-0" />
          </div>
          {project.ports.map((port) => (
            <PortRow
              key={port.id}
              port={port}
              onRemove={(p) => onRemovePort(project.name, p.service)}
              onKill={handleKillSingle}
              forward={isRemote ? forwards.byPort[port.port] ?? { state: "cancelled" } : undefined}
              onToggleForward={isRemote ? handleToggleForward : undefined}
            />
          ))}
        </div>
      )}
    </div>
  );
}
