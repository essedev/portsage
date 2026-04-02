#!/usr/bin/env bash
set -euo pipefail

# Grimport MCP - Install script
# Registers the MCP server in Claude Code and installs the skill.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MCP_DIR="$SCRIPT_DIR"
SKILL_NAME="grimport"

echo "=== Grimport MCP - Setup ==="
echo ""

# Check dependencies
if ! command -v uv &>/dev/null; then
    echo "Error: uv is not installed. Install it: curl -LsSf https://astral.sh/uv/install.sh | sh"
    exit 1
fi

if ! command -v jq &>/dev/null; then
    echo "Error: jq is not installed. Install it: brew install jq"
    exit 1
fi

# Install Python dependencies
echo "[1/4] Installing Python dependencies..."
cd "$MCP_DIR"
uv sync --quiet

# Choose installation scope
echo ""
echo "Where do you want to install the MCP server?"
echo "  1) Global (all projects) - ~/.claude.json"
echo "  2) This project only     - .mcp.json"
echo ""
read -rp "Choice [1/2]: " CHOICE

if [ "$CHOICE" = "2" ]; then
    MCP_FILE=".mcp.json"
    if [ ! -f "$MCP_FILE" ]; then
        echo "{}" > "$MCP_FILE"
    fi
else
    MCP_FILE="$HOME/.claude.json"
    if [ ! -f "$MCP_FILE" ]; then
        echo "{}" > "$MCP_FILE"
    fi
fi

# Register MCP server
echo "[2/4] Registering MCP server in $MCP_FILE..."
jq --arg dir "$MCP_DIR" '.mcpServers["grimport"] = {
    "type": "stdio",
    "command": "uv",
    "args": ["--directory", $dir, "run", "python", "server.py"]
}' "$MCP_FILE" > "$MCP_FILE.tmp" && mv "$MCP_FILE.tmp" "$MCP_FILE"

# Install skill
echo "[3/4] Installing skill..."
SKILL_DIR="$HOME/.claude/skills/$SKILL_NAME"
mkdir -p "$SKILL_DIR"
cp "$SCRIPT_DIR/SKILL.md" "$SKILL_DIR/SKILL.md"

# Add permissions
echo "[4/4] Adding tool permissions..."
SETTINGS_FILE="$HOME/.claude/settings.json"
if [ ! -f "$SETTINGS_FILE" ]; then
    echo '{}' > "$SETTINGS_FILE"
fi

jq '.permissions.allow = (.permissions.allow // []) + [
    "mcp__grimport__list_all",
    "mcp__grimport__reserve_range",
    "mcp__grimport__register_port",
    "mcp__grimport__release_project",
    "mcp__grimport__scan_active"
] | .permissions.allow |= unique' "$SETTINGS_FILE" > "$SETTINGS_FILE.tmp" && mv "$SETTINGS_FILE.tmp" "$SETTINGS_FILE"

echo ""
echo "=== Done! ==="
echo ""
echo "Make sure the Grimport app is running before using MCP tools."
echo "Restart Claude Code to load the new MCP server."
