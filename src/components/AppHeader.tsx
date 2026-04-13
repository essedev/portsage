import { UIText } from "@/components/ui/UIText";
import { UIBadge } from "@/components/ui/UIBadge";
import type { ProjectStatus } from "@/lib/types";

interface AppHeaderProps {
  projects: ProjectStatus[];
}

export function AppHeader({ projects }: AppHeaderProps) {
  const totalActive = projects
    .flatMap((p) => p.ports)
    .filter((p) => p.active).length;

  return (
    <header
      className="
        flex items-center justify-between
        bg-bg-surface border-b border-border-subtle
        px-[var(--spacing-3)] h-10
      "
      data-tauri-drag-region
    >
      <div className="flex items-baseline gap-[var(--spacing-2)]">
        <UIText
          variant="title"
          as="h1"
          className="select-none -mt-1"
          style={{ textShadow: "0 0 12px var(--color-accent-amber-glow)" }}
        >
          portsage
        </UIText>
        <UIText variant="label" className="text-text-muted select-none">
          ports under control
        </UIText>
      </div>
      {totalActive > 0 && (
        <UIBadge variant="active">{totalActive} active</UIBadge>
      )}
    </header>
  );
}
