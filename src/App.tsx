import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppHeader } from "@/components/AppHeader";
import { Sidebar } from "@/components/Sidebar";
import { ProjectDetail } from "@/components/ProjectDetail";
import { SettingsPanel } from "@/components/SettingsPanel";
import { UnmanagedPortsPanel } from "@/components/UnmanagedPortsPanel";
import { PopoverPanel } from "@/components/PopoverPanel";
import { GrimText } from "@/components/ui/GrimText";
import { GrimToast } from "@/components/ui/GrimToast";
import { useProjects } from "@/features/projects/useProjects";
import type { ProjectStatus } from "@/lib/types";

type View = "project" | "unmanaged" | "settings";

function MainWindow() {
  const {
    projects,
    unmanagedPorts,
    loading,
    error,
    clearError,
    create,
    remove,
    addPort,
    removePort,
  } = useProjects();
  const [selected, setSelected] = useState<ProjectStatus | null>(null);
  const [activeView, setActiveView] = useState<View>("project");

  const currentProject = selected
    ? projects.find((p) => p.id === selected.id) ?? null
    : null;

  const handleSelectProject = (project: ProjectStatus) => {
    setSelected(project);
    setActiveView("project");
  };

  const handleShowSettings = () => {
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
        />

        <main className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center h-full">
              <GrimText variant="body" className="text-text-muted">
                Loading...
              </GrimText>
            </div>
          ) : activeView === "settings" ? (
            <SettingsPanel />
          ) : activeView === "unmanaged" ? (
            <UnmanagedPortsPanel ports={unmanagedPorts} />
          ) : currentProject ? (
            <ProjectDetail
              project={currentProject}
              onDelete={(id) => {
                remove(id);
                setSelected(null);
              }}
              onAddPort={addPort}
              onRemovePort={removePort}
            />
          ) : (
            <div className="flex items-center justify-center h-full">
              <GrimText variant="body" className="text-text-muted">
                Select a project from the sidebar
              </GrimText>
            </div>
          )}
        </main>
      </div>

      <GrimToast message={error} onDismiss={clearError} />
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

  return <MainWindow />;
}

export default App;
