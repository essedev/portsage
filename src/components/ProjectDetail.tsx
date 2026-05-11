import { useState } from "react";
import { Trash2, FolderOpen, Terminal, Plus, Power } from "lucide-react";
import { useConfirm } from "@/lib/dialog";
import { useToast } from "@/lib/toast";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIBadge } from "@/components/ui/UIBadge";
import { PortRow } from "@/components/PortRow";
import { AddPortForm } from "@/components/AddPortForm";
import * as cmd from "@/lib/commands";
import type { ProjectStatus, PortStatus, KillOutcome } from "@/lib/types";

interface ProjectDetailProps {
  project: ProjectStatus;
  onDelete: (id: number) => void;
  onAddPort: (projectId: number, service: string, port: number) => void;
  onRemovePort: (id: number) => void;
  onKillPort: (port: number) => Promise<KillOutcome | null>;
  onKillProject: (
    projectId: number,
  ) => Promise<Array<[number, KillOutcome]> | null>;
}

export function ProjectDetail({
  project,
  onDelete,
  onAddPort,
  onRemovePort,
  onKillPort,
  onKillProject,
}: ProjectDetailProps) {
  const [showAddPort, setShowAddPort] = useState(false);
  const confirm = useConfirm();
  const { showError, showSuccess } = useToast();
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
    if (ok) onDelete(project.id);
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
    const results = await onKillProject(project.id);
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
    }
  };

  const reportProjectOutcomes = (results: Array<[number, KillOutcome]>) => {
    if (results.length === 0) {
      showSuccess("No active ports to stop");
      return;
    }
    const denied = results.filter(([, o]) => o === "permission_denied");
    if (denied.length > 0) {
      const ports = denied.map(([p]) => p).join(", ");
      showError(`Permission denied for port${denied.length === 1 ? "" : "s"} ${ports}`);
      return;
    }
    const killed = results.filter(([, o]) => o === "killed").length;
    const terminated = results.filter(([, o]) => o === "terminated").length;
    const parts: string[] = [];
    if (terminated > 0) parts.push(`${terminated} stopped`);
    if (killed > 0) parts.push(`${killed} force-killed`);
    showSuccess(parts.length > 0 ? parts.join(", ") : "Done");
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-4)] p-[var(--spacing-5)]">
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
        <div className="flex items-center gap-[var(--spacing-1)]">
          {project.path && (
            <>
              <UIButton
                variant="ghost"
                size="icon"
                title="Open in Finder"
                onClick={() => cmd.openInFinder(project.path!)}
              >
                <FolderOpen size={16} />
              </UIButton>
              <UIButton
                variant="ghost"
                size="icon"
                title="Open in Terminal"
                onClick={() => cmd.openInTerminal(project.path!)}
              >
                <Terminal size={16} />
              </UIButton>
            </>
          )}
          <UIButton
            variant="ghost"
            size="icon"
            onClick={handleKillAll}
            disabled={activePorts === 0}
            title={
              activePorts === 0
                ? "No active ports to stop"
                : `Stop all ${activePorts} active port${activePorts === 1 ? "" : "s"}`
            }
          >
            <Power size={16} />
          </UIButton>
          <UIButton
            variant="danger"
            size="icon"
            onClick={handleDelete}
            title="Remove project"
          >
            <Trash2 size={16} />
          </UIButton>
        </div>
      </div>

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
        >
          <Plus size={16} />
          Add
        </UIButton>
      </div>

      {showAddPort && (
        <AddPortForm
          rangeStart={project.range_start}
          rangeEnd={project.range_end}
          usedPorts={project.ports.map((p) => p.port)}
          onSubmit={(service, port) => {
            onAddPort(project.id, service, port);
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
          </div>
          {project.ports.map((port) => (
            <PortRow
              key={port.id}
              port={port}
              onRemove={onRemovePort}
              onKill={handleKillSingle}
            />
          ))}
        </div>
      )}
    </div>
  );
}
