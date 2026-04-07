import { useState, useEffect } from "react";
import { Plus, Settings, Plug, AlertTriangle } from "lucide-react";
import { UISearch } from "@/components/ui/UISearch";
import { UIButton } from "@/components/ui/UIButton";
import { UIText } from "@/components/ui/UIText";
import { UIBadge } from "@/components/ui/UIBadge";
import { UIDivider } from "@/components/ui/UIDivider";
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
        <UISearch
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <UIButton
          variant="ghost"
          className="w-full justify-start"
          onClick={() => setShowAdd(!showAdd)}
        >
          <Plus size={16} />
          New project
        </UIButton>
      </div>

      {showAdd && (
        <>
          <UIDivider />
          <AddProjectForm
            onSubmit={(name, path) => {
              onCreate(name, path);
              setShowAdd(false);
            }}
            onCancel={() => setShowAdd(false)}
          />
          <UIDivider />
        </>
      )}

      <nav className="flex-1 overflow-y-auto px-[var(--spacing-2)] pt-[var(--spacing-2)] pb-[var(--spacing-2)]">
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
                focus-visible:outline-none focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
                ${isSelected
                  ? "bg-bg-elevated border border-accent-amber/30"
                  : "border border-transparent hover:bg-bg-elevated"
                }
              `}
            >
              <UIText
                variant="section"
                className={`truncate text-[12px] ${isSelected ? "" : "text-text-secondary!"}`}
              >
                {project.name}
              </UIText>
              {active > 0 && (
                <UIBadge variant="active">{active}</UIBadge>
              )}
            </button>
          );
        })}

        {unmanagedPorts.length > 0 && (
          <>
            <UIDivider className="my-[var(--spacing-2)]" />
            <button
              onClick={onShowUnmanaged}
              className={`
                w-full flex items-center justify-between
                px-[var(--spacing-2)] py-[var(--spacing-2)]
                rounded-[var(--radius-sm)]
                text-left cursor-pointer transition-colors duration-150
                focus-visible:outline-none focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
                ${activeView === "unmanaged"
                  ? "bg-bg-elevated border border-accent-amber/30"
                  : "border border-transparent hover:bg-bg-elevated"
                }
              `}
            >
              <div className="flex items-center gap-[var(--spacing-1)]">
                <AlertTriangle size={12} className="text-accent-amber" />
                <UIText
                  variant="body"
                  className={`text-[12px] ${activeView === "unmanaged" ? "text-accent-amber!" : "text-text-secondary!"}`}
                >
                  Unmanaged
                </UIText>
              </div>
              <UIBadge variant="inactive">{unmanagedPorts.length}</UIBadge>
            </button>
          </>
        )}
      </nav>

      <div className="flex flex-col gap-[var(--spacing-1)] p-[var(--spacing-2)]">
        {!mcpInstalled && (
          <UIButton
            variant="primary"
            className="w-full justify-start text-[12px]!"
            onClick={onShowSettings}
          >
            <Plug size={14} />
            Configure MCP
          </UIButton>
        )}
        <UIButton
          variant="ghost"
          className={`w-full justify-start text-[12px]! ${activeView === "settings" ? "bg-bg-elevated" : ""}`}
          onClick={onShowSettings}
        >
          <Settings size={14} />
          Settings
        </UIButton>
      </div>
    </aside>
  );
}
