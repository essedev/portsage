import { useState, useEffect } from "react";
import {
  Plus,
  Settings,
  Plug,
  AlertTriangle,
  Trash2,
  ChevronRight,
  ChevronDown,
  Sparkles,
} from "lucide-react";
import { UISearch } from "@/components/ui/UISearch";
import { UIButton } from "@/components/ui/UIButton";
import { UIText } from "@/components/ui/UIText";
import { UIBadge } from "@/components/ui/UIBadge";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIDivider } from "@/components/ui/UIDivider";
import { AddProjectForm } from "@/components/AddProjectForm";
import { BackendSwitcher } from "@/components/BackendSwitcher";
import * as cmd from "@/lib/commands";
import type {
  BackendTarget,
  ProjectStatus,
  RemoteBackend,
  TunnelState,
  UnmanagedPort,
} from "@/lib/types";

type View = "project" | "unmanaged" | "settings" | "trash" | "prune";

type SettingsTab = "general" | "integrations" | "data" | "backends";

interface SidebarProps {
  projects: ProjectStatus[];
  unmanagedPorts: UnmanagedPort[];
  selectedId?: number;
  activeView: View;
  onSelect: (project: ProjectStatus) => void;
  onCreate: (name: string, path?: string) => void;
  onShowSettings: (tab?: SettingsTab) => void;
  onShowUnmanaged: () => void;
  onShowTrash: () => void;
  onShowPrune: () => void;
  /** Entries currently in the trash. The row hides itself when this is 0. */
  trashCount: number;
  /** Projects that look abandoned, for the prune row. */
  staleCount: number;
  backendTarget: BackendTarget | null;
  remoteBackends: RemoteBackend[];
  tunnels: Record<string, TunnelState>;
  onSelectBackend: (target: BackendTarget) => void;
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
  onShowTrash,
  onShowPrune,
  trashCount,
  staleCount,
  backendTarget,
  remoteBackends,
  tunnels,
  onSelectBackend,
}: SidebarProps) {
  const [search, setSearch] = useState("");
  const [showAdd, setShowAdd] = useState(false);
  const [mcpInstalled, setMcpInstalled] = useState(true);
  const [showArchived, setShowArchived] = useState(false);
  const [activeOnly, setActiveOnly] = useState(false);

  useEffect(() => {
    cmd.checkMcpInstalled().then(setMcpInstalled).catch(() => setMcpInstalled(false));
  }, [activeView]);

  const matches = (p: ProjectStatus) =>
    p.name.toLowerCase().includes(search.toLowerCase());
  const hasActivePort = (p: ProjectStatus) => p.ports.some((port) => port.active);
  // Alphabetical, not by range: the range order is registration order, which
  // is arbitrary to the eye. Both are stable (a project stays put), but only
  // one lets you guess where a name will be. Sorting by "active first" was
  // rejected on purpose - rows would jump every time a server starts.
  const byName = (a: ProjectStatus, b: ProjectStatus) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  // Archived projects live in their own collapsed section rather than in
  // Settings: out of the way, but still visible enough that nobody forgets
  // their names and ranges are still taken.
  const filtered = projects
    .filter((p) => !p.archived_at && matches(p) && (!activeOnly || hasActivePort(p)))
    .sort(byName);
  const archived = projects.filter((p) => p.archived_at && matches(p)).sort(byName);
  const activeCount = projects.filter((p) => !p.archived_at && hasActivePort(p)).length;

  return (
    <aside
      className="
        w-60 h-full flex flex-col
        bg-bg-surface border-r border-border-subtle
      "
    >
      <div className="p-[var(--spacing-3)] flex flex-col gap-[var(--spacing-2)]">
        <BackendSwitcher
          target={backendTarget}
          remotes={remoteBackends}
          tunnels={tunnels}
          onSelectTarget={onSelectBackend}
          onAddBackend={() => onShowSettings("backends")}
        />
        <UISearch
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        {/* One filter, not a menu: with a search box already there, the only
            question worth a control is "what is running right now". */}
        {activeCount > 0 && (
          <button
            onClick={() => setActiveOnly(!activeOnly)}
            aria-pressed={activeOnly}
            className={`
              flex items-center justify-between
              px-[var(--spacing-2)] py-[var(--spacing-1)]
              rounded-[var(--radius-sm)] cursor-pointer
              border transition-colors duration-150
              focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
              ${activeOnly
                ? "border-accent-amber/30 bg-bg-elevated"
                : "border-transparent hover:bg-bg-elevated"
              }
            `}
          >
            <div className="flex items-center gap-[var(--spacing-1)]">
              <UIStatus active={true} />
              <UIText
                variant="body"
                className={`text-[11px] ${activeOnly ? "text-accent-amber!" : "text-text-secondary!"}`}
              >
                Active only
              </UIText>
            </div>
            <UIBadge variant={activeOnly ? "active" : "inactive"}>{activeCount}</UIBadge>
          </button>
        )}
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
        {filtered.length === 0 && projects.length > 0 && (
          <UIText
            variant="body"
            className="text-text-muted text-[12px]! px-[var(--spacing-2)] py-[var(--spacing-2)] block"
          >
            No matches
          </UIText>
        )}
        {filtered.map((project) => {
          const active = project.ports.filter((p) => p.active).length;
          const hasActive = active > 0;
          const isSelected = project.id === selectedId && activeView === "project";

          // Visual hierarchy:
          // - active project: status dot ambra + name in text-primary (or amber if selected)
          // - inactive project: no dot, name in text-secondary
          // Selected state adds the elevated bg + amber-tinted border on top of the above.
          return (
            <button
              key={project.id}
              onClick={() => onSelect(project)}
              aria-current={isSelected ? "page" : undefined}
              className={`
                w-full flex items-center justify-between gap-[var(--spacing-2)]
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
              <div className="flex items-center gap-[var(--spacing-2)] min-w-0 flex-1">
                <div className="w-2 shrink-0 flex justify-center">
                  {hasActive && <UIStatus active={true} />}
                </div>
                <UIText
                  variant="section"
                  className={`
                    truncate text-[12px]
                    ${isSelected
                      ? ""
                      : hasActive
                        ? "text-text-primary!"
                        : "text-text-secondary!"
                    }
                  `}
                >
                  {project.name}
                </UIText>
              </div>
              {hasActive && (
                <UIBadge variant="active">{active}</UIBadge>
              )}
            </button>
          );
        })}

        {archived.length > 0 && (
          <>
            <UIDivider className="my-[var(--spacing-2)]" />
            <button
              onClick={() => setShowArchived(!showArchived)}
              aria-expanded={showArchived}
              className="
                w-full flex items-center justify-between
                px-[var(--spacing-2)] py-[var(--spacing-2)]
                rounded-[var(--radius-sm)]
                text-left cursor-pointer transition-colors duration-150
                border border-transparent hover:bg-bg-elevated
                focus-visible:outline-none focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
              "
            >
              <div className="flex items-center gap-[var(--spacing-1)]">
                {showArchived ? (
                  <ChevronDown size={12} className="text-text-muted" />
                ) : (
                  <ChevronRight size={12} className="text-text-muted" />
                )}
                <UIText variant="body" className="text-[12px] text-text-secondary!">
                  Archived
                </UIText>
              </div>
              <UIBadge variant="inactive">{archived.length}</UIBadge>
            </button>
            {showArchived &&
              archived.map((project) => {
                const isSelected = project.id === selectedId && activeView === "project";
                return (
                  <button
                    key={project.id}
                    onClick={() => onSelect(project)}
                    aria-current={isSelected ? "page" : undefined}
                    className={`
                      w-full flex items-center gap-[var(--spacing-2)]
                      pl-[var(--spacing-4)] pr-[var(--spacing-2)] py-[var(--spacing-2)]
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
                      className="truncate text-[12px] text-text-muted!"
                    >
                      {project.name}
                    </UIText>
                  </button>
                );
              })}
          </>
        )}

      </nav>

      <div className="flex flex-col gap-[var(--spacing-1)] p-[var(--spacing-2)]">
        {/* System views are pinned next to Settings rather than living in
            the scrollable project list: with 27 projects they used to sit
            below the fold. Each row still appears only when it has something
            to show. */}
        {(staleCount > 0 || unmanagedPorts.length > 0 || trashCount > 0) && (
          <UIDivider className="mb-[var(--spacing-1)]" />
        )}
        {unmanagedPorts.length > 0 && (
          <SidebarSystemRow
            icon={<AlertTriangle size={14} className="text-accent-amber" />}
            label="Unmanaged"
            count={unmanagedPorts.length}
            selected={activeView === "unmanaged"}
            onClick={onShowUnmanaged}
          />
        )}
        {staleCount > 0 && (
          <SidebarSystemRow
            icon={<Sparkles size={14} className="text-text-muted" />}
            label="Prune"
            count={staleCount}
            selected={activeView === "prune"}
            onClick={onShowPrune}
          />
        )}
        {trashCount > 0 && (
          <SidebarSystemRow
            icon={<Trash2 size={14} className="text-text-muted" />}
            label="Trash"
            count={trashCount}
            selected={activeView === "trash"}
            onClick={onShowTrash}
          />
        )}
        {!mcpInstalled && (
          <UIButton
            variant="primary"
            className="w-full justify-start text-[12px]!"
            onClick={() => onShowSettings("integrations")}
          >
            <Plug size={14} aria-hidden="true" />
            Configure MCP
          </UIButton>
        )}
        <UIButton
          variant="ghost"
          className={`w-full justify-start text-[12px]! ${activeView === "settings" ? "bg-bg-elevated" : ""}`}
          onClick={() => onShowSettings()}
        >
          <Settings size={14} aria-hidden="true" />
          Settings
        </UIButton>
      </div>
    </aside>
  );
}

/**
 * One pinned system view in the sidebar footer (Unmanaged, Prune, Trash).
 * Same shape as a project row so the two read as one list, but it lives
 * outside the scroll area and carries its own count.
 */
function SidebarSystemRow({
  icon,
  label,
  count,
  selected,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  count: number;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      aria-current={selected ? "page" : undefined}
      className={`
        w-full flex items-center justify-between
        px-[var(--spacing-2)] py-[var(--spacing-2)]
        rounded-[var(--radius-sm)]
        text-left cursor-pointer transition-colors duration-150
        focus-visible:outline-none focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
        ${selected
          ? "bg-bg-elevated border border-accent-amber/30"
          : "border border-transparent hover:bg-bg-elevated"
        }
      `}
    >
      <div className="flex items-center gap-[var(--spacing-2)]">
        {icon}
        <UIText
          variant="body"
          className={`text-[12px] ${selected ? "text-accent-amber!" : "text-text-secondary!"}`}
        >
          {label}
        </UIText>
      </div>
      <UIBadge variant="inactive">{count}</UIBadge>
    </button>
  );
}
