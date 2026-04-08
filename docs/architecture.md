# Architecture

Eidolon is a modular, research-driven agent runtime built in Rust, designed following the **Regula Framework** — a set of agentic design patterns focused on structured autonomy, deterministic recovery, and machine-first interfaces.

Every subsystem (tool execution, permission enforcement, session management, MCP integration, semantic indexing) is exposed as a composable, overridable surface, making the entire system embeddable into larger orchestration pipelines. Critically, **none of these subsystems are inherently coupled to software engineering** — the architecture is designed to generalize across knowledge-work domains.

## Design Philosophy

Eidolon's architecture optimizes for three properties:

1. **Domain independence** — The runtime dispatches tools, enforces permissions, runs hooks, manages sessions, and coordinates agents. What those tools *do* is a configuration detail. The current tool suite is coding-native, but the runtime doesn't know or care.
2. **Composability** — Every mechanism (`ToolExecutor`, `ApiClient`, `PermissionPolicy`, prompt builder, hook pipeline) is a replaceable surface. Swap any component without rewriting the conversation loop.
3. **Observability** — Structured I/O everywhere. JSON output on every command, machine-readable sessions, visualized retrieval trajectories (planned), and plugin hooks that let external processes monitor and control every tool call.

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
│  background indexer · IndexHandle · auto-context            │
├──────────────────────────────────────────────────────────────┤
│                        indexing                              │
│  file discovery · chunker · Candle BERT · cosine search     │
│  cache persistence · incremental rebuild                    │
└──────────────────────────────────────────────────────────────┘
```

The binary crate (`eidolon-cli`) sits at the top and depends on six library crates. The `runtime` crate is the shared foundation — it owns config resolution, session persistence, permission enforcement, and the conversation loop. The `indexing` crate provides a self-contained embedding pipeline that the runtime wraps with a background thread and the tools crate exposes as the `semantic_search` tool. This layered design means any crate can be replaced or extended independently without modifying the core conversation engine.

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
    │       ├── conversation.rs # Turn execution loop + auto-context injection
    │       ├── indexer.rs      # Background indexer thread + IndexHandle
    │       ├── session.rs      # Session persistence (JSONL)
    │       ├── session_control.rs # Session lifecycle management
    │       ├── permissions.rs  # Permission evaluation
    │       ├── permission_enforcer.rs # Per-tool-call permission enforcement
    │       ├── policy_engine.rs # Git-aware permission policy evaluation
    │       ├── prompt.rs       # System prompt assembly
    │       ├── oauth.rs        # PKCE OAuth + credential storage
    │       ├── hooks.rs        # Pre/post tool-use hook execution
    │       ├── bash.rs         # Shell command execution
    │       ├── bash_validation.rs # Shell command safety validation
    │       ├── bootstrap.rs    # Runtime startup and initialization
    │       ├── sandbox.rs      # Container detection (Docker/Podman)
    │       ├── compact.rs      # Token-aware session compaction
    │       ├── summary_compression.rs # Token-aware session summarization
    │       ├── file_ops.rs     # read_file, write_file, edit_file
    │       ├── json.rs         # JSON parsing utilities
    │       ├── mcp.rs          # MCP module root
    │       ├── mcp_client.rs   # MCP transport (stdio, websocket, remote)
    │       ├── mcp_stdio.rs    # MCP stdio subprocess management
    │       ├── mcp_tool_bridge.rs # Bridge MCP tools into the tool executor
    │       ├── mcp_lifecycle_hardened.rs # MCP server lifecycle validation
    │       ├── lsp_client.rs   # Language Server Protocol client
    │       ├── plugin_lifecycle.rs # Plugin health and discovery
    │       ├── branch_lock.rs  # Git branch locking
    │       ├── stale_branch.rs # Git branch freshness checks
    │       ├── green_contract.rs # Contract enforcement
    │       ├── trust_resolver.rs # Trust and permission resolution
    │       ├── recovery_recipes.rs # Automatic error recovery strategies
    │       ├── remote.rs       # Remote execution support
    │       ├── sse.rs          # Server-sent events for runtime
    │       ├── usage.rs        # Token usage tracking
    │       ├── task_packet.rs  # Structured task/work packets
    │       ├── task_registry.rs # Task tracking registry
    │       ├── lane_events.rs  # Lane/workflow event tracking
    │       ├── worker_boot.rs  # Worker bootstrap and lifecycle
    │       ├── team_cron_registry.rs # Team/worker scheduling
    │       ├── syndicate_collection.rs # Syndicate collection discovery
    │       ├── syndicate_memory.rs # Session-scoped shared memory
    │       └── syndicate_orchestrator.rs # Multi-agent orchestration
    ├── commands/               # Slash command registry and dispatch
    │   └── src/lib.rs          # Command specs, validation, skill resolution
    ├── tools/                  # Tool implementations
    │   └── src/
    │       ├── lib.rs          # Tool registry, bash/file/agent/skill execution
    │       └── lane_completion.rs # Git lane workflow support
    ├── indexing/               # Semantic workspace indexing
    │   └── src/
    │       ├── lib.rs          # Crate root + re-exports
    │       ├── types.rs        # ChunkMeta, IndexConfig, SearchResult
    │       ├── discovery.rs    # Gitignore-aware file walker
    │       ├── chunker.rs      # Sliding-window line chunker
    │       ├── model.rs        # HuggingFace model download + Candle BERT load
    │       ├── embedder.rs     # Batch embedding inference + L2 normalization
    │       ├── search.rs       # Brute-force cosine similarity search
    │       ├── cache.rs        # Bincode index persistence (atomic writes)
    │       └── index.rs        # Incremental index builder
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
├── tools        ← api, commands, runtime, plugins, indexing
├── plugins      ← (standalone — only serde)
├── runtime      ← plugins, indexing
├── indexing     ← candle, tokenizers, hf-hub, ignore
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

1. **Built-in tools**: `bash`, `read_file`, `write_file`, `edit_file`, `glob_search`, `grep_search`, `semantic_search`, `Skill`, `Agent`, `SyndicateMemoryWrite`, `SyndicateMemoryRead`, `SyndicateMemoryLog`, `SyndicateMemorySearch`, `TodoRead`, `TodoWrite`
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

## Semantic Indexing

PR #3 added a local embedding-based codebase indexer that runs entirely in-process via the Candle ML framework (pure Rust, no Python or external services).

### Architecture

```
  startup (once per process)
    ├── ConfigLoader reads indexing config from .eidolon.json
    ├── ensure_indexer_started() — OnceLock guard, idempotent
    │     ├── start_background_indexer() — spawns OS thread
    │     │     ├── ensure_model() — download all-MiniLM-L6-v2 via hf-hub
    │     │     ├── load_model() — BertModel from safetensors
    │     │     ├── load_cache() — attempt warm start from bincode
    │     │     ├── build_index() — discover → chunk → hash → embed
    │     │     ├── save_cache() — atomic write to .eidolon/.index-cache/
    │     │     └── publish embedder + index via OnceLock
    │     └── init_index_handle() — store in tools global
    └── IndexHandle cloned into ConversationRuntime

  per turn
    ├── build_auto_context(user_message)
    │     ├── embed user message via IndexHandle
    │     ├── cosine similarity search (top-K, threshold 0.3)
    │     └── inject <codebase_context> into system prompt
    └── model can call semantic_search tool for explicit queries
