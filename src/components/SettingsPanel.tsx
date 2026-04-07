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
import { getVersion } from "@tauri-apps/api/app";
import * as cmd from "@/lib/commands";
import skillMd from "../../mcp/SKILL.md?raw";

type McpClient = {
  value: string;
  label: string;
  configPath: string;
  rulesPath: string | null;
  // mcpServers: Cursor, Claude Desktop, Cline, Windsurf - root key {"mcpServers": {...}}
  // vscodeServers: VS Code Copilot - root key {"servers": {...}}
  // toml: Codex - [mcp_servers.name] sections
  format: "mcpServers" | "vscodeServers" | "toml";
  // Frontmatter wrapper used when generating the rules file content. null = no rules support.
  rulesFormat?: "cursor" | "windsurf";
};

// Ordered by actual MCP usage relevance (not just editor user base).
// Removed: Continue (declining, surclassed by Cline), Zed (too niche).
// Update as the ecosystem shifts.
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

  // VS Code Copilot uses "servers" as the root key, all others use "mcpServers".
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

// Source of truth: mcp/SKILL.md - imported raw at build time so it stays in sync.
// Strip the YAML frontmatter (between --- markers at top) so we can re-wrap with the
// editor-specific frontmatter that each rules format requires.
const SKILL_BODY = skillMd.replace(/^---\n[\s\S]*?\n---\n+/, "");

const SKILL_DESCRIPTION =
  "Manages port allocation across development projects. Use it when you need to assign ports to a new project, register services, or check which ports are in use.";

function generateRulesContent(client: McpClient): string {
  // Cursor .mdc: agent-mode rule (description present, no globs, alwaysApply false).
  // The AI decides when to attach the rule based on the description.
  if (client.rulesFormat === "cursor") {
    return `---
description: ${SKILL_DESCRIPTION}
alwaysApply: false
---

${SKILL_BODY}`;
  }
  // Windsurf .md: model_decision trigger - the model attaches the rule when the
  // description matches the current task context.
  if (client.rulesFormat === "windsurf") {
    return `---
trigger: model_decision
description: ${SKILL_DESCRIPTION}
---

${SKILL_BODY}`;
  }
  // Fallback: raw markdown body, no frontmatter.
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
        <GrimText variant="label">{label}</GrimText>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-[var(--spacing-1)] text-text-muted hover:text-text-primary transition-colors cursor-pointer"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
          <GrimText variant="mono" className="text-[10px]!">
            {copied ? "Copied" : "Copy"}
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
        defaultPath: "portsage-backup.portsage",
        filters: [{ name: "Portsage", extensions: ["portsage"] }],
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
        filters: [{ name: "Portsage", extensions: ["portsage"] }],
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
        Settings
      </GrimText>

      <GrimDivider />

      {/* Port Config Section */}
      <div className="flex flex-col gap-[var(--spacing-3)]">
        <GrimText variant="section" as="h3">
          Port configuration
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
          <GrimText variant="body">Launch at login</GrimText>
        </label>

        <GrimDivider />

        <GrimText variant="section" as="h3">
          Port range
        </GrimText>

        <GrimText variant="body" className="text-text-secondary">
          Changes only affect new projects. Already assigned ranges stay the same.
        </GrimText>

        <div className="flex items-end gap-[var(--spacing-3)]">
          <GrimInput
            label="Base port"
            type="number"
            value={basePort}
            onChange={(e) => setBasePort(e.target.value)}
            wrapperClassName="w-32"
          />
          <GrimInput
            label="Range size"
            type="number"
            value={rangeSize}
            onChange={(e) => setRangeSize(e.target.value)}
            wrapperClassName="w-32"
          />
          <GrimButton variant="primary" onClick={handleSaveConfig}>
            <Save size={14} />
            {configSaved ? "Saved" : "Save"}
          </GrimButton>
        </div>
      </div>

      <GrimDivider />

      {/* MCP Section */}
      <div className="flex flex-col gap-[var(--spacing-3)]">
        <GrimText variant="section" as="h3">
          MCP integration
        </GrimText>

        {/* Claude Code - Auto Install */}
        <div className="flex flex-col gap-[var(--spacing-2)]">
          <GrimText variant="label">Claude Code</GrimText>
          <div className="flex items-center gap-[var(--spacing-3)]">
            {mcpInstalled === null ? (
              <GrimText variant="body" className="text-text-muted">
                Checking...
              </GrimText>
            ) : mcpInstalled ? (
              <>
                <GrimBadge variant="active">
                  <CheckCircle size={12} />
                  Connected
                </GrimBadge>
                <GrimButton variant="danger" onClick={handleUninstall}>
                  <Trash2 size={14} />
                  Remove
                </GrimButton>
              </>
            ) : (
              <>
                <GrimBadge variant="inactive">
                  <XCircle size={12} />
                  Not connected
                </GrimBadge>
                <GrimButton
                  variant="primary"
                  onClick={handleInstall}
                  disabled={installing}
                >
                  <Download size={14} />
                  {installing ? "Connecting..." : "Connect"}
                </GrimButton>
              </>
            )}
          </div>

          <GrimText variant="body" className="text-text-secondary">
            {mcpInstalled
              ? "Claude Code can now manage your project ports automatically. Restart Claude Code if you just connected."
              : "Connect to Claude Code to reserve ports and register services automatically."}
          </GrimText>

          {mcpInstalled && (
            <div className="flex flex-col gap-[var(--spacing-1)] bg-bg-surface rounded-[var(--radius-md)] p-[var(--spacing-3)]">
              <GrimText variant="label">Available tools</GrimText>
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
          <GrimText variant="label">Other editors</GrimText>

          <GrimSelect
            label="Select editor"
            options={MCP_CLIENTS.map((c) => ({ value: c.value, label: c.label }))}
            value={selectedClient}
            onChange={setSelectedClient}
          />

          {(() => {
            const client = MCP_CLIENTS.find((c) => c.value === selectedClient);
            if (!client) return null;
            const configCode = generateMcpConfig(client, mcpDir);
            const configLabel = client.format === "toml" ? "MCP config (TOML)" : "MCP config";

            return (
              <div className="flex flex-col gap-[var(--spacing-3)]">
                <CodeBlock code={configCode} label={configLabel} />

                <GrimText variant="body" className="text-text-secondary">
                  Paste the config into <GrimText variant="mono" className="text-[11px]!">{client.configPath}</GrimText>
                </GrimText>

                {client.rulesPath && (
                  <CodeBlock code={generateRulesContent(client)} label={`Instructions for ${client.label}`} />
                )}

                {client.rulesPath && (
                  <GrimText variant="body" className="text-text-secondary">
                    Paste the instructions into the file <GrimText variant="mono" className="text-[11px]!">{client.rulesPath}</GrimText> at the root of your project.
                  </GrimText>
                )}

                <GrimText variant="body" className="text-text-muted text-[11px]!">
                  Portsage must be running to use the MCP tools.
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
          Data
        </GrimText>

        <GrimText variant="body" className="text-text-secondary">
          Export or import the database and preferences for backup or migration.
        </GrimText>

        <div className="flex gap-[var(--spacing-2)]">
          <GrimButton variant="ghost" onClick={handleExport}>
            <Download size={14} />
            Export
          </GrimButton>
          <GrimButton variant="ghost" onClick={handleImport}>
            <Upload size={14} />
            Import
          </GrimButton>
        </div>
      </div>

      {version && (
        <div className="flex justify-end pt-[var(--spacing-2)]">
          <GrimText variant="mono" className="text-[10px]! text-text-muted">
            v{version}
          </GrimText>
        </div>
      )}
    </div>
  );
}
