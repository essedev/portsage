import { useState, useEffect } from "react";
import { Plus, Settings, Plug, AlertTriangle } from "lucide-react";
import { GrimSearch } from "@/components/ui/GrimSearch";
import { GrimButton } from "@/components/ui/GrimButton";
import { GrimText } from "@/components/ui/GrimText";
import { GrimBadge } from "@/components/ui/GrimBadge";
import { GrimDivider } from "@/components/ui/GrimDivider";
import { AddProjectForm } from "@/components/AddProjectForm";
import * as cmd from "@/lib/commands";
import type { ProjectStatus, UnmanagedPort } from "@/lib/types";

type View = "project" | "unmanaged" | "settings";

interface SidebarProps {
  projects: ProjectStatus[];
  unmanagedPorts: UnmanagedPort[];
  selectedId?: number;
  activeView: View;
  onSelect: (project: ProjectStatus) => void;
  onCreate: (name: string, path?: string) => void;
  onShowSettings: () => void;
  onShowUnmanaged: () => void;
}

export function Sidebar({
  projects,
  unmanagedPorts,
  selectedId,
  activeView,
  onSelect,
  onCreate,
  onShowSettings,
  onShowUnmanaged,
}: SidebarProps) {
  const [search, setSearch] = useState("");
  const [showAdd, setShowAdd] = useState(false);
  const [mcpInstalled, setMcpInstalled] = useState(true);

  useEffect(() => {
    cmd.checkMcpInstalled().then(setMcpInstalled).catch(() => setMcpInstalled(false));
  }, [activeView]);

  const filtered = projects.filter((p) =>
    p.name.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <aside
      className="
        w-60 h-full flex flex-col
        bg-bg-surface border-r border-border-subtle
      "
    >
      <div className="p-[var(--spacing-3)] flex flex-col gap-[var(--spacing-2)]">
        <GrimSearch
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <GrimButton
          variant="ghost"
          className="w-full justify-start"
          onClick={() => setShowAdd(!showAdd)}
        >
          <Plus size={16} />
          Nuovo progetto
        </GrimButton>
      </div>

      {showAdd && (
        <>
          <GrimDivider />
          <AddProjectForm
            onSubmit={(name, path) => {
              onCreate(name, path);
              setShowAdd(false);
            }}
            onCancel={() => setShowAdd(false)}
          />
          <GrimDivider />
        </>
      )}

      <nav className="flex-1 overflow-y-auto px-[var(--spacing-2)] pb-[var(--spacing-2)]">
        {filtered.map((project) => {
          const active = project.ports.filter((p) => p.active).length;
          const isSelected = project.id === selectedId && activeView === "project";

          return (
            <button
              key={project.id}
              onClick={() => onSelect(project)}
              className={`
                w-full flex items-center justify-between
                px-[var(--spacing-2)] py-[var(--spacing-2)]
                rounded-[var(--radius-sm)]
                text-left cursor-pointer transition-colors duration-150
                ${isSelected
                  ? "bg-bg-elevated border border-accent-amber/30"
                  : "border border-transparent hover:bg-bg-elevated"
                }
              `}
            >
              <GrimText
                variant="section"
                className={`truncate text-[12px] ${isSelected ? "" : "text-text-secondary!"}`}
              >
                {project.name}
              </GrimText>
              {active > 0 && (
                <GrimBadge variant="active">{active}</GrimBadge>
              )}
            </button>
          );
        })}

        {unmanagedPorts.length > 0 && (
          <>
            <GrimDivider className="my-[var(--spacing-2)]" />
            <button
              onClick={onShowUnmanaged}
              className={`
                w-full flex items-center justify-between
                px-[var(--spacing-2)] py-[var(--spacing-2)]
                rounded-[var(--radius-sm)]
                text-left cursor-pointer transition-colors duration-150
                ${activeView === "unmanaged"
                  ? "bg-bg-elevated border border-accent-amber/30"
                  : "border border-transparent hover:bg-bg-elevated"
                }
              `}
            >
              <div className="flex items-center gap-[var(--spacing-1)]">
                <AlertTriangle size={12} className="text-accent-amber" />
                <GrimText
                  variant="body"
                  className={`text-[12px] ${activeView === "unmanaged" ? "text-accent-amber!" : "text-text-secondary!"}`}
                >
                  Non gestite
                </GrimText>
              </div>
              <GrimBadge variant="inactive">{unmanagedPorts.length}</GrimBadge>
            </button>
          </>
        )}
      </nav>

      <div className="flex flex-col gap-[var(--spacing-1)] p-[var(--spacing-2)]">
        {!mcpInstalled && (
          <GrimButton
            variant="primary"
            className="w-full justify-start text-[12px]!"
            onClick={onShowSettings}
          >
            <Plug size={14} />
            Connetti a Claude Code
          </GrimButton>
        )}
        <GrimButton
          variant="ghost"
          className={`w-full justify-start text-[12px]! ${activeView === "settings" ? "bg-bg-elevated" : ""}`}
          onClick={onShowSettings}
        >
          <Settings size={14} />
          Impostazioni
        </GrimButton>
      </div>
    </aside>
  );
}
