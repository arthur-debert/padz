[] PADZ - Command-Line Note Taking

PADZ is a fast, context-aware command-line note-taking tool designed for developers who want quick pad notes without leaving their terminal.

## What is PADZ?

PADZ helps you quickly create, manage, and find pad notes (called "pad") directly from your command line. Think of it as a developer-friendly notepad that understands your project context.

### Key Features

- **Project-Aware**: Automatically associates notes with your Git projects
- **Global Notes**: Create notes that aren't tied to any specific project
- **Fast Access**: Simple commands to create, view, edit, and search notes
- **Pinning**: Keep important notes at the top of your list (up to 5)
- **Soft Delete**: Deleted notes can be recovered before permanent deletion
- **Search**: Full-text search across all your notes
- **Editor Integration**: Uses your `$EDITOR` for comfortable editing

## Core Concepts

### pad

A "pad" is a single note with:
- A **title**: Short description of the note
- **Content**: The actual note content
- **Metadata**: Creation time, update time, project association

### Scopes

PADZ operates in two scopes:

1. **Project Scope** (default): Notes associated with the current Git project
   - When you run `padz` inside a Git repository, notes are scoped to that project
   - Different projects have separate note lists
   - Useful for project-specific TODOs, ideas, snippets

2. **Global Scope** (via `-g` flag): Notes not tied to any project
   - Available from anywhere
   - Use for personal notes, general snippets, cross-project information
   - Access with `--global` or `-g` flag

Now, the scope only changes the path to the data store , nothing else.

### Project Detection

PADZ determines the current project by walking up the directory tree looking for a `.git` directory. If found, the project name is the basename of that directory. If no `.git` is found, you're in "global" scope.

### Indexing

pad are indexed by their display order (1, 2, 3, ...). The index changes as you create, delete, or pin notes:
- **Pinned notes** appear first with prefix `p1`, `p2`, etc.
- **Regular notes** follow, numbered sequentially
- **Deleted notes** (when shown) appear with prefix `d1`, `d2`, etc.

The index is NOT permanent - it's a display convenience. Internally, each pad has a unique UUID.

## Quick Start

```bash
# List all pad in current project
padz
padz list

# Create a new pad (opens $EDITOR)
padz create
padz new
padz n

# Create with a title
padz create "My note title"
padz "Quick note"

# Create with piped content
echo "TODO: Fix the bug" | padz new "Bug fix"

# View a pad
padz view 1
padz v 1
padz 1          # Shortcut: naked integer

# Edit a pad in $EDITOR
padz open 1
padz o 1

# Delete a pad (soft delete)
padz delete 1
padz rm 1

# Search for pad
padz search "keyword"
padz ls -s "keyword"
```

## Command Reference

### Creating pad

#### `padz create [title...] [flags]`

Aliases: `new`, `n`, `c`

Create a new pad. If no content is piped, opens `$EDITOR`.

**Flags:**
- `-g, --global`: Create in global scope
- `-t, --title`: Specify title explicitly

**Examples:**
```bash
padz create                              # Opens editor, prompts for title
padz create "Shopping list"              # Opens editor with title set
padz "Quick note"                        # Shortcut syntax
padz -t "My Title" Some initial content  # Title + initial content
echo "Content" | padz new "Title"        # Pipe content in
padz create --global "Global note"       # Create in global scope
```

**Shortcuts:**
- Just `padz "Some text"` creates a new pad
- Anything that isn't a recognized command is treated as a create

### Viewing pad

#### `padz list [flags]`

Aliases: `ls`

List all pad in current scope.

**Flags:**
- `-g, --global`: Show only global pad
- `-s, --search <term>`: Search for pad containing term
- `--deleted`: Show only soft-deleted pad
- `--include-deleted`: Include deleted pad in listing

**Output Format:**
```
p1  📌 10 minutes ago  Important note (pinned)
p2  📌 1 hour ago      Another pinned note
1   2 hours ago       Regular note
2   3 days ago        Another note
```

**Search Ranking:**
When using `-s` or `--search`, results are ranked by:
1. Exact title matches
2. Partial title matches
3. Content matches
4. Match length
5. Original creation order

**Examples:**
```bash
padz list                    # List project pad
padz ls                      # Same (alias)
padz                         # Same (default command)
padz ls -g                   # List global pad
padz ls -s "TODO"            # Search for "TODO"
padz ls --deleted            # Show only deleted pad
padz ls --include-deleted    # Show active + deleted pad
```

#### `padz view <index> [flags]`

Aliases: `v`

View the content of a pad in your terminal.

**Flags:**
- `-g, --global`: Operate on global pad
- `--pager`: Use system pager (like `less`)

