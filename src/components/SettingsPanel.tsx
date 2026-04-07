import { useState, useEffect } from "react";
import { GrimText } from "@/components/ui/GrimText";
import { GrimButton } from "@/components/ui/GrimButton";
import { GrimInput } from "@/components/ui/GrimInput";
import { GrimDivider } from "@/components/ui/GrimDivider";
import { GrimBadge } from "@/components/ui/GrimBadge";
import { GrimSelect } from "@/components/ui/GrimSelect";
import { CheckCircle, XCircle, Download, Upload, Trash2, Save, Copy, Check } from "lucide-react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import * as cmd from "@/lib/commands";

type McpClient = {
  value: string;
  label: string;
  configPath: string;
  rulesPath: string | null;
  format: "mcpServers" | "contextServers" | "toml";
};

const MCP_CLIENTS: McpClient[] = [
  { value: "cursor", label: "Cursor", configPath: "~/.cursor/mcp.json", rulesPath: ".cursorrules", format: "mcpServers" },
  { value: "windsurf", label: "Windsurf", configPath: "~/.codeium/windsurf/mcp_config.json", rulesPath: ".windsurfrules", format: "mcpServers" },
  { value: "vscode", label: "VS Code (Copilot)", configPath: ".vscode/mcp.json", rulesPath: null, format: "mcpServers" },
  { value: "claude-desktop", label: "Claude Desktop", configPath: "~/Library/Application Support/Claude/claude_desktop_config.json", rulesPath: null, format: "mcpServers" },
  { value: "continue", label: "Continue", configPath: "~/.continue/config.json", rulesPath: null, format: "mcpServers" },
  { value: "cline", label: "Cline (VS Code)", configPath: "~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json", rulesPath: null, format: "mcpServers" },
  { value: "codex", label: "Codex (OpenAI)", configPath: "~/.codex/config.toml", rulesPath: null, format: "toml" },
  { value: "zed", label: "Zed", configPath: "~/.config/zed/settings.json", rulesPath: null, format: "contextServers" },
];

function generateMcpConfig(client: McpClient, mcpDir: string): string {
  if (client.format === "toml") {
    return `[mcp_servers.grimport]
command = "uv"
args = ["--directory", "${mcpDir}", "run", "python", "server.py"]`;
  }

  const key = client.format === "contextServers" ? "context_servers" : "mcpServers";
  const obj = {
    [key]: {
      grimport: {
        command: "uv",
        args: ["--directory", mcpDir, "run", "python", "server.py"],
      },
    },
  };
  return JSON.stringify(obj, null, 2);
}

const SKILL_INSTRUCTIONS = `# Grimport - Port Allocation Manager

Gestisce l'allocazione delle porte tra progetti di sviluppo.
Usa quando devi assegnare porte a un nuovo progetto, registrare servizi,
o verificare quali porte sono in uso.

## Tool disponibili

### list_all
Mostra tutti i progetti registrati con range porte, servizi e stato attivo.

### reserve_range
Riserva il prossimo range di porte libero per un nuovo progetto.
- project_name: nome del progetto (es. "my-app")
- path: path opzionale alla directory del progetto

### register_port
Registra una porta specifica per un servizio dentro il range di un progetto.
- project_name: nome del progetto
- service: nome del servizio (es. "vite", "postgres", "redis")
- port: numero porta (deve essere nel range del progetto)

### release_project
Libera il range di porte di un progetto.

### scan_active
Scanna tutte le porte TCP attive sulla macchina.

## Workflow consigliato

Quando assegni porte a un nuovo progetto:
1. Chiama list_all per vedere i range occupati
2. Chiama reserve_range con il nome del progetto
3. Usa le porte del range assegnato nel docker-compose.yml e vite.config
4. Chiama register_port per ogni servizio configurato`;

function CodeBlock({ code, label }: { code: string; label: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-1)]">
      <div className="flex items-center justify-between">
        <GrimText variant="label">{label}</GrimText>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-[var(--spacing-1)] text-text-muted hover:text-text-primary transition-colors cursor-pointer"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
          <GrimText variant="mono" className="text-[10px]!">
            {copied ? "Copiato" : "Copia"}
          </GrimText>
        </button>
      </div>
      <pre className="bg-bg-deep border border-border-subtle rounded-[var(--radius-sm)] p-[var(--spacing-2)] overflow-x-auto">
        <GrimText variant="mono" className="text-[11px]! text-text-secondary whitespace-pre-wrap break-all">
          {code}
        </GrimText>
      </pre>
    </div>
  );
}

export function SettingsPanel() {
  const [mcpInstalled, setMcpInstalled] = useState<boolean | null>(null);
  const [installing, setInstalling] = useState(false);
  const [basePort, setBasePort] = useState("");
  const [rangeSize, setRangeSize] = useState("");
  const [configSaved, setConfigSaved] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [mcpDir, setMcpDir] = useState("");
  const [selectedClient, setSelectedClient] = useState("cursor");

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
    cmd.getMcpDir().then(setMcpDir).catch(() => {});
  }, []);

  const handleInstall = async () => {
    setInstalling(true);
    try {
      const mcpDir = await cmd.getMcpDir();
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
              rounded-full border border-transparent transition-colors duration-200 cursor-pointer
              ${autostart ? "bg-accent-amber" : "bg-status-inactive"}
              focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber focus-visible:ring-offset-2 focus-visible:ring-offset-bg-deep
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
          Integrazione MCP
        </GrimText>

        {/* Claude Code - Auto Install */}
        <div className="flex flex-col gap-[var(--spacing-2)]">
          <GrimText variant="label">Claude Code</GrimText>
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

        {/* Other Editors - Manual Config */}
        <div className="flex flex-col gap-[var(--spacing-2)]">
          <GrimText variant="label">Altri editor</GrimText>

          <GrimSelect
            label="Seleziona editor"
            options={MCP_CLIENTS.map((c) => ({ value: c.value, label: c.label }))}
            value={selectedClient}
            onChange={setSelectedClient}
          />

          {(() => {
            const client = MCP_CLIENTS.find((c) => c.value === selectedClient);
            if (!client) return null;
            const configCode = generateMcpConfig(client, mcpDir);
            const configLabel = client.format === "toml" ? "Config MCP (TOML)" : "Config MCP";

            return (
              <div className="flex flex-col gap-[var(--spacing-3)]">
                <CodeBlock code={configCode} label={configLabel} />

                <GrimText variant="body" className="text-text-secondary">
                  Incolla il config in <GrimText variant="mono" className="text-[11px]!">{client.configPath}</GrimText>
                </GrimText>

                {client.rulesPath && (
                  <CodeBlock code={SKILL_INSTRUCTIONS} label={`Istruzioni per ${client.label}`} />
                )}

                {client.rulesPath && (
                  <GrimText variant="body" className="text-text-secondary">
                    Incolla le istruzioni nel file <GrimText variant="mono" className="text-[11px]!">{client.rulesPath}</GrimText> nella root del tuo progetto.
                  </GrimText>
                )}

                <GrimText variant="body" className="text-text-muted text-[11px]!">
                  L'app Grimport deve essere in esecuzione per usare i tool MCP.
                </GrimText>
              </div>
            );
          })()}
        </div>
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
