# padz-cli

Command-line interface for padz - a fast, project-aware scratch pad.

## Installation

```bash
cargo install padz-cli
```

The binary is installed as `padz`.

## Usage

```bash
# Create a new pad
padz create "Meeting Notes"

# List all pads
padz

# View a pad
padz view 1

# Search pads
padz search "keyword"
```

## Shell Completions

Enable tab completion for commands, options, and pad titles:

```bash
# Bash - add to ~/.bashrc
eval "$(padz completions bash)"

# Zsh - add to ~/.zshrc
eval "$(padz completions zsh)"
```

## Architecture

This crate is a thin CLI wrapper around the `padzapp` library. It handles:

- Argument parsing (clap)
- Terminal I/O (stdout/stderr)
- Output formatting and styling
- Dynamic shell completions (clap_complete)

All business logic lives in the `padzapp` library crate.

## Documentation

See the main [padz repository](https://github.com/arthur-debert/padz) for full documentation.
