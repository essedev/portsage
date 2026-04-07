import { useState } from "react";
import { Trash2, FolderOpen, Terminal, Plus } from "lucide-react";
import { useConfirm } from "@/lib/dialog";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIBadge } from "@/components/ui/UIBadge";
import { PortRow } from "@/components/PortRow";
import { AddPortForm } from "@/components/AddPortForm";
import * as cmd from "@/lib/commands";
import type { ProjectStatus } from "@/lib/types";

interface ProjectDetailProps {
  project: ProjectStatus;
  onDelete: (id: number) => void;
  onAddPort: (projectId: number, service: string, port: number) => void;
  onRemovePort: (id: number) => void;
}

export function ProjectDetail({
  project,
  onDelete,
  onAddPort,
  onRemovePort,
}: ProjectDetailProps) {
  const [showAddPort, setShowAddPort] = useState(false);
  const confirm = useConfirm();
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
                title="Open in Finder"
                onClick={() => cmd.openInFinder(project.path!)}
              >
                <FolderOpen size={16} />
              </UIButton>
              <UIButton
                variant="ghost"
                title="Open in Terminal"
                onClick={() => cmd.openInTerminal(project.path!)}
              >
                <Terminal size={16} />
              </UIButton>
            </>
          )}
          <UIButton
            variant="danger"
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
        <div className="flex flex-col gap-[var(--spacing-1)]">
          {project.ports.map((port) => (
            <PortRow key={port.id} port={port} onRemove={onRemovePort} />
          ))}
        </div>
      )}
    </div>
  );
}
