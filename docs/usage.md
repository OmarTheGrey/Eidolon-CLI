# Usage Guide

Eidolon operates as an interactive agent runtime with multiple execution modes — from a persistent REPL for exploratory work to one-shot commands for automation and pipeline integration. While the current tool suite is coding-native, the runtime interfaces described here are domain-independent by design.

## Running Modes

### Interactive REPL

```bash
eidolon-cli
```

Opens a persistent session where you interact with the agent through messages and slash commands. Conversation history persists to `.eidolon/sessions/` as structured JSONL.

### One-Shot Prompt

```bash
eidolon-cli prompt "summarize this repository"
```

Executes a single conversation turn and exits. Useful for scripting.

### Shorthand Prompt

```bash
eidolon-cli "explain rust/crates/runtime/src/lib.rs"
```

Any bare string argument is treated as a prompt.

### JSON Output

```bash
eidolon-cli --output-format json prompt "status"
```

All commands support `--output-format json` for machine-readable output — essential for embedding Eidolon into larger orchestration systems or CI/CD pipelines.

## CLI Flags

| Flag | Description |
|---|---|
| `--model <alias>` | Model to use (`opus`, `sonnet`, `haiku`, or full model name) |
| `--output-format <fmt>` | Output format: `text` (default) or `json` |
| `--permission-mode <mode>` | Permission mode: `read-only`, `workspace-write`, `danger-full-access` |
| `--dangerously-skip-permissions` | Skip all permission checks |
| `--allowedTools <tools>` | Comma-separated list of allowed tools |
| `--resume <ref>` | Resume a session (`latest`, `last`, or session path) |
| `--print` / `-p` | Print system prompt and exit |
| `--help` / `-h` | Show help |
| `--version` / `-V` | Show version |

## Model Aliases

### Anthropic (default)

| Alias | Resolves To |
|---|---|
| `opus` | `claude-opus-4-6` |
| `sonnet` | `claude-sonnet-4-6` |
| `haiku` | `claude-haiku-4-5-20251213` |

### xAI (Grok)

| Alias | Resolves To |
|---|---|
| `grok` | `grok-3` |
| `grok-3` | `grok-3` |
| `grok-mini` | `grok-3-mini` |
| `grok-3-mini` | `grok-3-mini` |
| `grok-2` | `grok-2` |

Grok models require the `XAI_API_KEY` environment variable. The API endpoint can be overridden with `XAI_BASE_URL`.

### OpenAI-Compatible Providers

Any model name not matching a built-in alias is routed through the OpenAI-compatible provider. Set `OPENAI_API_KEY` and optionally `OPENAI_BASE_URL` to use third-party OpenAI-compatible endpoints.

Pass `--model sonnet` or use the `/model` slash command in the REPL.

## Slash Commands

Inside the REPL, prefix commands with `/`:

| Command | Description |
|---|---|
| `/help` | List available commands |
| `/doctor` | Run preflight diagnostics |
| `/status` | Current session info (model, tokens, cost) |
| `/cost` | Token usage and cost breakdown |
| `/model [name]` | Show or switch the active model |
| `/permissions [mode]` | Show or switch permission mode |
| `/config` | Show resolved configuration |
| `/session` | Session metadata |
| `/compact` | Force session compaction |
| `/export` | Export conversation |
| `/agents [args]` | Manage sub-agents |
| `/syndicate [args]` | Run or inspect Syndicate collections |
| `/mcp [args]` | Manage MCP servers |
| `/skills [args]` | Browse and invoke skills |
| `/plugins [args]` | Plugin management |
| `/sandbox` | Sandbox/container status |
| `/init` | Initialize project config |
| `/login` | Start OAuth flow |
| `/logout` | Clear saved credentials |
| `/diff` | Show recent file changes |

## Session Management

### How Sessions Work

Every REPL interaction is persisted to `.eidolon/sessions/` as JSONL files (one JSON object per line). Sessions capture the full conversation history including tool calls, results, and metadata — making them fully replayable and inspectable by external tooling.

### Resume a Session

```bash
eidolon-cli --resume latest
```

`latest`, `last`, and `recent` are aliases that resolve to the most recent session file.

### Resume and Run Commands

```bash
eidolon-cli --resume latest /status /diff
```

Resumes the session and immediately executes the given slash commands.

### Session Compaction

When conversation history grows large (measured by estimated token count), the runtime automatically compacts older messages into summaries — preserving context while keeping the active window efficient. Force compaction manually with `/compact`.

## Tool Usage

The agent runtime has access to a core set of built-in tools. Each tool call passes through the permission enforcer and plugin hook pipeline before execution — nothing runs unchecked.

