# Architecture

Eidolon is a modular agent runtime built in Rust, designed following the **Regula Framework** — a set of agentic design patterns focused on structured autonomy, deterministic recovery, and machine-first interfaces. Every subsystem (tool execution, permission enforcement, session management, MCP integration) is exposed as a composable, overridable surface, making the entire system embeddable into larger orchestration pipelines.

## High-Level Overview

```
┌──────────────────────────────────────────────────────────────┐
│                      eidolon-cli binary                      │
│  CLI parsing · REPL loop · TUI rendering · session I/O      │
├──────────┬───────────┬────────────┬──────────┬───────────────┤
│ commands │   tools   │  plugins   │   api    │   telemetry   │
│ slash cmd│ tool exec │ hooks &    │ provider │ tracing &     │
│ registry │ bash/file │ lifecycle  │ clients  │ analytics     │
├──────────┴───────────┴────────────┴──────────┴───────────────┤
│                         runtime                              │
│  config · sessions · permissions · MCP · OAuth · prompts     │
│  conversation loop · compaction · sandbox · file ops         │
└──────────────────────────────────────────────────────────────┘
```

The binary crate (`eidolon-cli`) sits at the top and depends on six library crates. The `runtime` crate is the shared foundation — it owns config resolution, session persistence, permission enforcement, and the conversation loop. This layered design means any crate can be replaced or extended independently without modifying the core conversation engine.

## Workspace Structure

```
rust/
├── Cargo.toml                  # Workspace manifest
└── crates/
    ├── eidolon-cli/            # Binary — CLI entry, REPL, TUI
    │   └── src/
    │       ├── main.rs         # CLI parsing, session mgmt, conversation loop
    │       ├── init.rs         # `eidolon init` — project scaffolding
    │       ├── input.rs        # Rustyline-based input with slash-command completion
    │       └── render.rs       # Markdown rendering, spinners, syntax highlighting
    ├── api/                    # HTTP clients for LLM providers
    │   └── src/
    │       ├── lib.rs          # Public API surface
    │       ├── client.rs       # OAuth token exchange
    │       ├── error.rs        # ApiError types
    │       ├── types.rs        # Request/response protocol types
    │       ├── sse.rs          # Server-sent event stream parsing
    │       ├── prompt_cache.rs # Prompt caching with TTL management
    │       └── providers/
    │           ├── mod.rs      # Provider trait, ProviderKind enum
    │           ├── anthropic.rs# Anthropic API + auth + retry logic
    │           └── openai_compat.rs # OpenAI/xAI compatible client
    ├── runtime/                # Core runtime — the backbone
    │   └── src/
    │       ├── lib.rs          # Public API (sessions, config, permissions, MCP, tools)
    │       ├── config.rs       # Config loading and precedence merging
    │       ├── conversation.rs # Turn execution loop
    │       ├── session.rs      # Session persistence (JSONL)
    │       ├── permissions.rs  # Permission evaluation
    │       ├── permission_enforcer.rs
    │       ├── prompt.rs       # System prompt assembly
    │       ├── oauth.rs        # PKCE OAuth + credential storage
    │       ├── hooks.rs        # Pre/post tool-use hook execution
    │       ├── bash.rs         # Shell command execution
    │       ├── sandbox.rs      # Container detection (Docker/Podman)
    │       ├── compact.rs      # Token-aware session compaction
    │       ├── file_ops.rs     # read_file, write_file, edit_file
    │       ├── mcp_client.rs   # MCP transport (stdio, websocket, remote)
    │       ├── mcp_stdio.rs    # MCP stdio subprocess management
    │       ├── mcp_tool_bridge.rs # Bridge MCP tools into the tool executor
    │       ├── mcp_lifecycle_hardened.rs # MCP server lifecycle validation
    │       ├── plugin_lifecycle.rs # Plugin health and discovery
    │       ├── stale_branch.rs # Git branch freshness checks
    │       ├── recovery_recipes.rs # Automatic error recovery
    │       └── task_packet.rs  # Structured task/work packets
    ├── commands/               # Slash command registry and dispatch
    │   └── src/lib.rs          # Command specs, validation, skill resolution
    ├── tools/                  # Tool implementations
    │   └── src/
    │       ├── lib.rs          # Tool registry, bash/file/agent/skill execution
    │       └── lane_completion.rs # Git lane workflow support
    ├── plugins/                # Plugin system
    │   └── src/lib.rs          # Plugin kinds, metadata, hooks, lifecycle
    ├── telemetry/              # Tracing and analytics
    │   └── src/lib.rs          # ClientIdentity, SessionTracer, TelemetrySink
    └── mock-anthropic-service/ # Test mock server
        └── src/main.rs         # Lightweight mock Anthropic API
```

