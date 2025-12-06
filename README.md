# padz

A fast, project-aware scratch pad for the command line.

## Installation

```bash
cargo install padz
```

## Usage

```bash
# Create a new pad
padz create "My note title"
padz n "Quick note"

# List all pads
padz list
padz ls

# View a pad
padz view 1
padz v 1

# Edit a pad
padz edit 1
padz e 1

# Delete a pad
padz delete 1
padz rm 1

# Pin/unpin pads
padz pin 1
padz unpin p1

# Search pads
padz search "query"

# Use global pads (shared across projects)
padz -g list
padz --global create "Global note"
```

## Shell Completions

Generate shell completions:

```bash
# Bash
padz completions bash >> ~/.bashrc

# Zsh
padz completions zsh >> ~/.zshrc
```

## Features

- Project-aware: pads are stored per-project by default
- Global pads: use `-g` flag for cross-project notes
- Pin important pads for quick access
- Full-text search across all pads
- Dynamic shell completion with pad titles

## License

MIT
