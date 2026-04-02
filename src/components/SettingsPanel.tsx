import { useState, useEffect } from "react";
import { GrimText } from "@/components/ui/GrimText";
import { GrimButton } from "@/components/ui/GrimButton";
import { GrimInput } from "@/components/ui/GrimInput";
import { GrimDivider } from "@/components/ui/GrimDivider";
import { GrimBadge } from "@/components/ui/GrimBadge";
import { CheckCircle, XCircle, Download, Upload, Trash2, Save } from "lucide-react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import * as cmd from "@/lib/commands";

export function SettingsPanel() {
  const [mcpInstalled, setMcpInstalled] = useState<boolean | null>(null);
  const [installing, setInstalling] = useState(false);
  const [basePort, setBasePort] = useState("");
  const [rangeSize, setRangeSize] = useState("");
  const [configSaved, setConfigSaved] = useState(false);
  const [autostart, setAutostart] = useState(false);

  const checkMcp = async () => {
    try {
      const installed = await cmd.checkMcpInstalled();
      setMcpInstalled(installed);
    } catch {
      setMcpInstalled(false);
    }
  };

  const loadConfig = async () => {
    try {
      const config = await cmd.getConfig();
      setBasePort(config.base_port);
      setRangeSize(config.range_size);
    } catch (err) {
      console.error("Failed to load config:", err);
    }
  };

  useEffect(() => {
    checkMcp();
    loadConfig();
    isEnabled().then(setAutostart).catch(() => {});
  }, []);

  const handleInstall = async () => {
    setInstalling(true);
    try {
      const mcpDir = await getMcpDir();
      await cmd.installMcp(mcpDir);
      await new Promise((r) => setTimeout(r, 1500));
      setMcpInstalled(true);
    } catch (err) {
      console.error("Failed to install MCP:", err);
      await checkMcp();
    } finally {
      setInstalling(false);
    }
  };

  const handleUninstall = async () => {
    try {
      await cmd.uninstallMcp();
      setMcpInstalled(false);
    } catch (err) {
      console.error("Failed to uninstall MCP:", err);
      await checkMcp();
    }
  };

  const handleToggleAutostart = async () => {
    try {
      if (autostart) {
        await disable();
      } else {
        await enable();
      }
      setAutostart(!autostart);
    } catch (err) {
      console.error("Failed to toggle autostart:", err);
    }
  };

  const handleExport = async () => {
    try {
      const path = await save({
        defaultPath: "grimport-backup.grimport",
        filters: [{ name: "Grimport", extensions: ["grimport"] }],
      });
      if (path) {
        await cmd.exportData(path);
      }
    } catch (err) {
      console.error("Failed to export:", err);
    }
  };

  const handleImport = async () => {
    try {
      const path = await open({
        filters: [{ name: "Grimport", extensions: ["grimport"] }],
        multiple: false,
      });
      if (path) {
        await cmd.importData(path);
        await loadConfig();
      }
    } catch (err) {
      console.error("Failed to import:", err);
    }
  };

  const handleSaveConfig = async () => {
    try {
      await cmd.setConfig("base_port", basePort);
      await cmd.setConfig("range_size", rangeSize);
      setConfigSaved(true);
      setTimeout(() => setConfigSaved(false), 2000);
    } catch (err) {
      console.error("Failed to save config:", err);
    }
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-6)] p-[var(--spacing-5)]">
      <GrimText variant="title" as="h2">
        Impostazioni
      </GrimText>

      <GrimDivider />

      {/* Port Config Section */}
      <div className="flex flex-col gap-[var(--spacing-3)]">
        <GrimText variant="section" as="h3">
          Configurazione porte
        </GrimText>

        <label className="inline-flex items-center gap-[var(--spacing-2)] cursor-pointer w-fit">
          <button
            type="button"
            role="switch"
            aria-checked={autostart}
            onClick={handleToggleAutostart}
            className={`
              relative inline-flex h-[18px] w-[32px] shrink-0 items-center
              rounded-full transition-colors duration-200 cursor-pointer
              ${autostart ? "bg-accent-amber" : "bg-status-inactive"}
            `}
          >
            <span
              className={`
                inline-block h-[14px] w-[14px] rounded-full bg-white
                transition-transform duration-200 shadow-sm
                ${autostart ? "translate-x-[16px]" : "translate-x-[2px]"}
              `}
            />
          </button>
          <GrimText variant="body">Avvio automatico al login</GrimText>
        </label>

        <GrimDivider />

        <GrimText variant="section" as="h3">
          Range porte
        </GrimText>

        <GrimText variant="body" className="text-text-secondary">
          Modifica influenza solo i nuovi progetti. I range gia' assegnati non cambiano.
        </GrimText>

        <div className="flex items-end gap-[var(--spacing-3)]">
          <GrimInput
            label="Porta base"
            type="number"
            value={basePort}
            onChange={(e) => setBasePort(e.target.value)}
            wrapperClassName="w-32"
          />
          <GrimInput
            label="Dimensione range"
            type="number"
            value={rangeSize}
            onChange={(e) => setRangeSize(e.target.value)}
            wrapperClassName="w-32"
          />
          <GrimButton variant="primary" onClick={handleSaveConfig}>
            <Save size={14} />
            {configSaved ? "Salvato" : "Salva"}
          </GrimButton>
        </div>
      </div>

      <GrimDivider />

      {/* MCP Section */}
      <div className="flex flex-col gap-[var(--spacing-3)]">
        <GrimText variant="section" as="h3">
          Integrazione Claude Code
        </GrimText>

        <div className="flex items-center gap-[var(--spacing-3)]">
          {mcpInstalled === null ? (
            <GrimText variant="body" className="text-text-muted">
              Verifica in corso...
            </GrimText>
          ) : mcpInstalled ? (
            <>
              <GrimBadge variant="active">
                <CheckCircle size={12} />
                Connesso
              </GrimBadge>
              <GrimButton variant="danger" onClick={handleUninstall}>
                <Trash2 size={14} />
                Rimuovi
              </GrimButton>
            </>
          ) : (
            <>
              <GrimBadge variant="inactive">
                <XCircle size={12} />
                Non connesso
              </GrimBadge>
              <GrimButton
                variant="primary"
                onClick={handleInstall}
                disabled={installing}
              >
                <Download size={14} />
                {installing ? "Connessione..." : "Connetti"}
              </GrimButton>
            </>
          )}
        </div>

        <GrimText variant="body" className="text-text-secondary">
          {mcpInstalled
            ? "Claude Code puo' gestire le porte dei tuoi progetti automaticamente. Riavvia Claude Code se hai appena connesso."
            : "Connetti a Claude Code per riservare porte e registrare servizi automaticamente."}
        </GrimText>

        {mcpInstalled && (
          <div className="flex flex-col gap-[var(--spacing-1)] bg-bg-surface rounded-[var(--radius-md)] p-[var(--spacing-3)]">
            <GrimText variant="label">Tool disponibili</GrimText>
            <div className="flex flex-wrap gap-[var(--spacing-1)]">
              {["list_all", "reserve_range", "register_port", "release_project", "scan_active"].map(
                (tool) => (
                  <GrimBadge key={tool} variant="inactive">
                    <GrimText variant="mono" className="text-[10px]!">
                      {tool}
                    </GrimText>
                  </GrimBadge>
                ),
              )}
            </div>
          </div>
        )}
      </div>

      <GrimDivider />

      {/* Export/Import Section */}
      <div className="flex flex-col gap-[var(--spacing-3)]">
        <GrimText variant="section" as="h3">
          Dati
        </GrimText>

        <GrimText variant="body" className="text-text-secondary">
          Esporta o importa il database e le preferenze per backup o migrazione.
        </GrimText>

        <div className="flex gap-[var(--spacing-2)]">
          <GrimButton variant="ghost" onClick={handleExport}>
            <Download size={14} />
            Esporta
          </GrimButton>
          <GrimButton variant="ghost" onClick={handleImport}>
            <Upload size={14} />
            Importa
          </GrimButton>
        </div>
      </div>
    </div>
  );
}

async function getMcpDir(): Promise<string> {
  try {
    const { resourceDir } = await import("@tauri-apps/api/path");
    const base = await resourceDir();
    return base + "mcp";
  } catch {
    return "/Users/doppia/Development/Projects/grimport/mcp";
  }
}
