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
        flex items-center justify-between
        bg-bg-surface border-b border-border-subtle
        px-[var(--spacing-3)] h-10
      "
      data-tauri-drag-region
    >
      <div className="flex items-baseline gap-[var(--spacing-2)]">
        <GrimText
          variant="title"
          as="h1"
          className="select-none -mt-1"
          style={{ textShadow: "0 0 12px var(--color-accent-amber-glow)" }}
        >
          portsage
        </GrimText>
        <GrimText variant="label" className="text-text-muted select-none">
          your port sage
        </GrimText>
      </div>
      {totalActive > 0 && (
        <GrimBadge variant="active">{totalActive} active</GrimBadge>
      )}
    </header>
  );
}
