# Template Variables Reference

## List Template (`list.tmp`)

Receives `ListData` struct:

```rust
struct ListData {
    pads: Vec<PadLineData>,      // Array of pad rows
    empty: bool,                  // True if no pads
    pin_marker: String,           // "⚲" character
    help_text: String,            // Help for empty list
    deleted_help: bool,           // Show deleted section help
    peek: bool,                   // Show content previews

    // Column widths (pass to col() filter)
    col_left_pin: usize,          // 2
    col_status: usize,            // 2
    col_index: usize,             // 4
    col_right_pin: usize,         // 2
    col_time: usize,              // 14
}
```

### PadLineData (per row)

```rust
struct PadLineData {
    indent: String,               // Depth-based indentation
    left_pin: String,             // Pin marker or empty
    status_icon: String,          // Todo status (⚪︎, ☉, ⚫︎)
    index: String,                // Display index (p1., 1., d1.)
    title: String,                // Pad title (raw)
    title_width: usize,           // Calculated width for title
    right_pin: String,            // Pin marker for pinned in regular section
    time_ago: String,             // Formatted timestamp

    // Semantic flags
    is_pinned_section: bool,      // In pinned section
    is_deleted: bool,             // Deleted pad
    is_separator: bool,           // Section separator row

    // Optional content
    matches: Vec<MatchLineData>,  // Search results
    peek: Option<PeekResult>,     // Content preview
}
```

### MatchLineData (search results)

```rust
struct MatchLineData {
    line_num: usize,
    prefix: String,               // Text before match
    matched: String,              // Matched text (for highlighting)
    suffix: String,               // Text after match
}
```

### PeekResult (content preview)

```rust
struct PeekResult {
    lines: Vec<String>,           // Preview lines
    truncated: bool,              // More content exists
}
```

## Full Pad Template (`full_pad.tmp`)

Receives `FullPadData` struct:

```rust
struct FullPadData {
    pads: Vec<FullPadEntry>,
}

struct FullPadEntry {
    index: String,                // Display index
    title: String,                // Pad title
    content: String,              // Full content
    is_pinned: bool,
    is_deleted: bool,
}
```

## Messages Template (`messages.tmp`)

Receives `MessagesData` struct:

```rust
struct MessagesData {
    messages: Vec<MessageData>,
}

struct MessageData {
    content: String,              // Message text
    style: String,                // Style name: "success", "error", "warning", "info"
}
```

## Text List Template (`text_list.tmp`)

Receives `TextListData` struct:

```rust
struct TextListData {
    lines: Vec<String>,           // Simple string lines
}
```

## JSON Output Types

When `--output=json`, these are serialized directly:

```rust
struct JsonPadList {
    pads: Vec<JsonPad>,
}

struct JsonPad {
    id: String,                   // UUID
    index: String,                // Display index
    title: String,
    content: String,
    created_at: String,           // ISO 8601
    updated_at: String,           // ISO 8601
    is_pinned: bool,
    is_deleted: bool,
    parent_id: Option<String>,
    todo_status: String,          // "planned", "in_progress", "done"
}

struct JsonMessages {
    messages: Vec<JsonMessage>,
}

struct JsonMessage {
    level: String,                // "success", "error", "warning", "info"
    content: String,
}
```
