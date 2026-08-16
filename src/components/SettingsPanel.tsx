import { useState, useEffect } from "react";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIInput } from "@/components/ui/UIInput";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIBadge } from "@/components/ui/UIBadge";
import { UISelect } from "@/components/ui/UISelect";
import { UITabs, UITabPanel } from "@/components/ui/UITabs";
import { RemoteBackendsPanel } from "@/components/RemoteBackendsPanel";
import type {
  BackendTarget,
  RemoteBackend,
  TunnelState,
} from "@/lib/types";
import {
  CheckCircle,
  XCircle,
  Download,
  Upload,
  Trash2,
  Save,
  Copy,
  Check,
  ChevronRight,
} from "lucide-react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { useConfirm } from "@/lib/dialog";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { getVersion } from "@tauri-apps/api/app";
import * as cmd from "@/lib/commands";
import { useToast } from "@/lib/toast";
import { humanizeError } from "@/lib/errors";
import skillMd from "../../mcp/SKILL.md?raw";

type McpClient = {
  value: string;
  label: string;
  configPath: string;
  rulesPath: string | null;
  format: "mcpServers" | "vscodeServers" | "toml";
  rulesFormat?: "cursor" | "windsurf";
};

const MCP_CLIENTS: McpClient[] = [
  { value: "cursor", label: "Cursor", configPath: "~/.cursor/mcp.json", rulesPath: ".cursor/rules/portsage.mdc", rulesFormat: "cursor", format: "mcpServers" },
  { value: "claude-desktop", label: "Claude Desktop", configPath: "~/Library/Application Support/Claude/claude_desktop_config.json", rulesPath: null, format: "mcpServers" },
  { value: "cline", label: "Cline (VS Code)", configPath: "~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json", rulesPath: null, format: "mcpServers" },
  { value: "vscode", label: "VS Code (Copilot)", configPath: ".vscode/mcp.json", rulesPath: null, format: "vscodeServers" },
  { value: "codex", label: "Codex (OpenAI)", configPath: "~/.codex/config.toml", rulesPath: null, format: "toml" },
  { value: "windsurf", label: "Windsurf", configPath: "~/.codeium/windsurf/mcp_config.json", rulesPath: ".windsurf/rules/portsage.md", rulesFormat: "windsurf", format: "mcpServers" },
];

function generateMcpConfig(client: McpClient, mcpDir: string): string {
  if (client.format === "toml") {
    return `[mcp_servers.portsage]
command = "uv"
args = ["--directory", "${mcpDir}", "run", "python", "server.py"]`;
  }

  const key = client.format === "vscodeServers" ? "servers" : "mcpServers";
  const obj = {
    [key]: {
      portsage: {
        command: "uv",
        args: ["--directory", mcpDir, "run", "python", "server.py"],
      },
    },
  };
  return JSON.stringify(obj, null, 2);
}

const SKILL_BODY = skillMd.replace(/^---\n[\s\S]*?\n---\n+/, "");

const SKILL_DESCRIPTION =
  "Manages port allocation across development projects. Use it when you need to assign ports to a new project, register services, or check which ports are in use.";

function generateRulesContent(client: McpClient): string {
  if (client.rulesFormat === "cursor") {
    return `---
description: ${SKILL_DESCRIPTION}
alwaysApply: false
---

${SKILL_BODY}`;
  }
  if (client.rulesFormat === "windsurf") {
    return `---
trigger: model_decision
description: ${SKILL_DESCRIPTION}
---

${SKILL_BODY}`;
  }
  return SKILL_BODY;
}

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
        <UIText variant="label">{label}</UIText>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-[var(--spacing-1)] text-text-muted hover:text-text-primary transition-colors cursor-pointer"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
          <UIText variant="mono" className="text-[10px]!">
            {copied ? "Copied" : "Copy"}
          </UIText>
        </button>
      </div>
      <pre className="bg-bg-deep border border-border-subtle rounded-[var(--radius-sm)] p-[var(--spacing-2)] overflow-x-auto">
        <UIText variant="mono" className="text-[11px]! text-text-secondary whitespace-pre-wrap break-all">
          {code}
        </UIText>
      </pre>
    </div>
  );
}

