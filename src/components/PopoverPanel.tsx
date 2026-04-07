import { UIText } from "@/components/ui/UIText";
import { UIBadge } from "@/components/ui/UIBadge";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIButton } from "@/components/ui/UIButton";
import { useProjects } from "@/features/projects/useProjects";
import { invoke } from "@tauri-apps/api/core";

export function PopoverPanel() {
  const { projects } = useProjects();

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
        className="flex items-center justify-between px-[var(--spacing-4)] h-10 shrink-0"
        data-tauri-drag-region
      >
        <UIText
          variant="title"
          className="text-[14px]!"
          style={{ textShadow: "0 0 12px var(--color-accent-amber-glow)" }}
        >
          portsage
        </UIText>
        {totalPorts > 0 && (
          <UIBadge variant={totalActive > 0 ? "active" : "inactive"}>
            {totalActive} active
          </UIBadge>
        )}
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
                      <UIText variant="body" className="flex-1 text-[12px]!">
                        {port.service}
                      </UIText>
                      <UIText variant="mono" className="text-[11px]! tabular-nums">
                        {port.port}
                      </UIText>
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
        <UIButton variant="ghost" className="text-[12px]!" onClick={quit}>
          Quit
        </UIButton>
        <UIText variant="label">
          {totalActive}/{totalPorts} active ports
        </UIText>
        <UIButton variant="ghost" className="text-[12px]!" onClick={openMain}>
          Open portsage
        </UIButton>
      </footer>
    </div>
  );
}
