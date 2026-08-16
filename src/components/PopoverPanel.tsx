import { Power } from "lucide-react";
import { UIText } from "@/components/ui/UIText";
import { UIBadge } from "@/components/ui/UIBadge";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIButton } from "@/components/ui/UIButton";
import { UIPortLink } from "@/components/ui/UIPortLink";
import { useProjects } from "@/features/projects/useProjects";
import { invoke } from "@tauri-apps/api/core";

export function PopoverPanel() {
  const { projects: allProjects } = useProjects();
  // The popover is the at-a-glance view: archived projects are shelved and
  // have no business taking up room in 350x480.
  const projects = allProjects.filter((p) => !p.archived_at);

  const totalActive = projects
    .flatMap((p) => p.ports)
    .filter((p) => p.active).length;
  const totalPorts = projects.flatMap((p) => p.ports).length;

  const openMain = () => {
    invoke("show_main_window");
  };

  const quit = () => {
    invoke("quit_app");
  };

  return (
    <div className="flex flex-col h-screen bg-bg-surface rounded-[var(--radius-lg)] overflow-hidden">
      <header
        className="flex items-center justify-between gap-[var(--spacing-2)] px-[var(--spacing-4)] h-10 shrink-0"
        data-tauri-drag-region
      >
        <UIText
          variant="title"
          className="text-[14px]!"
          style={{ textShadow: "0 0 12px var(--color-accent-amber-glow)" }}
        >
          portsage
        </UIText>
        <div className="flex items-center gap-[var(--spacing-2)]">
          {totalPorts > 0 && (
            <UIBadge variant={totalActive > 0 ? "active" : "inactive"}>
              {totalActive} active
            </UIBadge>
          )}
          {/* Quit lives in the header as a low-key icon - it's a rare action
              vs. the primary "Open portsage" CTA in the footer. Power icon
              reads as "shut down the app". */}
          <UIButton
            variant="ghost"
            size="icon-sm"
            onClick={quit}
            title="Quit portsage"
            aria-label="Quit portsage"
          >
            <Power size={14} aria-hidden="true" />
          </UIButton>
        </div>
      </header>

      <UIDivider />

      <div className="flex-1 overflow-y-auto px-[var(--spacing-3)] py-[var(--spacing-2)]">
        {projects.length === 0 ? (
          <p className="text-text-muted text-[13px] text-center py-[var(--spacing-4)]">
            No projects
          </p>
        ) : (
          <div className="flex flex-col gap-[var(--spacing-3)]">
            {projects.map((project) => {
              return (
                <div key={project.id} className="flex flex-col gap-[var(--spacing-1)]">
                  <div className="flex items-center justify-between">
                    <UIText variant="section" className="text-[12px]!">
                      {project.name}
                    </UIText>
                    <UIText variant="mono" className="text-text-secondary text-[11px]!">
                      {project.range_start}-{project.range_end}
                    </UIText>
                  </div>
                  {project.ports.map((port) => (
                    <div
                      key={port.id}
                      className="flex items-center gap-[var(--spacing-2)] pl-[var(--spacing-2)]"
                    >
                      <UIStatus active={port.active} />
                      <UIText
                        variant="body"
                        className={`flex-1 text-[12px]! ${port.active ? "" : "text-text-secondary!"}`}
                      >
                        {port.service}
                      </UIText>
                      <UIPortLink port={port.port} className="text-[11px]!" />
                    </div>
                  ))}
                  {project.ports.length === 0 && (
                    <UIText variant="body" className="text-text-muted text-[11px]! pl-[var(--spacing-2)]">
                      No ports
                    </UIText>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      <UIDivider />

      <footer className="flex items-center justify-between px-[var(--spacing-4)] h-10 shrink-0">
        <UIText variant="label" className="tabular-nums">
          {totalActive}/{totalPorts} active
        </UIText>
        <UIButton variant="ghost" className="text-[12px]!" onClick={openMain}>
          Open portsage
        </UIButton>
      </footer>
    </div>
  );
}
