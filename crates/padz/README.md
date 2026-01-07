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

## Architecture

This crate is a thin CLI wrapper around the `padz` library. It handles:

- Argument parsing (clap)
- Terminal I/O (stdout/stderr)
- Output formatting and styling
- Shell completion scripts

All business logic lives in the `padz` library crate.

## Documentation

See the main [padz repository](https://github.com/arthur-debert/padz) for full documentation.
