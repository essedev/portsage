import { GrimText } from "@/components/ui/GrimText";
import { GrimBadge } from "@/components/ui/GrimBadge";
import { GrimStatus } from "@/components/ui/GrimStatus";
import { GrimDivider } from "@/components/ui/GrimDivider";
import { GrimButton } from "@/components/ui/GrimButton";
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
        <GrimText
          variant="title"
          className="text-[14px]!"
          style={{ textShadow: "0 0 12px var(--color-accent-amber-glow)" }}
        >
          grimport
        </GrimText>
        {totalPorts > 0 && (
          <GrimBadge variant={totalActive > 0 ? "active" : "inactive"}>
            {totalActive} attive
          </GrimBadge>
        )}
      </header>

      <GrimDivider />

      <div className="flex-1 overflow-y-auto px-[var(--spacing-3)] py-[var(--spacing-2)]">
        {projects.length === 0 ? (
          <p className="text-text-muted text-[13px] text-center py-[var(--spacing-4)]">
            Nessun progetto
          </p>
        ) : (
          <div className="flex flex-col gap-[var(--spacing-3)]">
            {projects.map((project) => {
              return (
                <div key={project.id} className="flex flex-col gap-[var(--spacing-1)]">
                  <div className="flex items-center justify-between">
                    <GrimText variant="section" className="text-[12px]!">
                      {project.name}
                    </GrimText>
                    <GrimText variant="mono" className="text-text-secondary text-[11px]!">
                      {project.range_start}-{project.range_end}
                    </GrimText>
                  </div>
                  {project.ports.map((port) => (
                    <div
                      key={port.id}
                      className="flex items-center gap-[var(--spacing-2)] pl-[var(--spacing-2)]"
                    >
                      <GrimStatus active={port.active} />
                      <GrimText variant="body" className="flex-1 text-[12px]!">
                        {port.service}
                      </GrimText>
                      <GrimText variant="mono" className="text-[11px]! tabular-nums">
                        {port.port}
                      </GrimText>
                    </div>
                  ))}
                  {project.ports.length === 0 && (
                    <GrimText variant="body" className="text-text-muted text-[11px]! pl-[var(--spacing-2)]">
                      Nessuna porta
                    </GrimText>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      <GrimDivider />

      <footer className="flex items-center justify-between px-[var(--spacing-4)] h-10 shrink-0">
        <GrimButton variant="ghost" className="text-[12px]!" onClick={quit}>
          Esci
        </GrimButton>
        <GrimText variant="label">
          {totalActive}/{totalPorts} porte attive
        </GrimText>
        <GrimButton variant="ghost" className="text-[12px]!" onClick={openMain}>
          Apri grimport
        </GrimButton>
      </footer>
    </div>
  );
}