**Examples:**
```bash
padz view 1      # View pad #1
padz v 1         # Same (alias)
padz 1           # Same (naked integer shortcut)
padz view p1     # View pinned pad #1
padz view d1     # View deleted pad #1 (if using --deleted flag)
```

**Note:** The "naked integer" shortcut (`padz 1`) defaults to `view`. This is configurable in the code via `config.NakedIntCommand`.

#### `padz peek <index> [flags]`

Show first and last few lines of a pad.

**Flags:**
- `-g, --global`: Operate on global pad
- `-n, --lines <int>`: Number of lines from start/end (default: 3)

**Examples:**
```bash
padz peek 1       # Show first/last 3 lines
padz peek 1 -n 5  # Show first/last 5 lines
```

### Editing pad

#### `padz open <index> [flags]`

Aliases: `o`, `e`

Open a pad in `$EDITOR`.

**Flags:**
- `-g, --global`: Operate on global pad
- `--lazy`: Launch editor and exit immediately (non-blocking)

**Examples:**
```bash
padz open 1       # Edit pad #1
padz o 1          # Same (alias)
padz e 1          # Same (alias)
padz open p1      # Edit pinned pad #1
padz open 1 --lazy  # Launch editor, don't wait
```

### Deleting pad

#### `padz delete <index> [flags]`

Aliases: `rm`, `d`, `del`

Soft-delete a pad. It can be restored later.

**Flags:**
- `-g, --global`: Operate on global pad

**Examples:**
```bash
padz delete 1     # Soft-delete pad #1
padz rm 1         # Same (alias)
padz d 1          # Same (alias)
```

**Note:** Soft-deleted pad are:
- Hidden from normal listings
- Visible with `padz ls --deleted` or `padz ls --include-deleted`
- Auto-cleaned after 7 days by default
- Can be restored with `padz restore`
- Can be permanently deleted with `padz flush`

### Managing Deleted pad

#### `padz restore [id...] [flags]`

Aliases: `undelete`, `recover`

Restore soft-deleted pad back to active state.

**Flags:**
- `-g, --global`: Restore from global scope
- `-p, --project <name>`: Restore from specific project
- `-a, --all`: Restore from all projects
- `--newer-than <duration>`: Only restore items deleted less than duration ago (e.g., `1h`, `30m`)

**Examples:**
```bash
padz restore d1                # Restore specific deleted pad
padz restore d1 d2 d3          # Restore multiple pad
padz restore --newer-than 1h   # Restore pad deleted < 1 hour ago
padz restore --all             # Restore all deleted pad everywhere
```

#### `padz flush [id...] [flags]`

Aliases: `purge`

Permanently delete soft-deleted pad from disk.

**Flags:**
- `-g, --global`: Flush from global scope
- `-p, --project <name>`: Flush from specific project
- `-a, --all`: Flush from all projects
- `--older-than <duration>`: Only flush items deleted more than duration ago (e.g., `7d`, `24h`)

**Examples:**
```bash
padz flush                      # Flush all deleted pad (current scope)
padz flush d1                   # Flush specific pad
padz flush d1 d2 d3             # Flush multiple pad
padz flush --older-than 7d      # Flush pad deleted > 7 days ago
padz flush --older-than 24h     # Flush pad deleted > 24 hours ago
padz flush --all                # Flush from all projects
```

**Warning:** Flush is permanent. Flushed pad cannot be recovered.

### Searching

#### `padz search [term...] [flags]`

Search through pad titles and content using regular expressions.

**Flags:**
- `-g, --global`: Search only global pad
- `-p, --project <name>`: Limit search to specific project

**Examples:**
```bash
padz search "my term"           # Search for "my term"
padz search my term             # Same (spaces automatically joined)
padz search "TODO|FIXME"        # Search using regex
padz search "func.*main"        # Regex pattern
padz search -g "global notes"   # Search only global pad
```

**Note:** The search command is separate from `padz ls -s`. Both perform search, but `search` is a dedicated command with additional options.

### Pinning

#### `padz pin <id> [id...] [flags]`

Aliases: `p`

Pin one or more pad to the top of the list.

**Flags:**
- `-g, --global`: Operate on global pad

**Limits:**
- Maximum 5 pad can be pinned at once
- Pinned pad appear with `p1`, `p2`, `p3`, `p4`, `p5` prefixes
- Pinned pad always appear first in listings

**Examples:**
```bash
padz pin 1         # Pin pad #1
padz pin 1 3 5     # Pin multiple pad
padz p 1           # Same (alias)
```

#### `padz unpin <id> [id...] [flags]`

Aliases: `u`

Unpin pad.

**Flags:**
- `-g, --global`: Operate on global pad

**Examples:**
```bash
padz unpin p1      # Unpin pinned pad #1
padz unpin p1 p2   # Unpin multiple pinned pad
padz u p1          # Same (alias)
```

### Bulk Operations

