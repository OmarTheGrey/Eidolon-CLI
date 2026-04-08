# Eidolon

<p align="center">
  <img src="assets/eidolon-hero.png" alt="Eidolon" width="600" />
</p>

<p align="center">
  <strong>A fully extensible AI coding agent harness — built to be embedded, extended, and composed.</strong>
</p>


---

Eidolon is an agentic coding system built from the ground up in Rust, inspired by Copilot-CLI, ClaudeCode and Opencode. It provides an interactive terminal interface for LLM-driven coding workflows, with a modular architecture designed around extensible **tools**, **skills**, **plugins**, and **sub-agents**.

Unlike a simple chat wrapper, Eidolon is built as an **agent runtime** — a system where every internal mechanism (tool execution, permission enforcement, session management, prompt construction, MCP integration) is exposed as a composable, overridable surface. The goal is a coding harness that can be embedded into larger autonomous systems, orchestration pipelines, and multi-agent architectures.

This project is built following the **Regula Framework** — a set of agentic design patterns focused on structured autonomy, deterministic recovery, and machine-first interfaces for AI agent coordination.
- [![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/OmarTheGrey/Eidolon-CLI)

## What Eidolon Does

- **Interactive REPL** with streaming markdown rendering, syntax highlighting, and slash commands
- **Tool execution** — bash, file read/write/edit, glob, grep, with permission enforcement per call
- **Skills system** — reusable prompt templates discovered from project directories, shareable across sessions
- **Sub-agents** — spawn independent agent workers for parallel tasks with isolated contexts
- **Syndicate mode** — launch pre-defined multi-agent collections for coordinated coding runs
- **Plugin hooks** — pre/post tool-use lifecycle hooks via shell scripts for audit, validation, or custom logic
- **MCP integration** — connect external tool servers over stdio, WebSocket, or HTTP transports
- **Semantic workspace indexing** — local Candle-based embeddings (all-MiniLM-L6-v2) for codebase-aware search and automatic context injection
- **Session persistence** — JSONL-based conversation history with token-aware automatic compaction
- **Multi-provider support** — Anthropic Claude, OpenAI, and xAI Grok via a unified provider abstraction
- **OAuth & API key auth** — built-in PKCE OAuth flow or simple environment variable auth
- **JSON output mode** — every command supports `--output-format json` for programmatic consumption

## Why It Exists

Eidolon is designed to be the **foundation layer** for larger projects — not just a standalone tool. Its internals are structured so that:

- Tools, commands, and providers can be added without touching the core conversation loop
- The permission system, config resolution, and prompt construction are all independently extensible
- Sessions, compaction, and state management are transparent and machine-readable
- The plugin and hook system allows external processes to observe and modify tool execution
- Everything outputs structured data suitable for orchestration by other agents or systems

This makes Eidolon suitable as the coding worker in multi-agent setups, CI/CD pipelines, or any context where an autonomous coding assistant needs to be controlled, observed, and composed with other systems.

## Quick Start

```bash
cd rust
cargo build --workspace
./target/debug/eidolon-cli --help
```

Authenticate and run:

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
./target/debug/eidolon-cli prompt "summarize this repository"
```

Or start the interactive REPL:

```bash
./target/debug/eidolon-cli
```

Run `/doctor` as your first command — it validates auth, config, plugins, and sandbox state.

## Syndicate Mode

PR #1 introduced a first-class multi-agent orchestration path called **Syndicate**.

Use it from CLI:

```bash
# list available collections
./target/debug/eidolon-cli syndicate --list

# run a collection with default task
./target/debug/eidolon-cli syndicate feature-build

# run a collection with an explicit task
./target/debug/eidolon-cli syndicate feature-build "Implement retries and update tests"
```

Or from the REPL with `/syndicate`.

Syndicate runs now include:

- Session-scoped shared memory (`SyndicateMemoryWrite/Read/Log/Search`)
- Structured per-agent progress and summary output
- Spawn safety guards (session spawn cap and Syndicate recursion protection)
- Correct failure accounting when an individual agent spawn fails

## Semantic Workspace Indexing

PR #3 introduced a **local semantic indexing system** that gives the agent deep codebase awareness without external services.

When enabled, a background thread downloads and runs the `all-MiniLM-L6-v2` sentence transformer model (~80 MB) via [Candle](https://github.com/huggingface/candle) (pure Rust, no Python required). It walks the workspace, chunks source files, and builds a 384-dimensional vector index that supports:

- **`semantic_search` tool** — the model can query the index for code snippets semantically related to any natural language query
- **Auto-context injection** — each conversation turn automatically embeds the user's message and injects the most relevant code snippets into the system prompt, so the agent starts every response with workspace awareness
- **Incremental rebuilds** — file content hashes allow the index to skip unchanged files on restart
- **Disk caching** — the built index is persisted to `.eidolon/index/` for fast warm starts

Enable it in `.eidolon.json`:

```json
{
  "indexing": {
    "enabled": true
  }
}
```

See [Configuration](docs/configuration.md) for the full set of indexing options.

## Documentation

| Document | Description |
|---|---|
| [Setup Guide](docs/setup-guide.md) | Installation, build, authentication, containers |
| [Usage Guide](docs/usage.md) | CLI flags, REPL commands, sessions, tools, workflows |
| [Architecture](docs/architecture.md) | Crate map, data flows, system design |
| [Configuration](docs/configuration.md) | Config files, environment variables, MCP servers |
| [Development](docs/development.md) | Contributing, testing, adding tools/commands/providers |
| [Plugin System](docs/plugins.md) | Hooks, lifecycle, creating plugins |

## Repository Layout

```
rust/                    # Rust workspace
├── crates/
│   ├── eidolon-cli/     # Binary — CLI, REPL, TUI rendering
│   ├── api/             # LLM provider HTTP clients
│   ├── runtime/         # Core engine — config, sessions, MCP, permissions
│   ├── commands/        # Slash command registry
│   ├── tools/           # Tool implementations (bash, file ops, agents)
│   ├── indexing/        # Semantic workspace indexing (Candle embeddings)
│   ├── plugins/         # Plugin system and hooks
│   ├── telemetry/       # Tracing and analytics
│   └── mock-anthropic-service/  # Mock API for testing
docs/                    # Documentation
```

## Acknowledgments

Eidolon is built on top of excellent open-source Rust crates:

| Crate | Purpose |
|---|---|
| [tokio](https://crates.io/crates/tokio) | Async runtime |
| [reqwest](https://crates.io/crates/reqwest) | HTTP client with TLS |
| [serde](https://crates.io/crates/serde) / [serde_json](https://crates.io/crates/serde_json) | Serialization framework |
| [crossterm](https://crates.io/crates/crossterm) | Terminal manipulation |
| [rustyline](https://crates.io/crates/rustyline) | Line editing with history and completion |
| [syntect](https://crates.io/crates/syntect) | Syntax highlighting |
| [pulldown-cmark](https://crates.io/crates/pulldown-cmark) | Markdown parsing |
| [regex](https://crates.io/crates/regex) | Regular expressions |
| [sha2](https://crates.io/crates/sha2) | SHA-2 hashing |
| [glob](https://crates.io/crates/glob) | File pattern matching |
| [walkdir](https://crates.io/crates/walkdir) | Recursive directory traversal |
| [candle](https://crates.io/crates/candle-core) | Pure-Rust ML inference (BERT embeddings) |
| [tokenizers](https://crates.io/crates/tokenizers) | HuggingFace tokenizer (WordPiece) |
| [hf-hub](https://crates.io/crates/hf-hub) | HuggingFace model downloads |

## Author

Built by [OmarTheGrey](https://github.com/OmarTheGrey) using the Regula Framework.

## Disclaimer

This project is inspired by Anthropic's Claude Code. It is **not affiliated with, endorsed by, or maintained by Anthropic**.
