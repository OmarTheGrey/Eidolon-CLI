# Setup Guide

Get the Eidolon agent runtime built, authenticated, and running. This covers everything from source checkout to your first conversation turn.

## Prerequisites

- **Rust toolchain**: Install via [rustup](https://rustup.rs/) — edition 2021 or later
- **Git**: Required for branch-aware features and lane completion
- **Authentication**: One of the following:
  - `ANTHROPIC_API_KEY` environment variable
  - OAuth login via `eidolon login`

### Optional

- **Python 3**: Only if you plan to run parity harness scripts
- **Docker or Podman**: For sandboxed development (see [Container Workflow](#container-workflow))

## Build from Source

```bash
git clone https://github.com/OmarTheGrey/eidolon-cli.git
cd eidolon-cli/rust
cargo build --workspace
```

The binary is produced at `rust/target/debug/eidolon-cli`. For a release build:

```bash
cargo build --workspace --release
# Binary at rust/target/release/eidolon-cli
```

## Verify the Build

```bash
./target/debug/eidolon-cli --version
./target/debug/eidolon-cli --help
```

Run the test suite:

```bash
cargo test --workspace
```

Then start the REPL and run the built-in doctor check:

```bash
./target/debug/eidolon-cli
# Inside the REPL:
/doctor
```

`/doctor` runs preflight diagnostics — it validates authentication, config resolution, sandbox state, and plugin health. Run it after every fresh build to confirm the runtime is correctly wired.

## Authentication

### API Key (quickest)

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

### OAuth Login

```bash
cd rust
./target/debug/eidolon-cli login
```

This opens a browser for Anthropic's OAuth consent flow, then saves tokens to `~/.eidolon/credentials.json`. To log out:

```bash
./target/debug/eidolon-cli logout
```

### Environment Variables

| Variable | Purpose |
|---|---|
| `ANTHROPIC_API_KEY` | Anthropic API key (preferred for automation) |
| `ANTHROPIC_AUTH_TOKEN` | Bearer token (alternative to API key) |
| `ANTHROPIC_BASE_URL` | Override API endpoint (for proxies or local services) |
| `EIDOLON_CONFIG_HOME` | Override config directory (default: `~/.eidolon`) |

## First Run

```bash
cd rust
./target/debug/eidolon-cli
```

This launches the interactive REPL — the primary interface for the agent runtime. You'll see the Eidolon banner and a prompt. Type a message or a slash command:

```
> /help          # list available commands
> /doctor        # run diagnostics
> /status        # show current session state
> explain the runtime crate
```

For a one-shot prompt without entering the REPL:

```bash
./target/debug/eidolon-cli prompt "summarize this repository"
```

## Project Initialization

To configure Eidolon for a specific project workspace:

```bash
./target/debug/eidolon-cli init
```

This scaffolds:
- `.eidolon.json` — project-level configuration (model, permissions, MCP servers, tool policies)
- `.gitignore` entries for `.eidolon/sessions/` and other runtime state

Once initialized, the runtime automatically merges project config with user-global and machine-local settings. See [Configuration](configuration.md) for the full resolution order.

## Container Workflow

The repository includes a `Containerfile` for Docker/Podman-based development.

### Build the Image

```bash
# Docker
docker build -t eidolon-dev -f Containerfile .

# Podman
podman build -t eidolon-dev -f Containerfile .
```

### Run Tests in Container

```bash
# Docker
docker run --rm -it \
  -v "$PWD":/workspace \
  -e CARGO_TARGET_DIR=/tmp/eidolon-target \
  -w /workspace/rust \
  eidolon-dev \
  cargo test --workspace

# Podman (add :Z for SELinux)
podman run --rm -it \
  -v "$PWD":/workspace:Z \
  -e CARGO_TARGET_DIR=/tmp/eidolon-target \
  -w /workspace/rust \
  eidolon-dev \
  cargo test --workspace
```

### Interactive Shell in Container

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  -e CARGO_TARGET_DIR=/tmp/eidolon-target \
  -w /workspace/rust \
  eidolon-dev
```

Inside the container, `eidolon-cli sandbox` reports container detection markers.

### Bind-Mount a Second Repository

To run Eidolon against another project while keeping the CLI workspace mounted:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  -v "$HOME/src/other-repo":/repo \
  -e CARGO_TARGET_DIR=/tmp/eidolon-target \
  -w /workspace/rust \
  eidolon-dev
```

Then: `cargo run -p eidolon-cli -- prompt "summarize /repo"`

### Notes

- `CARGO_TARGET_DIR=/tmp/eidolon-target` keeps container build artifacts out of your host's `target/`
- Podman on Fedora/RHEL needs the `:Z` suffix for SELinux relabeling
- Both Docker and Podman use the same `Containerfile`
