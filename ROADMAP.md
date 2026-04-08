# Roadmap

Eidolon is a **research-driven project** exploring whether agent runtime patterns generalize beyond software engineering. This roadmap reflects active research directions, not product commitments — priorities shift as experiments produce results.

## Current State

Eidolon ships today as a fully functional agent runtime for coding workflows:

- Interactive REPL with streaming markdown, syntax highlighting, and slash commands
- Multi-provider support (Anthropic Claude, OpenAI, xAI Grok)
- Complete tool suite with per-call permission enforcement
- Sub-agent orchestration with lifecycle tracking and spawn budgets
- Syndicate mode — a framework for defining any agent team topology with session-scoped shared memory
- Semantic workspace indexing with configurable BERT models (any HuggingFace model via Candle, pure Rust)
- Plugin hooks, MCP integration, skills system
- Session persistence with auto-compaction and structured JSONL output

Nine Rust crates, single binary, no runtime dependencies.

---

## Near-Term: Eidolon Context Engine

**Status:** Design complete, implementation next

The semantic indexing pipeline is the foundation. The Context Engine replaces flat "embed everything, vector-search, inject top-K" retrieval with a structured, tiered, observable context management system.

### 1. Filesystem Management Paradigm

Context becomes an abstract virtual filesystem under the `eidolon://` protocol.

```
eidolon://
├── resources/              # Project docs, repos, data sources
│   └── my_project/
│       ├── docs/
│       └── src/
├── user/                   # User preferences, habits, working patterns
│   └── memories/
│       ├── preferences/
│       └── patterns/
└── agent/                  # Agent skills, instructions, task memories
    ├── skills/
    ├── memories/
    └── instructions/
```

Every piece of context gets a unique URI and lives in a navigable directory hierarchy. Agents browse and manipulate context deterministically — `ls`, `find`, `cat` — like a developer navigating a codebase. This transforms context management from vague semantic matching into traceable, auditable "file operations."

### 2. Tiered Context Loading (L0/L1/L2)

Every context entry is automatically processed into three tiers on write:

- **L0 (Abstract):** ~100 tokens. One-sentence summary. Enough for "is this relevant at all?" without burning context budget.
- **L1 (Overview):** ~2,000 tokens. Structure, key points, usage scenarios. Enough for planning and decision-making.
- **L2 (Details):** Full original content. Loaded only when the agent genuinely needs deep access.

```
eidolon://resources/my_project/
├── .abstract               # L0 — quick relevance check
├── .overview               # L1 — structure and key points
├── docs/
│   ├── .abstract
│   ├── .overview
│   ├── api/
│   │   ├── .abstract
│   │   ├── .overview
│   │   ├── auth.md        # L2 — full content, loaded on demand
│   │   └── endpoints.md
│   └── ...
└── src/
    └── ...
```

The agent starts at L0, drills to L1 when something looks relevant, and only loads L2 when it actually needs the details. Token consumption drops dramatically for broad tasks.

### 3. Directory Recursive Retrieval

Multi-phase retrieval strategy:

1. **Intent analysis** — decompose query into multiple retrieval conditions
2. **Initial positioning** — vector retrieval locates the highest-scoring *directory* (not just chunk)
3. **Refined exploration** — secondary retrieval within that directory
4. **Recursive drill-down** — repeat refinement through subdirectories
5. **Result aggregation** — return context with full structural awareness

"Lock the high-scoring directory first, then refine within it" finds not just the best-matching fragment but understands where it lives — its siblings, its parent structure, the broader context around it.

### 4. Visualized Retrieval Trajectory

Every retrieval produces a full trajectory — which directories were browsed, which tiers were loaded, and why. The hierarchical filesystem structure makes this human-readable by default. When retrieval goes wrong, you debug it like code.

This is essential for non-coding use cases. When an accounting agent retrieves the wrong policy document, you need to know *why* — not just that it did.

### 5. Automatic Session Memory

