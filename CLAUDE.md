# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Detected stack
- Languages: Rust.
- Frameworks: none detected from the supported starter markers.

## Verification
- Run Rust verification from `rust/`: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`

## Repository shape
- `rust/` contains the Rust workspace and active CLI/runtime implementation (8 crates).
- `docs/` contains all project documentation (architecture, setup, usage, config, development, plugins).
- `src/` + `tests/` are a companion Python reference workspace, not the primary runtime.

## Documentation
- See `docs/architecture.md` for system design and crate map.
- See `docs/development.md` for contributing and extension patterns.
- See `docs/configuration.md` for config file format and env vars.

## Working agreement
- Prefer small, reviewable changes and keep generated bootstrap files aligned with actual repo workflows.
- Keep shared defaults in `.eidolon.json`; reserve `.eidolon/settings.local.json` for machine-local overrides.
- Do not overwrite existing `CLAUDE.md` content automatically; update it intentionally when repo workflows change.
