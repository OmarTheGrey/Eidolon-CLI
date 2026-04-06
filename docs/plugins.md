# Plugin System

Plugins are Eidolon's mechanism for observing and modifying tool execution without touching the core runtime. They fire lifecycle hooks around every tool call — enabling audit trails, custom validation, metrics, error recovery, and any other cross-cutting concern that should run alongside the agent.

This is a key composability surface: in larger systems, plugins let external processes monitor and control agent behavior at the tool-call level.

## Plugin Kinds

| Kind | Location | Description |
|---|---|---|
| **Builtin** | Compiled into the binary | Internal plugins that ship with Eidolon |
| **Bundled** | `.eidolon/plugins/installed/` | Plugins distributed with the CLI release |
| **External** | `.eidolon/plugins/installed/` | User-installed plugins |

## Plugin Structure

Each plugin is a directory containing a `.claude-plugin/plugin.json` manifest:

```
my-plugin/
├── .claude-plugin/
│   └── plugin.json
└── hooks/
    ├── pre.sh
    └── post.sh
```

### Manifest Format

```json
{
  "name": "my-plugin",
  "version": "0.1.0",
  "description": "What this plugin does",
  "defaultEnabled": true,
  "hooks": {
    "PreToolUse": ["./hooks/pre.sh"],
    "PostToolUse": ["./hooks/post.sh"],
    "PostToolUseFailure": ["./hooks/on-failure.sh"]
  }
}
```

## Hook Events

| Event | When | Use Case |
|---|---|---|
| `PreToolUse` | Before a tool executes | Validation, logging, permission checks |
| `PostToolUse` | After successful tool execution | Audit trails, metrics, notifications |
| `PostToolUseFailure` | After a tool fails | Error reporting, recovery triggers |

## Hook Execution

Hook scripts are executed as subprocesses — fully isolated from the runtime. They receive a JSON payload on **stdin** with the tool call context:

```json
{
  "event": "PreToolUse",
  "tool_name": "bash",
  "tool_input": { "command": "ls -la" },
  "session_id": "session-1234567890-0"
}
```

The script's **exit code** determines behavior:
- `0` — hook passed, continue execution
- Non-zero — hook failed, report error

**stdout** from hook scripts is captured and may be included in hook result reporting.

## Plugin Lifecycle

Plugins follow a deterministic lifecycle — consistent with the Regula Framework’s emphasis on structured autonomy and predictable behavior:

```
Discovery → Healthcheck → Registration → Execution
```

1. **Discovery**: Scan `.eidolon/plugins/installed/` for directories containing `.claude-plugin/plugin.json`
2. **Healthcheck**: Verify the manifest is valid and all referenced hook scripts exist
3. **Registration**: Register hook handlers into the `HookRunner`
4. **Execution**: Hooks fire during tool calls as part of the conversation loop — every tool call, every time

## Managing Plugins

Use the `/plugins` slash command in the REPL:

```
/plugins              # list installed plugins
/plugins install <path>   # install from local path
/plugins remove <name>    # remove a plugin
/plugins status           # show health of all plugins
```

## Plugin Registry

Installed plugins are tracked in `.eidolon/plugins/installed.json`:

```json
{
  "plugins": {
    "my-plugin@bundled": {
      "kind": "bundled",
      "id": "my-plugin@bundled",
      "name": "my-plugin",
      "version": "0.1.0",
      "install_path": ".eidolon/plugins/installed/my-plugin-bundled",
      "installed_at_unix_ms": 1700000000000
    }
  }
}
```

## Creating a Plugin

1. Create a directory with the plugin name
2. Add `.claude-plugin/plugin.json` with the manifest
3. Write hook scripts in a `hooks/` directory
4. Install with `/plugins install ./my-plugin`

### Example: Audit Logger

```bash
#!/bin/bash
# hooks/post.sh — Log every tool call to a file
input=$(cat)
tool_name=$(echo "$input" | jq -r '.tool_name')
timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
echo "$timestamp $tool_name" >> /tmp/eidolon-audit.log
exit 0
```

### Example: Pre-flight Validator

```bash
#!/bin/bash
# hooks/pre.sh — Block write operations during business hours
input=$(cat)
tool_name=$(echo "$input" | jq -r '.tool_name')
hour=$(date +%H)
if [[ "$tool_name" == "write_file" && "$hour" -ge 9 && "$hour" -le 17 ]]; then
    echo "Blocked: write operations disabled during business hours"
    exit 1
fi
exit 0
```

## Degraded Mode

If a plugin fails its healthcheck, Eidolon enters **degraded mode** for that plugin — hooks from the failed plugin are skipped, but the rest of the system continues normally. Plugin health is reported via `/plugins status` and the `/doctor` diagnostic.