| Tool | Purpose |
|---|---|
| `bash` | Execute shell commands with timeout and background support |
| `read_file` | Read file contents with line range selection |
| `write_file` | Create or overwrite files |
| `edit_file` | Apply targeted string replacements |
| `glob_search` | Find files by glob pattern |
| `grep_search` | Search file contents with regex |
| `semantic_search` | Search the codebase by meaning using local embeddings |
| `Skill` | Invoke a skill from `.eidolon/skills/`, `.agents/skills/`, `.claude/skills/` |
| `Agent` | Spawn a sub-agent for parallel work |
| `SyndicateMemoryWrite` | Write session-scoped shared memory during syndicate runs |
| `SyndicateMemoryRead` | Read session-scoped shared memory during syndicate runs |
| `SyndicateMemoryLog` | Append observations to syndicate session logs |
| `SyndicateMemorySearch` | Search syndicate shared memory entries |
| `TodoRead` | Read the current todo list |
| `TodoWrite` | Update the todo list |
| `ProcessList` | List all background processes with status and command |
| `ProcessStatus` | Get the current status of a background process by ID |
| `ProcessOutput` | Read the stdout/stderr output log of a background process |
| `ProcessKill` | Terminate a running background process |

Additional tools come from MCP servers configured in your project — they appear alongside built-in tools and go through the same permission and hook pipeline.

## Background Processes

The `bash` tool supports `"run_in_background": true` for long-running commands like builds, test suites, or dev servers. When a background process is started:

1. The command runs detached — the conversation continues immediately
2. stdout/stderr are captured to a log file for later reading
3. The process is tracked in a registry with a unique ID (e.g. `bg-1`)

Use the process management tools to interact:

```
ProcessList        → see all running and finished processes
ProcessStatus(id)  → check if a specific process is done
ProcessOutput(id)  → read the captured output
ProcessKill(id)    → terminate a running process
```

This enables workflows like: kick off `cargo build --release`, continue reviewing code, then check the build result when ready.

## Semantic Search

When indexing is enabled (see [Configuration](configuration.md)), the agent gains deep workspace awareness through two mechanisms. The embedding model is **configurable** — any BERT-compatible model on HuggingFace works via the `modelId` config key. The default `all-MiniLM-L6-v2` (~80 MB) is fast and capable; swap it for a larger model when accuracy matters more.

### Automatic Context Injection

Every conversation turn, the user's message is embedded and compared against the workspace index. The top-K most relevant code snippets are automatically injected into the system prompt as a `<codebase_context>` section. This happens transparently — no action required from the user or the model.

### The `semantic_search` Tool

The model can also explicitly query the index:

```json
{
  "query": "how does permission enforcement work",
  "top_k": 5
}
```

This returns ranked results with file paths, line ranges, similarity scores, and code content. It's useful when the model needs to find code related to a concept rather than an exact string match (which `grep_search` handles).

### When to Use Which

| Need | Tool |
|---|---|
| Find an exact string or regex | `grep_search` |
| Find files by name pattern | `glob_search` |
| Find code related to a concept | `semantic_search` |
| Read a specific file | `read_file` |

The auto-context injection means the model often already has relevant code in its context before it even decides to search. This reduces the number of tool calls needed per turn.

## Skills

Skills are reusable prompt templates — one of Eidolon's key extensibility surfaces. They allow domain-specific knowledge to be packaged as `SKILL.md` files and discovered automatically.

### Discovery Paths

Skills are loaded from the following directories (project-level and user-level):

**Project-level** (relative to workspace root):

| Path | Notes |
|------|-------|
| `.eidolon/skills/` | Primary project skills |
| `.agents/skills/` | Agents-compatible |
| `.claude/skills/` | Claude-compatible |
| `.omc/skills/` | OMC-compatible |
| `.codex/skills/` | Codex-compatible |

**User-level** (home directory):

| Path | Notes |
|------|-------|
| `~/.eidolon/skills/` | Primary user skills |
| `~/.omc/skills/` | OMC-compatible |
| `~/.codex/skills/` | Codex-compatible |
| `~/.claude/skills/` | Claude-compatible |
| `~/.claude/skills/omc-learned/` | Auto-learned OMC skills |

Legacy `/commands` directories (e.g. `~/.eidolon/commands/`) are also loaded for backwards compatibility.

The `EIDOLON_CONFIG_HOME`, `CLAUDE_CONFIG_DIR`, and `CODEX_HOME` environment variables override the respective home-level discovery roots.

The model invokes skills with the `Skill` tool. Browse available skills with `/skills`.

### Skill File Format

```markdown
---
name: my-skill
description: What this skill does
---

# my-skill

Instructions and context for the model...
```

## Sub-Agents

The model can spawn sub-agents for parallel work using the `Agent` tool. Each agent runs as an independent conversation with its own context, executing tools and returning results to the parent.

Manage agents with `/agents`:

```
/agents           # list active agents
/agents status    # show agent statuses
```

## Syndicate Mode