```

### Key Design Decisions

- **OS thread, not async** — Candle inference is CPU-bound; running on a dedicated thread avoids starving the tokio runtime
- **OnceLock publishing** — the embedder is published before the index so that `is_ready()` never returns true when `query()` would fail
- **Incremental rebuilds** — each file is SHA-256 hashed; unchanged files reuse cached embeddings
- **Budget-capped injection** — auto-context is limited to 8000 characters to avoid overwhelming the system prompt
- **Graceful degradation** — if indexing is disabled, the model fails, or the index is still building, all code paths return `None` / fallback messages
- **Model-agnostic pipeline** — the `modelId` config key accepts any HuggingFace BERT-compatible model. Vector dimensions adapt automatically; the tokenizer loads from whatever model is specified. The default `all-MiniLM-L6-v2` (~80 MB) balances speed and quality, but larger models work transparently.

## Future Architecture: Eidolon Context Engine

The semantic indexing pipeline is the foundation for a more ambitious context management system planned as the next major architectural addition.

The **Eidolon Context Engine** replaces flat vector retrieval with a structured, tiered virtual filesystem:

```
eidolon://
├── resources/              # Project docs, repos, data sources
│   └── my_project/
│       ├── .abstract       # L0: ~100 tokens — quick relevance check
│       ├── .overview        # L1: ~2K tokens — structure and key points
│       └── src/             # L2: full content — loaded on demand
├── user/                   # User preferences, working patterns
│   └── memories/
└── agent/                  # Agent skills, instructions, task memories
    ├── skills/
    └── memories/
```

Key architectural properties:

- **Tiered loading (L0/L1/L2)** — agents start at one-sentence abstracts, drill to overviews for planning, and only load full content when genuinely needed. Dramatically reduces token consumption for broad tasks.
- **Directory recursive retrieval** — multi-phase search that locks the highest-scoring directory, refines within it, and recurses through subdirectories. Finds context with full structural awareness.
- **Visualized retrieval trajectories** — every retrieval produces an inspectable trace of which directories were browsed and why, making context management a debuggable system rather than a black box.
- **Automatic session memory** — at session boundaries, the engine extracts user preferences and agent experience, updating memory directories so the agent improves with use.

The existing indexing crate becomes the L2 ingestion layer. The permission system governs `eidolon://` path access. Syndicate agents share context through the same filesystem. Plugin hooks fire on context operations. The current architecture is designed to absorb this cleanly.

See the [Roadmap](../ROADMAP.md) for the full plan.