export type SettingsTab = "general" | "integrations" | "data" | "backends";

interface SettingsPanelProps {
  // Optional controlled tab state, used for deep-linking from other panels
  // (e.g. WelcomePanel jumping to "integrations"). Falls back to internal
  // state when not provided.
  tab?: SettingsTab;
  onTabChange?: (tab: SettingsTab) => void;
  // Multi-host (Phase 2). Threaded through from App's useBackends so the
  // sidebar switcher and this panel share a single state store.
  remoteBackends?: RemoteBackend[];
  tunnels?: Record<string, TunnelState>;
  backendTarget?: BackendTarget | null;
  onBackendsChanged?: () => void;
}

export function SettingsPanel({
  tab: controlledTab,
  onTabChange,
  remoteBackends = [],
  tunnels = {},
  backendTarget = null,
  onBackendsChanged,
}: SettingsPanelProps = {}) {
  const { showSuccess, showError } = useToast();
  const confirm = useConfirm();
  const [internalTab, setInternalTab] = useState<SettingsTab>("general");
  const tab = controlledTab ?? internalTab;
  const setTab = (next: SettingsTab) => {
    setInternalTab(next);
    onTabChange?.(next);
  };
  const [mcpInstalled, setMcpInstalled] = useState<boolean | null>(null);
  const [installing, setInstalling] = useState(false);
  const [basePort, setBasePort] = useState("");
  const [rangeSize, setRangeSize] = useState("");
  const [configSaved, setConfigSaved] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [mcpDir, setMcpDir] = useState("");
  const [selectedClient, setSelectedClient] = useState("cursor");
  const [otherEditorsOpen, setOtherEditorsOpen] = useState(false);
  const [version, setVersion] = useState("");

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
    getVersion().then(setVersion).catch(() => {});
  }, []);

  const handleInstall = async () => {
    setInstalling(true);
    try {
      const mcpDir = await cmd.getMcpDir();
      await cmd.installMcp(mcpDir);
      await new Promise((r) => setTimeout(r, 1500));
      setMcpInstalled(true);
      showSuccess("Connected to Claude Code. Restart Claude Code to load the new tools.");
    } catch (err) {
      showError(humanizeError(err));
      await checkMcp();
    } finally {
      setInstalling(false);
    }
  };

  const handleUninstall = async () => {
    const ok = await confirm({
      title: "Disconnect from Claude Code",
      message:
        "Disconnect Portsage from Claude Code? This removes the MCP server config, the skill file, and the tool permissions. You can reconnect at any time.",
      kind: "warning",
      okLabel: "Disconnect",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    try {
      await cmd.uninstallMcp();
      setMcpInstalled(false);
      showSuccess("Disconnected from Claude Code.");
    } catch (err) {
      showError(humanizeError(err));
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
        defaultPath: "portsage-backup.portsage",
        filters: [{ name: "Portsage", extensions: ["portsage"] }],
      });
      if (path) {
        await cmd.exportData(path);
        showSuccess("Backup exported successfully.");
      }
    } catch (err) {
      showError(humanizeError(err));
    }
  };

  const handleImport = async () => {
    try {
      const path = await open({
        filters: [{ name: "Portsage", extensions: ["portsage"] }],
        multiple: false,
      });
      if (!path) return;
      const ok = await confirm({
        title: "Replace current data",
        message:
          "Importing this file will REPLACE your current Portsage database, including all projects, ports, and settings. This cannot be undone. Continue?",
        kind: "warning",
        okLabel: "Replace",
        cancelLabel: "Cancel",
      });
      if (!ok) return;
      await cmd.importData(path);
      await loadConfig();
      showSuccess("Data imported successfully.");
    } catch (err) {
      showError(humanizeError(err));
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

  const otherClient = MCP_CLIENTS.find((c) => c.value === selectedClient);

  return (
    <div className="flex flex-col h-full">
      <div className="flex flex-col gap-[var(--spacing-4)] px-[var(--spacing-5)] pt-[var(--spacing-5)] pb-[var(--spacing-2)]">
        <UIText variant="title" as="h2">
          Settings
        </UIText>

        <UITabs<SettingsTab>
          ariaLabel="Settings sections"
          value={tab}
          onChange={setTab}
          options={[
            { value: "general", label: "General" },
            { value: "integrations", label: "Integrations" },
            { value: "backends", label: "Remote backends" },
            { value: "data", label: "Data" },
          ]}
        />
      </div>

      <div className="flex-1 overflow-y-auto px-[var(--spacing-5)] pb-[var(--spacing-5)]">
        <UITabPanel value="general" active={tab}>
          <div className="flex flex-col gap-[var(--spacing-5)]">
            <section className="flex flex-col gap-[var(--spacing-3)]">
              <UIText variant="section" as="h3">
                Launch
              </UIText>
              <label className="inline-flex items-center gap-[var(--spacing-2)] cursor-pointer w-fit">
                <button
                  type="button"
                  role="switch"
                  aria-checked={autostart}
                  aria-label="Launch at login"
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
                <UIText variant="body">Launch at login</UIText>
              </label>
            </section>

            <UIDivider />

            <section className="flex flex-col gap-[var(--spacing-3)]">
              <UIText variant="section" as="h3">
                Port range
              </UIText>
              <UIText variant="body" className="text-text-secondary">
                Changes only affect new projects. Already assigned ranges stay the same.
              </UIText>
              <div className="flex items-end gap-[var(--spacing-3)]">
                <UIInput
                  label="Base port"
                  type="number"
                  value={basePort}
                  onChange={(e) => setBasePort(e.target.value)}
                  wrapperClassName="w-32"
                />
                <UIInput
                  label="Range size"
                  type="number"
                  value={rangeSize}
                  onChange={(e) => setRangeSize(e.target.value)}
                  wrapperClassName="w-32"
                />
                <UIButton variant="primary" onClick={handleSaveConfig}>
                  <Save size={14} aria-hidden="true" />
                  {configSaved ? "Saved" : "Save"}
                </UIButton>
              </div>
            </section>
          </div>
        </UITabPanel>

        <UITabPanel value="integrations" active={tab}>
          <div className="flex flex-col gap-[var(--spacing-5)]">
            <section className="flex flex-col gap-[var(--spacing-3)]">
              <UIText variant="section" as="h3">
                Claude Code
              </UIText>
              <div className="flex items-center gap-[var(--spacing-3)]">
                {mcpInstalled === null ? (
                  <UIText variant="body" className="text-text-muted">
                    Checking...
                  </UIText>
                ) : mcpInstalled ? (
                  <>
                    <UIBadge variant="active">
                      <CheckCircle size={12} aria-hidden="true" />
                      Connected
                    </UIBadge>
                    <UIButton variant="danger" onClick={handleUninstall}>
                      <Trash2 size={14} aria-hidden="true" />
                      Remove
                    </UIButton>
                  </>
                ) : (
                  <>
                    <UIBadge variant="inactive">
                      <XCircle size={12} aria-hidden="true" />
                      Not connected
                    </UIBadge>
                    <UIButton
                      variant="primary"
                      onClick={handleInstall}
                      disabled={installing}
                    >
                      <Download size={14} aria-hidden="true" />
                      {installing ? "Connecting..." : "Connect"}
                    </UIButton>
                  </>
                )}
              </div>

              <UIText variant="body" className="text-text-secondary">
                {mcpInstalled
                  ? "Claude Code can now manage your project ports automatically. Restart Claude Code if you just connected."
                  : "Connect to Claude Code to reserve ports and register services automatically."}
              </UIText>

              {mcpInstalled && (
                <div className="flex flex-col gap-[var(--spacing-1)] bg-bg-surface rounded-[var(--radius-md)] p-[var(--spacing-3)]">
                  <UIText variant="label">Available tools</UIText>
                  <div className="flex flex-wrap gap-[var(--spacing-1)]">
                    {["list_all", "reserve_range", "register_port", "release_project", "scan_active"].map(
                      (tool) => (
                        <UIBadge key={tool} variant="inactive">
                          <UIText variant="mono" className="text-[10px]!">
                            {tool}
                          </UIText>
                        </UIBadge>
                      ),
                    )}
                  </div>
                </div>
              )}
            </section>

            <UIDivider />

            <section className="flex flex-col gap-[var(--spacing-2)]">
              <button
                type="button"
                onClick={() => setOtherEditorsOpen(!otherEditorsOpen)}
                aria-expanded={otherEditorsOpen}
                className="
                  flex items-center justify-between
                  text-left cursor-pointer
                  focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber rounded-[var(--radius-sm)]
                "
              >
                <UIText variant="section" as="h3">
                  Other editors
                </UIText>
                <ChevronRight
                  size={16}
                  aria-hidden="true"
                  className={`
                    text-text-secondary transition-transform duration-150
                    ${otherEditorsOpen ? "rotate-90" : ""}
                  `}
                />
              </button>

              {!otherEditorsOpen && (
                <UIText variant="body" className="text-text-secondary">
                  Manual config for Cursor, VS Code, Codex, Windsurf, Cline.
                </UIText>
              )}

              {otherEditorsOpen && (
                <div className="flex flex-col gap-[var(--spacing-3)] pt-[var(--spacing-2)]">
                  <UISelect
                    label="Select editor"
                    options={MCP_CLIENTS.map((c) => ({ value: c.value, label: c.label }))}
                    value={selectedClient}
                    onChange={setSelectedClient}
                  />

                  {otherClient && (() => {
                    const configCode = generateMcpConfig(otherClient, mcpDir);
                    const configLabel = otherClient.format === "toml" ? "MCP config (TOML)" : "MCP config";

                    return (
                      <div className="flex flex-col gap-[var(--spacing-3)]">
                        <CodeBlock code={configCode} label={configLabel} />

                        <UIText variant="body" className="text-text-secondary">
                          Paste the config into <UIText variant="mono" className="text-[11px]!">{otherClient.configPath}</UIText>
                        </UIText>

                        {otherClient.rulesPath && (
                          <CodeBlock code={generateRulesContent(otherClient)} label={`Instructions for ${otherClient.label}`} />
                        )}

                        {otherClient.rulesPath && (
                          <UIText variant="body" className="text-text-secondary">
                            Paste the instructions into <UIText variant="mono" className="text-[11px]!">{otherClient.rulesPath}</UIText> at the root of your project.
                          </UIText>
                        )}

                        <UIText variant="body" className="text-text-muted text-[11px]!">
                          Portsage must be running to use the MCP tools.
                        </UIText>
                      </div>
                    );
                  })()}
                </div>
              )}
            </section>
          </div>
        </UITabPanel>

        <UITabPanel value="backends" active={tab}>
          <RemoteBackendsPanel
            remotes={remoteBackends}
            tunnels={tunnels}
            currentTarget={backendTarget}
            onChanged={() => onBackendsChanged?.()}
          />
        </UITabPanel>

        <UITabPanel value="data" active={tab}>
          <div className="flex flex-col gap-[var(--spacing-3)]">
            <UIText variant="section" as="h3">
              Backup &amp; restore
            </UIText>
            <UIText variant="body" className="text-text-secondary">
              Export or import the database and preferences for backup or migration.
              Importing replaces the current data.
            </UIText>
            <div className="flex gap-[var(--spacing-2)]">
              <UIButton variant="ghost" onClick={handleExport}>
                <Download size={14} aria-hidden="true" />
                Export
              </UIButton>
              <UIButton variant="ghost" onClick={handleImport}>
                <Upload size={14} aria-hidden="true" />
                Import
              </UIButton>
            </div>
          </div>
        </UITabPanel>
      </div>

      {version && (
        <div className="flex justify-end px-[var(--spacing-5)] py-[var(--spacing-2)] border-t border-border-subtle">
          <UIText variant="mono" className="text-[10px]! text-text-muted">
            v{version}
          </UIText>
        </div>
      )}
    </div>
  );
}