## Crate Dependency Graph

```
eidolon-cli
├── api          ← runtime, telemetry
├── commands     ← runtime, plugins
├── tools        ← api, commands, runtime, plugins
├── plugins      ← (standalone — only serde)
├── runtime      ← plugins
└── telemetry    ← (standalone — only serde)
```

## Core Architectural Flows

### 1. Startup → REPL Loop

```
main() → parse_args()
  ├── CliAction::Repl → run_repl()
  │     ├── ConfigLoader::load()      # merge config files
  │     ├── resolve_startup_auth_source() # API key or OAuth
  │     ├── AnthropicClient::from_env()   # build provider
  │     ├── PluginManager::new()          # discover plugins
  │     ├── McpServerManager::new()       # boot MCP servers
  │     └── REPL loop:
  │           ├── input::read_line()      # rustyline prompt
  │           ├── /slash → dispatch_slash_command()
  │           └── text  → run_conversation_turn()
  ├── CliAction::Prompt → one-shot turn, then exit
  ├── CliAction::Login  → OAuth PKCE flow
  └── CliAction::Doctor → preflight diagnostics
```

### 2. Conversation Turn

```
run_conversation_turn(user_message)
  ├── build MessageRequest (model, system prompt, history, tools)
  ├── AnthropicClient::stream_message()
  │     └── SSE stream → ContentBlockDelta events
  ├── render streaming markdown to terminal
  ├── for each tool_use in response:
  │     ├── HookRunner::run(PreToolUse)
  │     ├── PermissionEnforcer::check()
  │     ├── tools::execute_tool(name, input)
  │     │     ├── "bash"       → run_bash()
  │     │     ├── "read_file"  → run_read_file()
  │     │     ├── "write_file" → run_write_file()
  │     │     ├── "Skill"      → execute_skill()
  │     │     ├── "Agent"      → execute_agent()
  │     │     ├── "SyndicateMemory*" → run_syndicate_memory_*
  │     │     └── MCP tools    → mcp_tool_bridge
  │     ├── HookRunner::run(PostToolUse)
  │     └── append tool result to conversation
  ├── estimate_session_tokens()
  │     └── if over threshold → compact_session()
  └── persist session to .eidolon/sessions/
```

### 3. Authentication

```
eidolon login
  ├── generate_pkce_pair() → code_verifier + code_challenge
  ├── generate_state()
  ├── open browser → Anthropic OAuth consent page
  ├── TcpListener on localhost:4545
  ├── receive callback with authorization code
  ├── exchange code for tokens (POST /oauth/token)
  └── save_oauth_credentials() → ~/.eidolon/credentials.json

read_api_key()
  ├── check ANTHROPIC_API_KEY env var
  ├── check ANTHROPIC_AUTH_TOKEN env var
  └── load_saved_oauth_token() from credentials.json
```

### 4. Config Resolution

Config files are loaded and merged in order (later wins):

```
~/.eidolon.json                        # user global
~/.config/eidolon/settings.json        # XDG-style global
<project>/.eidolon.json                # project root
<project>/.eidolon/settings.json       # project config dir
<project>/.eidolon/settings.local.json # machine-local (gitignored)
```

