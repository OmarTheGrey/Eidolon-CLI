# Eidolon

<p align="center">
  <img src="assets/eidolon-hero.png" alt="Eidolon" width="600" />
</p>

<p align="center">
  <strong>A research-driven, general-purpose agent runtime — built in Rust to be embedded, extended, and pointed at any domain.</strong>
</p>

<p align="center">
  <a href="docs/setup-guide.md">Setup</a> · <a href="docs/usage.md">Usage</a> · <a href="docs/architecture.md">Architecture</a> · <a href="docs/configuration.md">Config</a> · <a href="ROADMAP.md">Roadmap</a> · <a href="docs/development.md">Contributing</a>
</p>

---

Eidolon is a **model-agnostic agent runtime** built from scratch in Rust. It starts as a coding harness — interactive REPL, tool execution, multi-agent orchestration — but is architecturally designed to generalize beyond software engineering into any knowledge-work domain.

Every internal mechanism (tool dispatch, permission enforcement, session management, prompt construction, MCP integration, semantic indexing) is exposed as a composable, overridable surface. The runtime doesn't know or care whether its tools manipulate source code, financial documents, or marketing assets. What the agent *does* is a configuration detail. What the runtime *provides* — structured tool execution, observable state, coordinated multi-agent workflows, and tiered context management — is the constant.

This is a **personal research project** exploring what happens when you take the patterns that work for coding agents and point them at everything else. Coding is the proving ground. A general-purpose agent harness is the destination.

Built following the **Regula Framework** — a set of agentic design patterns focused on structured autonomy, deterministic recovery, and machine-first interfaces.

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/OmarTheGrey/Eidolon-CLI)

## What Eidolon Does

### Agent Runtime Core
- **Interactive REPL** with streaming markdown rendering, syntax highlighting, and slash commands
- **Tool execution** — bash, file read/write/edit, glob, grep, with permission enforcement per call
- **Session persistence** — JSONL-based conversation history with token-aware automatic compaction
- **Multi-provider support** — Anthropic Claude, OpenAI, and xAI Grok via a unified provider abstraction
- **JSON output mode** — every command supports `--output-format json` for programmatic consumption
- **OAuth & API key auth** — built-in PKCE OAuth flow or simple environment variable auth

### Multi-Agent Orchestration
- **Sub-agents** — spawn independent agent workers for parallel tasks with isolated contexts
- **Syndicate mode** — define and run *any* agent team topology with session-scoped shared memory (built-in collections are demos — the framework is the point)
- **Shared memory coordination** — key-value writes, reads, append logs, and full-text search across all agents in a run

### Extensibility
- **Skills system** — reusable prompt templates discovered from project directories, shareable across sessions
- **Plugin hooks** — pre/post tool-use lifecycle hooks via shell scripts for audit, validation, or custom logic
- **MCP integration** — connect external tool servers over stdio, WebSocket, or HTTP transports

### Intelligence
- **Semantic workspace indexing** — local Candle-based embeddings with *any* HuggingFace BERT model for codebase-aware search and automatic context injection
- **Auto-context injection** — every turn silently embeds the user's message and injects top matching code into the system prompt

## Why It Exists

Eidolon exists to answer a research question: **can the patterns that make coding agents effective be generalized to any knowledge-work domain?**

The architecture is intentionally over-engineered for a coding assistant — because it isn't *just* a coding assistant. Every design decision optimizes for composability and domain-independence:

- **Tool dispatch is domain-agnostic** — the runtime dispatches tools, enforces permissions, and runs hooks. Whether those tools manipulate source code, financial records, or HR documents is a configuration detail.
- **Syndicate mode generalizes** — agent team topologies aren't limited to "debugger + tester + reviewer." The framework supports any collection of specialized agents with any coordination pattern. A hiring pipeline, an accounting close, a content calendar — they're all just collection definitions.
- **The permission system scales to regulated domains** — three enforcement modes (read-only, workspace-write, full-access) with per-tool-call evaluation, audit hooks, and session persistence create a reviewable trail suitable for compliance-sensitive workflows.
- **Structured I/O everywhere** — JSON output on every command, machine-readable sessions, and structured tool results make Eidolon embeddable in larger orchestration systems, CI/CD pipelines, or other agent runtimes.

The immediate research directions beyond coding: **automated accounting workflows**, **HR process automation**, **marketing content operations**, and **compliance audit pipelines**. See the [Roadmap](ROADMAP.md) for the full plan.

This makes Eidolon suitable as the worker runtime in multi-agent setups, automation pipelines, or any context where an autonomous agent needs to be controlled, observed, and composed with other systems.

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

Syndicate is a **framework for defining agent team topologies** — not a fixed set of coding agents.

A Syndicate collection is a set of specialized agents, each with a role, system prompt, optional model override, and shared memory access. The built-in collections (`feature-build`, `code-review`, `debug-squad`) are **demos** that ship with the runtime to showcase the framework. The real value is defining your own:

```bash
# List available collections
./target/debug/eidolon-cli syndicate --list

# Run the built-in demo
./target/debug/eidolon-cli syndicate feature-build "Implement retries and update tests"
```

Define custom collections for any coordination pattern — a "migration pipeline" with a schema analyst, query rewriter, and test generator; a "security audit" team with a scanner, fix proposer, and reviewer; or any domain-specific agent team you need.

Syndicate runs include:
- Session-scoped shared memory (`SyndicateMemoryWrite/Read/Log/Search`) for coordination
- Structured per-agent progress and summary output
- Spawn safety guards (session spawn cap and recursion protection)

## Semantic Workspace Indexing

