import { useState } from "react";
import { Trash2, FolderOpen, Terminal, Plus } from "lucide-react";
import { GrimText } from "@/components/ui/GrimText";
import { GrimButton } from "@/components/ui/GrimButton";
import { GrimDivider } from "@/components/ui/GrimDivider";
import { GrimBadge } from "@/components/ui/GrimBadge";
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
  const activePorts = project.ports.filter((p) => p.active).length;

  return (
    <div className="flex flex-col gap-[var(--spacing-4)] p-[var(--spacing-5)]">
      <div className="flex items-start justify-between">
        <div className="flex flex-col gap-[var(--spacing-1)]">
          <GrimText variant="title" as="h2">
            {project.name}
          </GrimText>
          {project.path && (
            <GrimText variant="mono" className="text-text-secondary text-[11px]">
              {project.path}
            </GrimText>
          )}
        </div>
        <div className="flex items-center gap-[var(--spacing-1)]">
          {project.path && (
            <>
              <GrimButton
                variant="ghost"
                title="Apri nel Finder"
                onClick={() => cmd.openInFinder(project.path!)}
              >
                <FolderOpen size={16} />
              </GrimButton>
              <GrimButton
                variant="ghost"
                title="Apri nel Terminale"
                onClick={() => cmd.openInTerminal(project.path!)}
              >
                <Terminal size={16} />
              </GrimButton>
            </>
          )}
          <GrimButton
            variant="danger"
            onClick={() => onDelete(project.id)}
            title="Rimuovi progetto"
          >
            <Trash2 size={16} />
          </GrimButton>
        </div>
      </div>

      <div className="flex items-center gap-[var(--spacing-3)]">
        <GrimText variant="mono">
          Range: {project.range_start}-{project.range_end}
        </GrimText>
        <GrimBadge variant={activePorts > 0 ? "active" : "inactive"}>
          {activePorts} attive su {project.ports.length}
        </GrimBadge>
      </div>

      <GrimDivider />

      <div className="flex items-center justify-between">
        <GrimText variant="label" as="h3">PORTE</GrimText>
        <GrimButton
          variant="ghost"
          onClick={() => setShowAddPort(!showAddPort)}
        >
          <Plus size={16} />
          Aggiungi
        </GrimButton>
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
          Nessuna porta registrata
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