Each file is a JSON object with keys like `model`, `permissions`, `mcpServers`, `tools`, etc. Values from later files override earlier ones via deep merge.

### 5. MCP Integration

Eidolon treats MCP servers as first-class tool providers — external tools appear alongside built-in tools in the model's tool list and execute through the same permission and hook pipeline. This is a key composability surface: any MCP-compliant server can extend Eidolon's capabilities without modifying the binary.

```
McpServerManager::new(config)
  ├── for each server in config.mcpServers:
  │     ├── McpStdioServerConfig  → spawn subprocess, JSON-RPC over stdio
  │     ├── McpWebSocketServerConfig → connect websocket
  │     ├── McpRemoteServerConfig → HTTP transport
  │     └── McpManagedProxyServerConfig → managed proxy
  ├── McpLifecycleValidator::validate() per server
  └── register McpTool entries into GlobalToolRegistry
```

MCP tools are executed via the MCP bridge when the model calls them.

## Permission System

Eidolon's permission model is designed for machine-first control — every tool call passes through `PermissionEnforcer` before execution, making the system safe to embed in automated pipelines where unrestricted tool access is unacceptable.

Three permission modes control what tools can do:

| Mode | File Read | File Write | Shell Exec | Network |
|------|-----------|------------|------------|---------|
| `read-only` | ✓ | ✗ | ✗ | ✗ |
| `workspace-write` | ✓ | ✓ (in project) | ✓ (sandboxed) | ✗ |
| `danger-full-access` | ✓ | ✓ | ✓ | ✓ |

Permissions are evaluated per-tool-call by `PermissionEnforcer`. The mode can be set via CLI flag (`--permission-mode`), config file, or the `/permissions` slash command.

## Plugin System

Plugins provide hooks that execute before/after tool calls:

```
Plugin Kinds:
  ├── Builtin   — compiled into the binary
  ├── Bundled   — shipped in .eidolon/plugins/installed/
  └── External  — user-installed

Plugin Lifecycle:
  install → discover → healthcheck → register hooks → execute

Hook Events:
  ├── PreToolUse         — runs before each tool execution
  ├── PostToolUse        — runs after successful tool execution
  └── PostToolUseFailure — runs after failed tool execution
```

Plugin metadata lives in `.claude-plugin/plugin.json` within each plugin directory. Hooks are shell scripts receiving JSON input on stdin.

## Session Management

Sessions persist the full conversation history for resumption:

- **Format**: JSONL (one JSON object per line) in `.eidolon/sessions/`
- **Naming**: `session-{unix_ms}-{counter}.jsonl`
- **Compaction**: When estimated tokens exceed the threshold, the session is compacted — older messages are summarized to reduce context size
- **Resumption**: `eidolon --resume latest` loads the most recent session

## Tool Registry

Tools are the model's interface to the environment. The global tool registry combines:

1. **Built-in tools**: `bash`, `read_file`, `write_file`, `edit_file`, `glob_search`, `grep_search`, `Skill`, `Agent`, `SyndicateMemoryWrite`, `SyndicateMemoryRead`, `SyndicateMemoryLog`, `SyndicateMemorySearch`, `TodoRead`, `TodoWrite`
2. **MCP tools**: Dynamically registered from MCP server discovery
3. **Conditional tools**: Enabled based on config or runtime state

Each tool call goes through permission enforcement and hook execution before reaching the tool implementation.

## Syndicate Orchestration

PR #1 added a dedicated multi-agent orchestration path:

- CLI + slash entrypoints dispatch into `run_syndicate(...)` in `eidolon-cli`
- Collections are resolved from built-ins and workspace files
- A session-scoped shared memory file is initialized and exposed through `SyndicateMemory*` tools
- Agent manifests are polled for completion/failure and rolled up into a final summary

Safety constraints now applied in `tools`:

- Session-scoped spawn budget for sub-agent creation
- Recursion guard to block Syndicate -> Syndicate spawning loops