A background indexer gives the agent deep codebase awareness without external services.

When enabled, a dedicated OS thread downloads and runs a sentence transformer model via [Candle](https://github.com/huggingface/candle) (pure Rust ML, no Python, no GPU required). The **model is configurable** — any BERT-compatible model on HuggingFace works. The default `all-MiniLM-L6-v2` (~80 MB) is fast and capable; point `modelId` at a larger model when accuracy matters more than speed.

The pipeline walks the workspace, chunks source files into overlapping windows, batches through BERT, and builds an in-memory vector index with disk-backed caching and incremental rebuilds via SHA-256 content hashing.

This powers:
- **`semantic_search` tool** — the model can query the index for code semantically related to any natural language query
- **Auto-context injection** — each turn silently embeds the user's message and injects the most relevant chunks into the system prompt (budget-capped at 8,000 characters)

Enable in `.eidolon.json`:

```json
{
  "indexing": {
    "enabled": true,
    "modelId": "sentence-transformers/all-MiniLM-L6-v2"
  }
}
```

See [Configuration](docs/configuration.md) for the full set of indexing options.

## What's Next: The Eidolon Context Engine

The semantic indexing pipeline is a starting point. The immediate next step is a complete context management system that replaces flat "embed everything, vector-search, inject top-K" retrieval with something structurally aware and token-efficient.

The **Eidolon Context Engine** introduces five interlocking concepts:

| Concept | What It Solves |
|---|---|
| **Filesystem Management Paradigm** | Context becomes an abstract virtual filesystem under the `eidolon://` protocol — memories, resources, skills, and preferences all map to navigable directories with unique URIs. Agents browse and manipulate context deterministically, like a developer navigating a codebase. |
| **Tiered Context Loading** | Every piece of context is automatically processed into three tiers: **L0** (~100 tokens, one-sentence abstract), **L1** (~2K tokens, structural overview), **L2** (full content). The agent starts at L0, drills to L1 for planning, and only loads L2 when deep access is genuinely needed — dramatically reducing token consumption. |
| **Directory Recursive Retrieval** | Multi-phase retrieval that decomposes query intent, locks the highest-scoring *directory* (not just chunk), refines within it, and recurses through subdirectories. Finds not just the best-matching fragment but understands its structural context. |
| **Visualized Retrieval Trajectory** | Every retrieval produces a full trajectory — which directories were browsed, which tiers were loaded, and why. When retrieval goes wrong, you debug it like code, not guess. |
| **Automatic Session Memory** | At session boundaries, the engine analyzes results and feedback, then updates user preferences and agent experience memories. The agent gets measurably smarter with use. |

The Context Engine is the bridge between "coding assistant with good search" and "general-purpose agent runtime that can operate in any knowledge domain." See the [Roadmap](ROADMAP.md) for the full plan.

## Research Vision

Eidolon is a **research vehicle** exploring whether agent harness patterns generalize beyond software engineering.

Everything in the runtime — tool execution, permission enforcement, session persistence, multi-agent coordination, semantic indexing, hook pipelines, structured I/O — is domain-independent by design. The tool suite is coding-native *today*, but the architecture was built from the start to not be permanently married to `vim` and `cargo test`.

The concrete research directions:

- **Domain-agnostic tool registration** — declarative tool definitions (JSON/TOML) that non-developers can author
- **Pluggable prompt strategies** — domain-specific prompt architecture without forking the runtime
- **Workflow templates** — Syndicate collections generalized to accounting, HR, marketing, and compliance domains
- **Local model support** — Ollama and llama.cpp integration for air-gapped operation in data-sensitive environments
- **GPU-accelerated indexing** — Candle CUDA support for larger document corpora beyond codebases

## Documentation

| Document | Description |
|---|---|
| [Setup Guide](docs/setup-guide.md) | Installation, build, authentication, containers |
| [Usage Guide](docs/usage.md) | CLI flags, REPL commands, sessions, tools, workflows |
| [Architecture](docs/architecture.md) | Crate map, data flows, system design, research context |
| [Configuration](docs/configuration.md) | Config files, environment variables, MCP servers, indexing |
| [Development](docs/development.md) | Contributing, testing, adding tools/commands/providers |
| [Plugin System](docs/plugins.md) | Hooks, lifecycle, creating plugins |
| [Roadmap](ROADMAP.md) | Research directions, Context Engine, domain generalization |

## Repository Layout

```
rust/                    # Rust workspace (9 crates)
├── crates/
│   ├── eidolon-cli/     # Binary — CLI, REPL, TUI rendering
│   ├── api/             # LLM provider HTTP clients (Anthropic, OpenAI, xAI)
│   ├── runtime/         # Core engine — config, sessions, MCP, permissions, indexer
│   ├── commands/        # Slash command registry
│   ├── tools/           # Tool implementations (bash, file ops, agents, search)
│   ├── indexing/        # Semantic indexing (any BERT model via Candle, chunking, search, cache)
│   ├── plugins/         # Plugin system and hooks
│   ├── telemetry/       # Tracing and analytics
│   └── mock-anthropic-service/  # Mock API for testing
docs/                    # Documentation
ROADMAP.md               # Research directions and planned features
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

Built by [OmarTheGrey](https://github.com/OmarTheGrey) — a personal research tool, open-sourced because keeping it private felt selfish.

Eidolon is the successor to **NexforgeCLI** (deprecated), rebuilt from scratch in Rust with a generalized architecture. Built following the Regula Framework.

## Disclaimer

This project is inspired by Anthropic's Claude Code. It is **not affiliated with, endorsed by, or maintained by Anthropic**.
