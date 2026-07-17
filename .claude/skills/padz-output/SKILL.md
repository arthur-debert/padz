---
name: padz-output
description: |
  Guide for customizing padz CLI output: templates, styles, and rendering.
  Use when working on: (1) Modifying templates in crates/padz/src/cli/templates/,
  (2) Adding or changing styles in styles/default.yaml, (3) Debugging output with --output flag,
  (4) Adding new commands that need rendered output, (5) Understanding the outstanding crate integration.
---

# Padz Output Customization

## Architecture Overview

Padz uses a three-layer system for CLI output, much like web appl, wher the api returns a data object, and
the rendering system (using outstanding create) uses file based templates (minijinja) + styles to render:

1. **Templates** (Minijinja `.jinja` files) - Define structure and layout
2. **Styles** (YAML stylesheet) - Define colors and formatting
3. **Renderer** (outstanding crate) - Combines templates + styles with output mode

## Quick Reference

### Output Modes (`--output` flag)

```bash
# Human modes — templates + styles, terminal-dependent
padz list --output=auto       # Default: colors for TTY, plain for pipes
padz list --output=term       # Force ANSI colors
padz list --output=text       # Plain text, no colors
padz list --output=term-debug # Debug: shows [style-name]text[/style-name]

# Structured modes — the handler result serialized directly, templates bypassed
padz list --output=json       # Machine-readable JSON
padz list --output=yaml       # Same data as JSON
padz list --output=xml        # Same data, element-per-field
padz list --output=csv        # Flattened to one dotted-path row (lossy; see below)
```

The mode set is **standout's**, not padz's: standout defines the `--output` values and
`parse_cli` (`crates/padz/src/cli/setup.rs`) delegates to `App::extract_output_mode` to
read them back. Do not hand-write a `match` over mode strings — padz used to, the copy
only knew `json`, and `yaml`/`xml`/`csv` silently fell through to `Auto` and rendered
the human template to callers asking for data.

CSV is standout's generic flattening of the one result value: a single header row and a
single data row of dotted paths (`pads.0.pad.metadata.title`), where nesting deepens the
column name rather than adding rows, and nulls/empty arrays contribute no column. It is
lossy by design; JSON/YAML are the shapes agents should read. `crates/padz/tests/structured_output_e2e.rs`
pins all of this.

### Key Files

| Purpose | Location |
| --------- | ---------- |
| Templates | `crates/padz/src/cli/templates/*.jinja` |
| Template registry | `crates/padz/src/cli/templates.rs` |
| Stylesheet | `crates/padz/src/styles/default.yaml` |
| Theme loading | `crates/padz/src/cli/styles.rs` |
| Rendering logic | `crates/padz/src/cli/render.rs` |

## Templates

Templates use Minijinja syntax. Located in `crates/padz/src/cli/templates/`.

### Where template data comes from

Handlers return one typed, mode-independent result (`crates/padz/src/cli/result.rs`).
Standout serializes it once, then either emits it directly (any structured mode: json,
yaml, xml, csv) or renders a template with it. Templates therefore see the handler's
result at the top level.

Terminal-only fields — column widths, status glyphs, `time_ago`, the pin marker — are
**not** in the handler result. They are derived at render time by the view builders in
`crates/padz/src/cli/render.rs`, registered as standout context providers
(`context_fn`) in `crates/padz/src/cli/commands.rs`. Standout resolves providers only
on the template path, which is what keeps structured output free of them.

Two templates read a provider rather than the raw result:

| Template | Reads | Provider |
| -------- | ----- | -------- |
| `list.jinja` | `list_view` | `render::list_view_provider` |
| `modification_result.jinja` | `modification_view` | `render::modification_view_provider` |

Every other template reads the handler result directly.

### Main Templates

- `list.jinja` - Pad list with columns, indentation, status icons (via `list_view`)
- `modification_result.jinja` - Pads changed by a command (via `modification_view`)
- `view.jinja` - `padz view`: title + body per pad
- `full_pad.jinja` - Complete pad view (title + content)
- `text_list.jinja` - Simple line-by-line output
- `messages.jinja` - Command feedback (success/error/info)
- `path.jinja` / `uuid.jinja` - One pad path / uuid per line