Syndicate is a **framework for defining agent team topologies** — the built-in collections (`feature-build`, `code-review`, `debug-squad`) are demos that ship with the runtime. Define custom collections for any coordination pattern: migration pipelines, security audits, content workflows, or any domain-specific agent team.

Run from CLI:

```bash
eidolon-cli syndicate --list
eidolon-cli syndicate <collection>
eidolon-cli syndicate <collection> "<task override>"
```

Or from REPL:

```text
/syndicate --list
/syndicate <collection>
```

Syndicate runs provide:

- Session-scoped shared memory for coordination (`SyndicateMemory*` tools)
- Per-agent lifecycle tracking in the final summary
- Spawn safety protections (session spawn cap and recursion guard)

Custom collections are defined in workspace files — each agent gets a role, system prompt, optional model override, and access to the shared memory tools. See [Architecture](architecture.md) for the orchestration design.

## Common Workflows

### Code Review

```bash
eidolon-cli --permission-mode read-only "review the changes in src/"
```

### Automated Fixes

```bash
eidolon-cli --permission-mode workspace-write "fix all clippy warnings in the runtime crate"
```

### Scripting with JSON

```bash
result=$(eidolon-cli --output-format json prompt "list all TODO comments")
echo "$result" | jq '.content'
```

### Multi-Command Session

```bash
eidolon-cli --resume latest /doctor /status
```

## Profiles

Profiles let you run multiple fully isolated Eidolon instances from a single installation. Each profile gets its own config, sessions, skills, and credentials.

```bash
# Activate a profile via environment variable
export EIDOLON_PROFILE=work
eidolon-cli

# Or use a different profile per session
EIDOLON_PROFILE=personal eidolon-cli prompt "summarize my notes"
```

Profiles are stored under `~/.eidolon/profiles/<name>/`. Create and manage them programmatically via the `runtime::profile` module, or simply create the directory manually.

## MCP Server Mode

Eidolon can expose itself as a stdio MCP server, making it accessible from other MCP clients like Claude Desktop, Cursor, or VS Code.

```bash
eidolon-cli mcp-serve
```

This exposes:
- `session_search` — full-text search across all past conversations
- `session_stats` — indexed message count

Combined with the existing MCP client support, Eidolon is now **bidirectional** with MCP — it both consumes and exposes tools through the same protocol.

## OpenAI-Compatible API Server

Eidolon can run as an HTTP server that implements the OpenAI Chat Completions API, making it usable as a drop-in backend for any OpenAI-compatible frontend.

```bash
eidolon-cli serve --bind 127.0.0.1:8080
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/chat/completions` | Chat completion (streaming and non-streaming) |
| GET | `/v1/models` | List available models |
| GET | `/health` | Server health check |

### Compatible Frontends

Any application that supports the OpenAI API can point at Eidolon's server:
- [Open WebUI](https://github.com/open-webui/open-webui)
- [LobeChat](https://github.com/lobehub/lobe-chat)
- [LibreChat](https://github.com/danny-avila/LibreChat)
- [NextChat](https://github.com/ChatGPTNextWeb/ChatGPT-Next-Web)
- [Jan](https://github.com/janhq/jan)

### Session Persistence

By default, each request is stateless. To maintain conversation context across requests, include the `X-Eidolon-Session-Id` header:

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Eidolon-Session-Id: my-session" \
  -d '{"model": "sonnet", "messages": [{"role": "user", "content": "hello"}]}'
```

### Authentication

Optionally protect the server with a bearer token configured at startup. Requests must include `Authorization: Bearer <token>`.

## Themes

Eidolon supports data-driven visual theming via YAML skin definitions. Themes control spinner characters, tool result colors, diff coloring, box-drawing characters, and prompt symbols.

### Built-in Themes

| Theme | Description |
|-------|-------------|
| `default` | Standard blue/green/cyan palette |
| `mono` | Grayscale for minimal terminals or screen recordings |
| `slate` | Cool blue-grey palette |
| `ember` | Warm amber and crimson |

### Switching Themes

```
/theme            # show current theme and available options
/theme mono       # switch to monochrome
/theme ember      # switch to warm amber
```

### Custom Themes

Drop YAML files in `~/.eidolon/skins/` and they'll appear in `/theme`. A skin file looks like:

```yaml
name: my-theme
description: "My custom theme"
success: 70        # ANSI 256-color code
error: 203
muted: 245
diff_add: 70
diff_remove: 203
diff_hunk: "cyan"  # or a named color
spinner_frames: ["◐", "◓", "◑", "◒"]
prompt_symbol: "» "
```

Any missing fields inherit from the default theme.

## Inline Diff Previews

When the agent edits or overwrites a file, the terminal shows a proper unified diff preview with `@@ hunk headers`, colored additions/removals, and dimmed context lines. This gives immediate visibility into what changed without needing to inspect the file manually.
