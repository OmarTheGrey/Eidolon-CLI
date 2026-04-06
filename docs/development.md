# Development Guide

Guide for contributing to or extending the Eidolon agent runtime. The architecture is intentionally modular — adding a new tool, command, provider, or plugin should never require modifying the core conversation loop.

## Workspace Layout

The Rust workspace lives under `rust/` and contains 8 crates:

| Crate | Type | Purpose |
|---|---|---|
| `eidolon-cli` | Binary | CLI entry point, REPL, TUI rendering |
| `api` | Library | HTTP clients for Anthropic, OpenAI, xAI |
| `runtime` | Library | Core engine — config, sessions, permissions, MCP, OAuth |
| `commands` | Library | Slash command registry and dispatch |
| `tools` | Library | Tool implementations (bash, file ops, agents, skills) |
| `plugins` | Library | Plugin system — hooks, lifecycle, registry |
| `telemetry` | Library | Tracing, analytics, client identity |
| `mock-anthropic-service` | Binary | Mock Anthropic API for tests |

## Building

```bash
cd rust
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## Testing

```bash
# Full workspace tests
cargo test --workspace

# Individual crate
cargo test -p runtime
cargo test -p api
cargo test -p commands
cargo test -p tools

# Single test
cargo test -p tools -- skill_loads_local_skill_prompt

# With output
cargo test -p api -- --nocapture
```

### Mock Parity Harness

A deterministic mock Anthropic API server is used for integration tests:

```bash
cargo test -p eidolon-cli --test mock_parity_harness
```

The mock server (`mock-anthropic-service`) starts on a random port and returns scripted responses defined in `rust/mock_parity_scenarios.json`.

### Test Patterns

- **Unit tests** are in `#[cfg(test)]` modules within each crate's source files
- **Integration tests** live in `rust/crates/eidolon-cli/tests/`
- **Environment-sensitive tests** use an `env_lock()` mutex to serialize access to process-wide env vars
- **Path assertions** use `normalize_path_sep()` for cross-platform compatibility

## Adding a New Tool

Tools are one of Eidolon's primary extensibility surfaces. Every tool goes through the same permission enforcement and plugin hook pipeline, so new tools inherit audit, validation, and control behavior automatically.

1. **Define the input/output types** in `rust/crates/tools/src/lib.rs`:

```rust
#[derive(Deserialize)]
struct MyToolInput {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}
```

2. **Implement the execution function**:

```rust
fn run_my_tool(input: MyToolInput) -> Result<String, String> {
    // implementation
    Ok(serde_json::to_string(&result).unwrap())
}
```

3. **Register in the tool dispatch** — add a match arm in `execute_tool_with_enforcer()`:

```rust
"my_tool" => {
    maybe_enforce_permission_check(enforcer, name, input)?;
    from_value::<MyToolInput>(input).and_then(run_my_tool)
}
```

4. **Add the tool spec** to the tool manifest (in `tool_specs()`):

```rust
ToolSpec {
    name: "my_tool",
    description: "Does something useful",
    input_schema: json!({
        "type": "object",
        "properties": {
            "query": { "type": "string", "description": "Search query" },
            "limit": { "type": "integer", "description": "Max results" }
        },
        "required": ["query"]
    }),
}
```

5. **Write tests** in the `#[cfg(test)]` module at the bottom of `lib.rs`.

## Adding a Slash Command

Slash commands extend the REPL interface. They're registered declaratively — add a spec and wire up a handler.

1. **Add the spec** in `rust/crates/commands/src/lib.rs` to the `slash_command_specs()` array:

```rust
SlashCommandSpec {
    name: "mycommand",
    aliases: &["mc"],
    summary: "Does something in the REPL",
    argument_hint: Some("[args]"),
    resume_supported: false,
},
```

2. **Implement the handler** in `commands/src/lib.rs` or in `eidolon-cli/src/main.rs` depending on whether it needs runtime state.

3. **Wire up dispatch** in the REPL's slash command handler in `main.rs`.

## Adding a Provider

Providers abstract LLM API communication. Eidolon's provider system is designed so that new LLM backends can be added without touching the conversation loop or tool execution pipeline.

Providers implement the `Provider` trait in `rust/crates/api/src/providers/mod.rs`:

```rust
pub trait Provider {
    type Stream;
    fn send_message<'a>(&'a self, request: &'a MessageRequest) -> ProviderFuture<'a, MessageResponse>;
    fn stream_message<'a>(&'a self, request: &'a MessageRequest) -> ProviderFuture<'a, Self::Stream>;
}
```

1. Create a new file in `api/src/providers/` (e.g., `my_provider.rs`)
2. Implement `Provider` for your client struct
3. Add the variant to `ProviderKind` enum
4. Register it in `detect_provider_kind()` and model resolution

## Adding a Plugin

Plugins are directories with a `.claude-plugin/plugin.json` manifest:

```json
{
  "name": "my-plugin",
  "version": "0.1.0",
  "description": "What this plugin does",
  "defaultEnabled": true,
  "hooks": {
    "PreToolUse": ["./hooks/pre.sh"],
    "PostToolUse": ["./hooks/post.sh"]
  }
}
```

Hook scripts receive JSON on stdin with the tool name, input, and context. They can modify behavior or log events.

See the [Plugin System](plugins.md) doc for details.

## Runtime Extension Points

### Config Sources

Add new config sources in `runtime/src/config.rs` by extending `ConfigLoader::load()`.

### Permission Policies

Custom permission logic goes in `runtime/src/permissions.rs` and `runtime/src/permission_enforcer.rs`.

### MCP Transports

New MCP transport types are added in `runtime/src/mcp_client.rs`. Implement the transport and add a variant to the server config enum.

### Session Formats

Session serialization lives in `runtime/src/session.rs`. The primary format is JSONL with legacy JSON support.

### System Prompt

The system prompt is assembled in `runtime/src/prompt.rs` from project context, instruction files, and tool descriptions.

## Code Style

- **Edition**: Rust 2021
- **Unsafe**: Forbidden (`#![forbid(unsafe_code)]`)
- **Lints**: Clippy pedantic with `module_name_repetitions` allowed
- **Formatting**: `cargo fmt` — standard rustfmt
- **Error handling**: `Result<T, String>` for tool functions, typed errors (`ApiError`, `RuntimeError`, `ConfigError`) for library code
- **Testing**: Inline `#[cfg(test)]` modules, integration tests in `tests/` directories

## Verification Checklist

Before submitting changes:

```bash
cd rust
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