#### `padz cleanup [flags]`

Aliases: `clean`

Cleanup pad older than specified number of days.

**Flags:**
- `-d, --days <int>`: Delete pad older than this many days (default: 30)

**Examples:**
```bash
padz cleanup           # Delete pad older than 30 days
padz cleanup -d 90     # Delete pad older than 90 days
padz clean -d 7        # Delete pad older than 7 days
```

**Note:** This performs soft-delete. Use `flush` to permanently delete.

#### `padz nuke [flags]`

Delete all pad in the current scope.

**Examples:**
```bash
padz nuke              # Delete all pad in current project (with confirmation)
padz nuke --global     # Delete all global pad (with confirmation)
```

**Warning:** This is a destructive operation. You'll be prompted to confirm.

### Utility Commands

#### `padz copy <index> [flags]`

Aliases: `cp`

Copy pad content to system clipboard.

**Flags:**
- `-g, --global`: Operate on global pad

**Examples:**
```bash
padz copy 1        # Copy pad #1 to clipboard
padz cp 1          # Same (alias)
padz copy p1       # Copy pinned pad #1
```

#### `padz path <index> [flags]`

Get the full filesystem path to a pad file.

**Flags:**
- `-g, --global`: Operate on global pad

**Examples:**
```bash
padz path 1        # Get path to pad #1
padz path p1       # Get path to pinned pad #1

# Use in shell scripts
vim $(padz path 1)
cat $(padz path 1) | grep TODO
```

#### `padz export [id...] [flags]`

Export pad to files in a directory.

**Flags:**
- `-g, --global`: Export global pad
- `--format <string>`: Export format - `txt` or `markdown` (default: `txt`)

**Output:**
- Creates directory: `padz-export-YYYY-MM-DD-HH-mm/`
- Filename format: `<index>-<title>.<extension>`

**Examples:**
```bash
padz export                     # Export all pad as .txt
padz export --format markdown   # Export all as .md
padz export 1 2 3               # Export specific pad
padz export p1 p2               # Export pinned pad
```

## Global Flags

These flags work with all commands:

- `-f, --format <string>`: Output format - `plain`, `json`, or `term` (default: `term`)
- `--silent`: Suppress list output after commands
- `--verbose`: Show list output after commands (default)
- `-v, -vv, -vvv`: Increase verbosity for debugging

**Examples:**
```bash
padz list --format json      # Output as JSON
padz list --format plain     # Plain text output (no colors)
padz create "Note" --silent  # Don't show list after creating
padz list -vv                # Debug output
```

## How It Works

### Data Storage

The data storage is a directory. 
In it, we have each pad in a file,named pad-{UUID}.(txt | md)
and a data.json file that lists all files and their metadata (uuid, title , created, deletion pinning, etc)

this is the .padz directory. It's either in project's directory or uses the xdg data path for the global (user wide ) scope
### Auto-Cleanup

