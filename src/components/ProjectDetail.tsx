import { useState } from "react";
import {
  Trash2,
  FolderOpen,
  Terminal,
  Plus,
  Power,
  Pencil,
  Archive,
  ArchiveRestore,
} from "lucide-react";
import { useConfirm } from "@/lib/dialog";
import { useToast } from "@/lib/toast";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIBadge } from "@/components/ui/UIBadge";
import { ForwardIndicator } from "@/components/PortForwardIndicator";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIPortLink } from "@/components/ui/UIPortLink";
import { UITable, type UITableColumn } from "@/components/ui/UITable";
import { AddPortForm } from "@/components/AddPortForm";
import { EditProjectForm } from "@/components/EditProjectForm";
import * as cmd from "@/lib/commands";
import type { KillEntry } from "@/lib/commands";
import { describeKillOutcome, summarizeKillEntries } from "@/lib/killOutcome";
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
  /** Shelve or unshelve the project. Range and ports are kept either way. */
  onSetArchived: (name: string, archived: boolean) => void;
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
  onSetArchived,
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
    // The trash keeps it for 30 days, so promising the opposite would be a
    // lie that pushes people away from a reversible action.
    const message =
      portsCount > 0
        ? `Delete project "${project.name}" and its ${portsCount} registered port${portsCount === 1 ? "" : "s"}? It goes to the trash and can be restored for 30 days.`
        : `Delete project "${project.name}"? It goes to the trash and can be restored for 30 days.`;
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
    const report = describeKillOutcome(port, outcome);
    if (report.ok) showSuccess(report.message);
    else showError(report.message);
  };

  const reportProjectOutcomes = (results: KillEntry[]) => {
    const { errors, success } = summarizeKillEntries(results);
    // Failures are reported one line per kind: a partially successful kill
    // must not hide the ports that stayed up behind a green toast.
    errors.forEach((message) => showError(message));
    if (success) showSuccess(success);
  };

  // Inactive ports are dimmed rather than hidden: the dot already encodes
  // the state, but toning the text down makes the list far quicker to scan.
  const tone = (p: PortStatus) => (p.active ? "" : "text-text-muted!");

  const portColumns: UITableColumn<PortStatus>[] = [
    {
      key: "status",
      width: "w-10",
      align: "center",
      cell: (p) => <UIStatus active={p.active} />,
    },
    {
      key: "service",
      header: "Service",
      cell: (p) => (
        <UIText variant="body" className={`truncate block ${tone(p)}`}>
          {p.service}
        </UIText>
      ),
    },
    {
      key: "process",
      header: "Process",
      width: "w-32",
      cell: (p) => (
        <UIText variant="mono" className="truncate block text-text-muted text-[11px]!">
          {p.active && p.process ? p.process : ""}
        </UIText>
      ),
    },
    {
      key: "pid",
      header: "PID",
      width: "w-16",
      align: "right",
      cell: (p) => (
        <UIText variant="mono" className="text-text-secondary text-[11px]! tabular-nums">
          {p.pid ?? ""}
        </UIText>
      ),
    },
    {
      key: "port",
      header: "Port",
      width: "w-16",
      align: "right",
      cell: (p) => (
        <span className={tone(p)}>
          <UIPortLink port={p.port} />
        </span>
      ),
    },
    {
      key: "forward",
      width: "w-10",
      align: "center",
      cell: (p) =>
        isRemote ? (
          <ForwardIndicator
            forward={forwards.byPort[p.port] ?? { state: "cancelled" }}
            port={p.port}
            onClick={() => handleToggleForward(p)}
          />
        ) : null,
    },
    {
      key: "kill",
      width: "w-10",
      align: "center",
      cell: (p) =>
        p.active ? (
          <UIButton
            variant="warning"
            size="icon-sm"
            className="opacity-0 group-hover:opacity-100"
            title="Stop process on this port"
            aria-label={`Stop process on port ${p.port}`}
            onClick={() => handleKillSingle(p)}
          >
            <Power size={14} aria-hidden="true" />
          </UIButton>
        ) : null,
    },
    {
      key: "remove",
      width: "w-10",
      align: "center",
      cell: (p) => (
        <UIButton
          variant="danger"
          size="icon-sm"
          className="opacity-0 group-hover:opacity-100"
          title="Remove port from project"
          aria-label={`Remove ${p.service} (port ${p.port}) from project`}
          onClick={() => onRemovePort(project.name, p.service)}
        >
          <Trash2 size={14} aria-hidden="true" />
        </UIButton>
      ),
    },
  ];

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
            <UIButton
              variant="ghost"
              size="icon"
              title={
                project.archived_at
                  ? "Bring back into the list"
                  : "Archive: keeps range and ports, hides it from the list"
              }
              aria-label={project.archived_at ? "Unarchive project" : "Archive project"}
              onClick={() => onSetArchived(project.name, !project.archived_at)}
            >
              {project.archived_at ? (
                <ArchiveRestore size={16} aria-hidden="true" />
              ) : (
                <Archive size={16} aria-hidden="true" />
              )}
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

      <UITable
        columns={portColumns}
        rows={project.ports}
        rowKey={(p) => p.id}
        empty="No ports registered"
        dense
      />
    </div>
  );
}