### Partial Templates (reusable)

- `_pad_line.jinja` - Single row in list (included by list.jinja)
- `_match_lines.jinja` - Search result highlighting
- `_peek_content.jinja` - Content preview for `--peek`
- `_deleted_help.jinja` - Help text for deleted section

### Template Syntax

```jinja
{#- Whitespace-trimming comment -#}
{%- set view = list_view -%}
{% for pad in view.pads %}
  {{- pad.title | col(pad.title_width) | style("list-title") | nl -}}
{% endfor %}
```

Key filters from outstanding:

- `col(width, align='left'|'right')` - Column layout
- `style("name")` - Apply semantic style
- `nl` - Explicit newline
- `truncate_to_width()` - Truncate with ellipsis
- `indent(n)` - Add indentation

### Adding a New Template

1. Create `.jinja` file in `templates/` directory
2. Register in `templates.rs`:

   ```rust
   pub const MY_TEMPLATE: &str = include_str!("templates/my_template.jinja");
   ```

3. Add to renderer in `create_renderer()`:

   ```rust
   renderer.add_template("my_template", MY_TEMPLATE)?;
   ```

4. Call from render function:

   ```rust
   renderer.render("my_template", &data)?
   ```

## Styles (YAML Stylesheet)

Styles are defined in `crates/padz/src/styles/default.yaml` and embedded at compile time.

### Three-Layer Architecture

```yaml
# Layer 1: Visual (internal, prefixed with _)
_gold:
  light:
    fg: [196, 140, 0]
  dark:
    fg: [255, 214, 10]

# Layer 2: Presentation (aliases)
_accent: _gold

# Layer 3: Semantic (use in templates)
list-index: _accent
pinned:
  bold: true
  light:
    fg: [196, 140, 0]
  dark:
    fg: [255, 214, 10]
```

### Semantic Styles (use in templates)

**List:** `list-index`, `list-title`, `pinned`, `deleted-index`, `deleted-title`, `status-icon`

**Content:** `title`, `time`, `hint`

**Messages:** `error`, `warning`, `success`, `info`

**Search:** `highlight`, `match`

**Help:** `help-header`, `help-section`, `help-command`, `help-desc`, `help-usage`

**Misc:** `help-text`, `section-header`, `empty-message`, `preview`, `truncation`, `line-number`, `separator`

### Adding a New Style

Add to `default.yaml`:

```yaml
# Simple alias
my-style: _accent

# With modifiers
my-style:
  bold: true
  italic: true
  light:
    fg: [196, 140, 0]
  dark:
    fg: [255, 214, 10]
```

Use in templates: `{{ value | style("my-style") }}`

### Style Embedding

Styles are embedded at compile time via `embed_styles!` macro:

```rust
pub static DEFAULT_THEME: Lazy<Theme> = Lazy::new(|| {
    let mut registry = embed_styles!("src/styles");
    registry.get("default").expect("Failed to load default theme")
});
```

## Column Layout

Constants in `render.rs`:

```rust
pub const LINE_WIDTH: usize = 100;
pub const COL_LEFT_PIN: usize = 2;
pub const COL_STATUS: usize = 2;
pub const COL_INDEX: usize = 4;
pub const COL_RIGHT_PIN: usize = 2;
pub const COL_TIME: usize = 14;
```

Title width: `LINE_WIDTH - fixed_columns - indent_width`

## Structured Output

In a structured mode (json, yaml, xml, csv), the handler's result is serialized
directly and templates — plus the `context_fn` view builders — are bypassed entirely.

The serialized shapes are the result types in `crates/padz/src/cli/result.rs`
(`PadListResult`, `ModificationResult`, `PadContentResult`, `PathResult`, `UuidResult`,
`MessagesResult`). There are no separate JSON-only types: one value serves every mode,
which is what keeps the formats in agreement.

## Debugging Output

Use `--output=term-debug` to see style names:

```text
[pinned]⚲[/pinned] [list-index]p1.[/list-index] [list-title]My Pad[/list-title]
```

## References

- [TEMPLATE_VARIABLES.md](references/TEMPLATE_VARIABLES.md) - Variables available in each template
- [STYLE_REFERENCE.md](references/STYLE_REFERENCE.md) - Complete style reference
