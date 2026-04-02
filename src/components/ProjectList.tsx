import { useState } from "react";
import { GrimSearch } from "@/components/ui/GrimSearch";
import { ProjectCard } from "@/components/ProjectCard";
import type { ProjectStatus } from "@/lib/types";

interface ProjectListProps {
  projects: ProjectStatus[];
  selectedId?: number;
  compact?: boolean;
  onSelect?: (project: ProjectStatus) => void;
  onRemovePort?: (id: number) => void;
}

export function ProjectList({
  projects,
  selectedId,
  compact = false,
  onSelect,
  onRemovePort,
}: ProjectListProps) {
  const [search, setSearch] = useState("");

  const filtered = projects.filter((p) =>
    p.name.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="flex flex-col gap-[var(--spacing-3)]">
      <GrimSearch
        value={search}
        onChange={(e) => setSearch(e.target.value)}
      />
      <div className="flex flex-col gap-[var(--spacing-2)] overflow-y-auto">
        {filtered.length === 0 && (
          <p className="text-text-muted text-[13px] text-center py-[var(--spacing-4)]">
            Nessun progetto trovato
          </p>
        )}
        {filtered.map((project) => (
          <ProjectCard
            key={project.id}
            project={project}
            selected={project.id === selectedId}
            compact={compact}
            onClick={() => onSelect?.(project)}
            onRemovePort={onRemovePort}
          />
        ))}
      </div>
    </div>
  );
}
