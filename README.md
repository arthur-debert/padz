# padz

A terminal app for creating and managing scratch notes that is a good Unix citizen, using your `$EDITOR` and keeping its data in plain text files.

Designed for maximum ergonomics with a clever ID system for less typing, advanced completion for IDs and titles, batch editing, import/export, nested notes, soft delete, search, tags, and file type (txt, markdown) support.

Context-aware: automatically separates global pads (in your data dir) from project pads (in Git repos), providing sensible project-related and general notes.

Sports a smart-looking UI with dark/light mode awareness.

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

# Tags
padz tags create feature
padz add-tag 1 --tag feature
padz list --tag feature

# Use global pads (shared across projects)
padz -g list
padz --global create "Global note"
```

## Shell Completions

Enable tab completion for commands, options, and pad titles:

```bash
# Bash - add to ~/.bashrc
eval "$(padz completions bash)"

# Zsh - add to ~/.zshrc
eval "$(padz completions zsh)"
```

Completions include:
- All commands and their aliases (`view`/`v`, `edit`/`e`, etc.)
- Command options (`--global`, `--deleted`, `--peek`, etc.)
- Pad indexes (`1`, `2`, `p1`, `d1`, etc.)
- Pad titles for quick lookup

## Features

- **Unix-friendly**: uses your `$EDITOR`, stores data as plain text files
- **Context-aware**: project pads (in Git repos) and global pads (in data dir)
- **Ergonomic**: clever ID system, advanced completion for IDs and titles
- **Organized**: tags, pinning, nested notes, soft delete
- **Powerful**: full-text search, batch editing, import/export
- **File types**: supports txt and markdown
- **Smart UI**: dark/light mode awareness

## License

MIT
