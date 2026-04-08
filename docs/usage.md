# Usage Guide

Eidolon operates as an interactive agent runtime with multiple execution modes — from a persistent REPL for exploratory coding to one-shot commands for automation and pipeline integration.

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

| Alias | Resolves To |
|---|---|
| `opus` | `claude-opus-4-6` |
| `sonnet` | `claude-sonnet-4-6` |
| `haiku` | `claude-haiku-4-5-20251213` |

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

Additional tools come from MCP servers configured in your project — they appear alongside built-in tools and go through the same permission and hook pipeline.

## Semantic Search

When indexing is enabled (see [Configuration](configuration.md)), the agent gains deep codebase awareness through two mechanisms:

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

Syndicate mode coordinates a collection of specialized sub-agents around one shared task.

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

- Shared session memory for coordination (`SyndicateMemory*` tools)
- Per-agent lifecycle tracking in the final summary
- Spawn safety protections (session spawn cap and recursion guard)

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
