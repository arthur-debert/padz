# padz (library)

Core library for padz - a fast, project-aware scratch pad.

This crate provides the UI-agnostic business logic for padz. It includes:

- **API Layer** (`api.rs`): Thin facade over commands with input normalization
- **Command Layer** (`commands/`): Pure business logic for all operations
- **Store Layer** (`store/`): Storage abstraction with FileStore and InMemoryStore implementations
- **Model** (`model.rs`): Core data types (Pad, Metadata, Scope)

## Architecture

Everything in this crate is UI-agnostic:
- Functions take normal Rust arguments and return normal Rust types
- No stdout/stderr writes
- No `std::process::exit` calls
- No terminal assumptions

This enables the same core to serve CLI, web API, or any other UI.

## Usage

See the main [padz repository](https://github.com/arthur-debert/padz) for full documentation.

For the CLI tool, install `padz-cli` instead:

```bash
cargo install padz-cli
```
