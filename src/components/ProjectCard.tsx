import { UICard } from "@/components/ui/UICard";
import { UIText } from "@/components/ui/UIText";
import { UIBadge } from "@/components/ui/UIBadge";
import { PortRow } from "@/components/PortRow";
import type { ProjectStatus } from "@/lib/types";

interface ProjectCardProps {
  project: ProjectStatus;
  selected?: boolean;
  compact?: boolean;
  onClick?: () => void;
  onRemovePort?: (id: number) => void;
}

export function ProjectCard({
  project,
  selected = false,
  compact = false,
  onClick,
  onRemovePort,
}: ProjectCardProps) {
  const activePorts = project.ports.filter((p) => p.active).length;
  const totalPorts = project.ports.length;

  return (
    <UICard
      glow={selected}
      onClick={onClick}
      className={selected ? "border-accent-amber" : ""}
    >
      <div className="flex items-center justify-between gap-[var(--spacing-2)]">
        <UIText variant="section">{project.name}</UIText>
        <div className="flex items-center gap-[var(--spacing-2)]">
          <UIText variant="mono" className="text-text-secondary">
            {project.range_start}-{project.range_end}
          </UIText>
          {totalPorts > 0 && (
            <UIBadge variant={activePorts > 0 ? "active" : "inactive"}>
              {activePorts}/{totalPorts}
            </UIBadge>
          )}
        </div>
      </div>

      {!compact && project.ports.length > 0 && (
        <div className="mt-[var(--spacing-2)] flex flex-col gap-[var(--spacing-1)]">
          {project.ports.map((port) => (
            <PortRow key={port.id} port={port} onRemove={onRemovePort} />
          ))}
        </div>
      )}
    </UICard>
  );
}