At session boundaries, the engine analyzes task execution results and user feedback, then updates memory directories:

- **User memory** — preferences, communication patterns, recurring needs
- **Agent experience** — operational tips, tool usage patterns, domain heuristics from successful tasks

The agent gets measurably smarter with use through structured memory that persists, organizes, and surfaces itself at the right moment.

### Integration with Existing Architecture

The Context Engine integrates cleanly with the current system:

- The existing `indexing` crate becomes the L2 ingestion layer
- The permission system governs `eidolon://` path access
- Syndicate agents share context through the same virtual filesystem
- Plugin hooks fire on context operations (read, write, search)
- Session persistence captures context access patterns

---

## Medium-Term: Domain Generalization

**Status:** Research direction, not started

The runtime is domain-independent by design. These additions remove the remaining coding-specific assumptions:

### Domain-Agnostic Tool Registration

A declarative format (JSON or TOML) for defining tool suites that don't assume a software workspace. Non-developers should be able to author tool definitions for their domain without writing Rust.

```toml
[tool.lookup_invoice]
description = "Look up an invoice by number or date range"
permissions = "read-only"

[tool.lookup_invoice.parameters]
invoice_number = { type = "string", description = "Invoice number", optional = true }
date_from = { type = "string", description = "Start date (YYYY-MM-DD)", optional = true }
date_to = { type = "string", description = "End date (YYYY-MM-DD)", optional = true }
```

### Pluggable Prompt Strategies

The system prompt builder currently assumes a coding context. Generalizing this so different domains can inject their own prompt architecture — system context, role definitions, output format requirements — without forking the runtime.

### Workflow Templates

Syndicate collections generalized beyond coding agent teams:

- **Accounting close** — agents for data extraction, reconciliation, variance analysis, report generation
- **Content calendar** — research, drafting, review, scheduling agents with brand guideline skills
- **Compliance audit** — policy checker, evidence gatherer, report writer with read-only permissions
- **Hiring pipeline** — resume screening, criteria evaluation, coordination agents with HR-specific tools

Each template packages a collection definition, domain skills, tool configurations, and prompt strategies.

---

## Longer-Term: Runtime Capabilities

**Status:** Planned, dependent on medium-term progress

### Local Model Support

Ollama and llama.cpp integration through the existing provider abstraction. Critical for:
- Air-gapped operation in data-sensitive environments (healthcare, finance, government)
- Cost reduction for high-volume automated workflows
- Research with open-weight models

### GPU-Accelerated Indexing

Candle already supports CUDA. For document corpora larger than codebases — financial archives, legal document stores, research paper collections — GPU acceleration in the embedding pipeline becomes practical.

### Persistent Agent Teams

Currently, Syndicate runs are ephemeral — agents spin up, coordinate, and terminate. Persistent teams would maintain state across sessions, accumulating domain expertise through the Context Engine's memory system.

### Cross-Domain Agent Composition

A Syndicate run where agents from different domains collaborate — a coding agent generates a report template, a data agent fills it with financial data, a compliance agent reviews it. The permission system and shared memory already support this; the missing piece is domain-specific tool suites and prompt strategies.

---

## Non-Goals

- **SaaS product** — Eidolon is a personal research tool, open-sourced for the community. There are no plans for hosted services, pricing tiers, or managed infrastructure.
- **Framework lock-in** — every component is designed to be replaceable. If a better embedding library, ML runtime, or transport layer appears, swapping it should be straightforward.
- **Feature parity as an end goal** — Eidolon started by achieving parity with existing coding harnesses, but parity was a starting point, not a destination.

---

## Contributing

See [Development Guide](docs/development.md) for contributing patterns. The architecture is designed so that new tools, commands, providers, and plugins can be added without modifying the core conversation loop.

Research contributions — particularly around domain-specific tool definitions, prompt strategies, and Syndicate collection templates for non-coding domains — are especially welcome.