PADZ automatically cleans up soft-deleted pad:
- Runs in the background on most commands
- Permanently deletes pad that have been soft-deleted for > 7 days
- Non-blocking (won't slow down your commands)
- Configurable via `commands.CleanupOptions` in code

### Output Modes

Commands that modify pad (create, delete, pin, etc.) automatically show the updated list afterwards. You can control this:

- `--verbose` (default): Show list after command
- `--silent`: Don't show list after command

**Example:**
```bash
padz create "Note" --silent   # Create but don't list
padz delete 1 --verbose       # Delete and show updated list (default)
```

### Command Resolution

PADZ has smart command resolution for convenience:

1. **No arguments**: Runs `list`
   ```bash
   padz          # Same as: padz list
   ```

2. **Single integer**: Runs `view` (or `open`, configurable)
   ```bash
   padz 1        # Same as: padz view 1
   ```

3. **Starts with flag**: Assumed to be for `list`
   ```bash
   padz -g       # Same as: padz list -g
   padz -s "foo" # Same as: padz list -s "foo"
   ```

4. **Known command**: Runs that command
   ```bash
   padz create
   padz search
   ```

5. **Unknown text**: Assumed to be `create` with title
   ```bash
   padz "My note"    # Same as: padz create "My note"
   ```

This means you rarely need to type `create` or `list` explicitly.

The key thing about this, called naked mode (no sub command specified) is how to integrate with the cli library.
we want to avoid haveing to parse as much as possible.

so the right way to do is; 
execute the command as it. if it has no subcommand , the library will generate an error. 
intercep the error. if it's a no command given error, now you will figure out what needs to be done.
use the parsed data from the cli to figure out if you have no args (list), an int(view) and so on.

one you do, inject the right string into the user generated input, then re ran the injected command through the cli library.




## Output Format

### Terminal Format (default)

Colorful, human-readable output with:
- Relative timestamps ("10 minutes ago", "3 days ago")
- Pin indicators (📌 for pinned items)
- Syntax highlighting for content
- Visual separators

### Plain Format

Plain text without colors or special formatting. Useful for:
- Piping to other commands
- Logging
- Scripts
(detected automatically)
```bash
padz list --format plain
```

### JSON Format

Machine-readable JSON output. Each pad is a JSON object with fields:
- `id`: UUID
- `project`: Project name
- `title`: pad title
- `content`: Full content
- `created_at`: ISO 8601 timestamp
- `updated_at`: ISO 8601 timestamp
- `size`: Content size in bytes
- `checksum`: Content checksum
- `is_pinned`: Boolean
- `pinned_at`: ISO 8601 timestamp (if pinned)
- `is_deleted`: Boolean
- `deleted_at`: ISO 8601 timestamp (if deleted)

```bash
padz list --format json | jq '.[0].title'
```

## Logging

PADZ maintains detailed logs for debugging:

**Console Logging:**
- Controlled by `-v` flags
- `-v`: Info level
- `-vv`: Debug level
- `-vvv`: Trace level

**File Logging:**
- Always enabled, logs everything
- JSON format for structured parsing
- Location:
  - **macOS**: `~/Library/Application Support/padz/padz.log`
  - **Linux**: `~/.local/state/padz/padz.log`
  - **Windows**: `%LOCALAPPDATA%\padz\padz.log`

**Finding the log file:**
```bash
padz ls -vv
# Look for: "Logger initialized with dual output log_file=..."
```

## Tips & Tricks

### Quick Capture

Create a pad with content in one line:
```bash
echo "TODO: Fix the auth bug" | padz "Auth Bug"
```
### Testing Environment

PADZ includes a testing environment for safe experimentation:

```bash
# Start isolated environment
./live-tests/run

# Inside the test environment:
padz create "Test note"
padz list
# ... test freely ...
exit

# All data is automatically cleaned up
```

The test environment provides:
- Isolated HOME directory
- Separate XDG directories
- Fresh padz data store
- No impact on your real pad
``

### Index confusion?

Remember that indexes are display-only and change:
- Pinning changes indexes
- Deleting changes indexes
- Creating new pad changes indexes

If you need stable references, use:
```bash
padz list --format json | jq -r '.[].id'   # Get UUIDs
```

## Limitations

- **Capacity**: Optimized for ~1,000 pad
- **Single User**: No multi-user or concurrent access
- **No Sync**: No built-in cloud sync (data is local)
- **No Encryption**: pad are stored as plain text
- **Pin Limit**: Maximum 5 pinned pad


## Understanding Indexes

PADZ uses **Display Indexes** (e.g., `1`, `p1`, `d1`) to make commands easy to type. It's much faster to type `padz copy 3` than `padz copy 550e8400-e29b-41d4-a716-446655440000`. However, it is crucial to understand how these indexes are calculated to avoid mistakes.

### The Consistency Challenge

A naive approach would be to number items based on the *current view*. If you search for "meeting" and get 3 results, they might be numbered 1, 2, and 3.

**The Danger:**
Imagine you have these notes in your main list:
1. `Buy Milk`
2. `Fix critical bug`
3. `Call Mom`

If you search for "critical", and the result is displayed as `#1: Fix critical bug`, you might instinctively type `padz delete 1`.
But if `padz` is stateless (which it is), `padz delete 1` would delete the *actual* #1 note ("Buy Milk"), not the one you saw in the search results.

### The PADZ Solution: Canonical Indexing

To solve this, PADZ indexes are **stable across views**. The index you see in a search result is the same index the item holds in the full, unfiltered list.

**How it works:**
1. PADZ loads **all** active notes for the current scope (the "Default View").
2. It assigns indexes (`1`, `2`, `3`...) based on this full list.
3. When you run a search, it filters the list but **preserves the original indexes**.

**Example:**
*   **Full List:**
    *   `1`: Buy Milk
    *   `2`: Fix critical bug
    *   `3`: Call Mom
*   **Search "critical":**
    *   Result: `2`: Fix critical bug (It retains index #2)

This ensures that `padz delete 2` always acts on "Fix critical bug", regardless of whether you are looking at the full list or a search result.

### Index Buckets

Indexes are divided into three stable "buckets":

1.  **Pinned (`p1`, `p2`...)**:
    *   Pinned items are always processed first.
    *   They have their own independent numbering sequence.
2.  **Regular (`1`, `2`...)**:
    *   The standard list of active notes.
    *   Numbered sequentially based on creation time (or configured sort order).
3.  **Deleted (`d1`, `d2`...)**:
    *   Soft-deleted items, only visible when using flags like `--deleted`.
    *   They form a separate list with its own sequence, ensuring they don't shift the indexes of active notes.
