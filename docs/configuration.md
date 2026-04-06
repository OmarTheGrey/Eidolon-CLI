# Configuration

Eidolon uses a layered JSON configuration system designed for composability — user-global defaults, project-level overrides, and machine-local secrets merge cleanly via deep merge, with later files in the resolution order winning.

## Resolution Order

| Priority | Path | Scope |
|---|---|---|
| 1 (lowest) | `~/.eidolon.json` | User global |
| 2 | `~/.config/eidolon/settings.json` | User global (XDG) |
| 3 | `<project>/.eidolon.json` | Project |
| 4 | `<project>/.eidolon/settings.json` | Project |
| 5 (highest) | `<project>/.eidolon/settings.local.json` | Machine-local (gitignored) |

View the resolved config in the REPL with `/config`.

## Config File Format

All config files are JSON objects. Example `.eidolon.json`:

```json
{
  "model": "claude-sonnet-4-6",
  "permissions": {
    "defaultMode": "workspace-write"
  },
  "tools": {
    "allowed": ["bash", "read_file", "write_file", "edit_file"],
    "disabled": []
  },
  "mcpServers": {
    "my-server": {
      "type": "stdio",
      "command": "node",
      "args": ["./mcp-server/index.js"]
    }
  }
}
```

## Configuration Keys

### `model`

Default model for conversations. Accepts full model names or aliases (`opus`, `sonnet`, `haiku`).

```json
{ "model": "claude-opus-4-6" }
```

### `permissions`

Control the default permission mode. This is a critical surface for embedding Eidolon in automated pipelines — the permission mode determines what the agent can do without human approval.

```json
{
  "permissions": {
    "defaultMode": "workspace-write"
  }
}
```

Valid modes: `read-only`, `workspace-write`, `danger-full-access`, `dontAsk`.

### `mcpServers`

Define MCP (Model Context Protocol) servers that extend the agent's tool capabilities. MCP is Eidolon's primary extensibility mechanism for integrating external tools — any MCP-compliant server automatically becomes available to the agent through the same permission and hook pipeline as built-in tools.

```json
{
  "mcpServers": {
    "database": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-postgres"],
      "env": {
        "DATABASE_URL": "postgresql://localhost/mydb"
      }
    },
    "remote-tools": {
      "type": "websocket",
      "url": "ws://localhost:8080/mcp"
    }
  }
}
```

**Server types:**

| Type | Transport | Config Keys |
|---|---|---|
| `stdio` | Subprocess with JSON-RPC over stdin/stdout | `command`, `args`, `env` |
| `websocket` | WebSocket connection | `url` |
| `remote` | HTTP transport | `url`, `headers` |
| `managed_proxy` | Managed proxy server | `url`, `proxyConfig` |

### `tools`

Control which tools are available to the agent. Use this to restrict the agent's capabilities for specific project contexts or automated workflows.

```json
{
  "tools": {
    "allowed": ["bash", "read_file"],
    "disabled": ["write_file"]
  }
}
```

## Environment Variables

| Variable | Description |
|---|---|
| `ANTHROPIC_API_KEY` | API key for Anthropic |
| `ANTHROPIC_AUTH_TOKEN` | Bearer token (alternative to API key) |
| `ANTHROPIC_BASE_URL` | Override the Anthropic API endpoint |
| `EIDOLON_CONFIG_HOME` | Override the config/credentials directory (default: `~/.eidolon`) |
| `CLAUDE_CONFIG_DIR` | Claude-compatible config directory for skill/command discovery |
| `CODEX_HOME` | Codex-compatible config directory for skill discovery |

## Project Structure

After running `eidolon-cli init`, your project will have:

```
project/
├── .eidolon.json              # Project configuration
├── .eidolon/
│   ├── settings.json          # Additional project config
│   ├── settings.local.json    # Machine-local overrides (gitignored)
│   ├── sessions/              # Conversation history (gitignored)
│   ├── skills/                # Project-specific skills
│   │   └── <name>/SKILL.md
│   ├── commands/              # Legacy command definitions
│   └── plugins/
│       └── installed/         # Installed plugins
└── CLAUDE.md                  # Project context file (read by system prompt)
```

## Instruction Files

The system prompt automatically includes content from these files when they exist:

| File | Scope |
|---|---|
| `CLAUDE.md` | Project root — primary project context |
| `.eidolon/instructions.md` | Project-level instructions |
| `~/.eidolon/instructions.md` | User-level global instructions |

These files let you provide persistent context to the model about your project's conventions, architecture, and preferences.
