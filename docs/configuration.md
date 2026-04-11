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
| `EIDOLON_PROFILE` | Activate a named profile — resolves config home to `~/.eidolon/profiles/<name>/` |
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

## Indexing

Controls the local semantic workspace indexer. When enabled, Eidolon downloads a small sentence transformer model and builds a vector index of your codebase in the background. This powers two features: the `semantic_search` tool (explicit model queries) and automatic context injection (transparent snippets in the system prompt).

```json
{
  "indexing": {
    "enabled": true,
    "modelId": "sentence-transformers/all-MiniLM-L6-v2",
    "chunkLines": 50,
    "overlapLines": 10,
    "maxFileSizeBytes": 524288,
    "autoContextTopK": 5,
    "autoContextEnabled": true,
    "cacheDir": ".eidolon/.index-cache",
    "excludedExtensions": [
      "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "wasm", "lock",
      "min.js", "min.css", "map", "bin", "exe", "dll", "so", "dylib", "o", "a",
      "pyc", "class", "jar", "zip", "tar", "gz", "bz2", "7z", "rar", "pdf",
      "doc", "docx"
    ]
  }
}
```

### Indexing Keys

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `false` | Enable the background indexer. When false, the `semantic_search` tool is unavailable and auto-context is skipped. |
| `modelId` | string | `"sentence-transformers/all-MiniLM-L6-v2"` | HuggingFace model identifier. The model is downloaded on first run (~80 MB) and cached locally. |
| `chunkLines` | integer | `50` | Number of lines per chunk when splitting source files. |
| `overlapLines` | integer | `10` | Overlap between adjacent chunks (improves retrieval of code near chunk boundaries). |
| `maxFileSizeBytes` | integer | `524288` (512 KB) | Files larger than this are skipped during indexing. |
| `autoContextTopK` | integer | `5` | Number of top matching chunks to inject into the system prompt per turn. |
| `autoContextEnabled` | boolean | `true` | Whether to automatically inject codebase context into the system prompt. Set to `false` to only use the explicit `semantic_search` tool. |
| `cacheDir` | string | `".eidolon/.index-cache"` | Directory for the persisted index cache (relative to workspace root). |
| `excludedExtensions` | string[] | *(see above)* | File extensions to skip during discovery. Supports compound extensions like `"min.js"`. |

### How It Works

1. **Model download**: On first run, the model weights (~80 MB) are downloaded from HuggingFace Hub and cached in the system's HF cache directory.
2. **File discovery**: The indexer walks the workspace using the `ignore` crate (respects `.gitignore`), skipping binary files, oversized files, and excluded extensions.
3. **Chunking**: Each source file is split into overlapping windows of `chunkLines` lines. Each chunk includes a `// File: <path>` header for context.
4. **Embedding**: Chunks are tokenized and passed through the BERT model in batches of 32. Output vectors are mean-pooled and L2-normalized to 384 dimensions.
5. **Caching**: The index is serialized via bincode and written atomically to `cacheDir`. On restart, unchanged files (identified by SHA-256 hash) reuse cached embeddings.
6. **Search**: Queries are embedded with the same model and compared against all chunk vectors via dot product (cosine similarity for unit vectors). Results above a 0.3 threshold are returned.

### Disabling Auto-Context

If you want the `semantic_search` tool available to the model but don't want implicit system prompt injection:

```json
{
  "indexing": {
    "enabled": true,
    "autoContextEnabled": false
  }
}
```

### Performance Notes

- The initial index build for a large workspace (10K+ files) may take 30–60 seconds depending on CPU. Subsequent warm starts (from cache) are much faster since only changed files are re-embedded.
- Embedding inference runs on a dedicated OS thread and does not block the REPL or the async runtime.
- The model runs on CPU. No GPU is required.
