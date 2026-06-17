import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppHeader } from "@/components/AppHeader";
import { Sidebar } from "@/components/Sidebar";
import { ProjectDetail } from "@/components/ProjectDetail";
import { SettingsPanel } from "@/components/SettingsPanel";
import { UnmanagedPortsPanel } from "@/components/UnmanagedPortsPanel";
import { PopoverPanel } from "@/components/PopoverPanel";
import { WelcomePanel } from "@/components/WelcomePanel";
import { UIText } from "@/components/ui/UIText";
import { ToastProvider } from "@/lib/toast";
import { DialogProvider } from "@/lib/dialog";
import { useProjects } from "@/features/projects/useProjects";
import { useBackends } from "@/features/backends/useBackends";
import type { ProjectStatus } from "@/lib/types";

type View = "project" | "unmanaged" | "settings";
type SettingsTab = "general" | "integrations" | "data" | "backends";

function MainWindow() {
  const {
    projects,
    unmanagedPorts,
    loading,
    refresh: refreshProjects,
    create,
    remove,
    update,
    addPort,
    removePort,
    killPort,
    killProject,
  } = useProjects();
  const {
    target: backendTarget,
    remotes: remoteBackends,
    tunnels,
    setTarget: selectBackend,
    refresh: refreshBackends,
  } = useBackends();
  const [selected, setSelected] = useState<ProjectStatus | null>(null);
  const [activeView, setActiveView] = useState<View>("project");
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("general");

  const currentProject = selected
    ? projects.find((p) => p.id === selected.id) ?? null
    : null;

  const handleSelectProject = (project: ProjectStatus) => {
    setSelected(project);
    setActiveView("project");
  };

  const handleShowSettings = (tab?: SettingsTab) => {
    if (tab) setSettingsTab(tab);
    setActiveView("settings");
  };

  const handleShowUnmanaged = () => {
    setActiveView("unmanaged");
    setSelected(null);
  };

  return (
    <div className="flex flex-col h-screen bg-bg-deep">
      <AppHeader projects={projects} />

      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          projects={projects}
          unmanagedPorts={unmanagedPorts}
          selectedId={currentProject?.id}
          activeView={activeView}
          onSelect={handleSelectProject}
          onCreate={create}
          onShowSettings={handleShowSettings}
          onShowUnmanaged={handleShowUnmanaged}
          backendTarget={backendTarget}
          remoteBackends={remoteBackends}
          tunnels={tunnels}
          onSelectBackend={(t) => {
            // Switch target, drop any project selected on the previous
            // backend (its id is meaningless on the new one), and refresh
            // so the user sees the new backend's project list immediately
            // rather than waiting for the next polling tick.
            void selectBackend(t).then((ok) => {
              if (ok) {
                setSelected(null);
                void refreshProjects();
              }
            });
          }}
        />

        <main className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center h-full">
              <UIText variant="body" className="text-text-muted">
                Loading...
              </UIText>
            </div>
          ) : activeView === "settings" ? (
            <SettingsPanel
              tab={settingsTab}
              onTabChange={setSettingsTab}
              remoteBackends={remoteBackends}
              tunnels={tunnels}
              backendTarget={backendTarget}
              onBackendsChanged={() => {
                void refreshBackends();
              }}
            />
          ) : activeView === "unmanaged" ? (
            <UnmanagedPortsPanel ports={unmanagedPorts} onKill={killPort} />
          ) : currentProject ? (
            <ProjectDetail
              project={currentProject}
              onDelete={(name) => {
                remove(name);
                setSelected(null);
              }}
              onUpdate={update}
              onAddPort={addPort}
              onRemovePort={removePort}
              onKillPort={killPort}
              onKillProject={killProject}
              backendTarget={backendTarget}
            />
          ) : (
            <WelcomePanel
              projects={projects}
              unmanagedPorts={unmanagedPorts}
              onCreate={create}
              onShowSettings={handleShowSettings}
              onShowUnmanaged={handleShowUnmanaged}
            />
          )}
        </main>
      </div>
    </div>
  );
}

function App() {
  const [windowLabel, setWindowLabel] = useState<string | null>(null);

  useEffect(() => {
    const label = getCurrentWindow().label;
    setWindowLabel(label);
  }, []);

  if (!windowLabel) return null;

  if (windowLabel === "popover") {
    return <PopoverPanel />;
  }

  return (
    <ToastProvider>
      <DialogProvider>
        <MainWindow />
      </DialogProvider>
    </ToastProvider>
  );
}

export default App;
