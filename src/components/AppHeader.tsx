import { GrimText } from "@/components/ui/GrimText";
import { GrimBadge } from "@/components/ui/GrimBadge";
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
        flex flex-col
        bg-bg-surface border-b border-border-subtle
      "
      data-tauri-drag-region
    >
      <div className="h-8 w-full" data-tauri-drag-region />
      <div className="flex items-center justify-between px-[var(--spacing-3)] pb-[var(--spacing-2)]">
        <div className="flex items-baseline gap-[var(--spacing-2)]">
          <GrimText
            variant="title"
            as="h1"
            className="select-none"
            style={{ textShadow: "0 0 12px var(--color-accent-amber-glow)" }}
          >
            grimport
          </GrimText>
          <GrimText variant="label" className="text-text-muted select-none">
            your port grimoire
          </GrimText>
        </div>
        {totalActive > 0 && (
          <GrimBadge variant="active">{totalActive} attive</GrimBadge>
        )}
      </div>
    </header>
  );
}
