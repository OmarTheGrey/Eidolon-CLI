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
| `Skill` | Invoke a skill from `.eidolon/skills/`, `.agents/skills/`, `.claude/skills/` |
| `Agent` | Spawn a sub-agent for parallel work |
| `TodoRead` | Read the current todo list |
| `TodoWrite` | Update the todo list |

Additional tools come from MCP servers configured in your project — they appear alongside built-in tools and go through the same permission and hook pipeline.

## Skills

Skills are reusable prompt templates — one of Eidolon's key extensibility surfaces. They allow domain-specific knowledge to be packaged as `SKILL.md` files and discovered automatically:

```
.eidolon/skills/<name>/SKILL.md
.agents/skills/<name>/SKILL.md
.claude/skills/<name>/SKILL.md
~/.eidolon/skills/<name>/SKILL.md
```

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
