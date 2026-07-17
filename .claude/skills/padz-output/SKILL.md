---
name: padz-output
description: |
  Guide for customizing padz CLI output: templates, styles, and rendering.
  Use when working on: (1) Modifying templates in crates/padz/src/cli/templates/,
  (2) Adding or changing styles in styles/default.css, (3) Debugging output with --output flag,
  (4) Adding new commands that need rendered output, (5) Understanding the standout crate integration.
---

# Padz Output

## The rule this skill exists to state

**Presentation policy lives in templates and CSS, not in Rust.** Wording,
pluralization, glyphs, section labels, index formatting, indentation, column
widths and style choice are all decided in `crates/padz/src/cli/templates/` and
`crates/padz/src/styles/default.css`. Rust derives *data* and stops.

If you are about to write a `format!` that a user will read, or a `match` that
picks a colour or a glyph, you are in the wrong file.

## Architecture

Padz renders with **standout** (not "outstanding" — that name is from a previous
era and appears nowhere in the codebase).

```text
handler -> Output::Render(typed result) -> standout serializes it once
                                             |-- structured mode: emitted as-is
                                             `-- human mode: template + context providers
```

A handler returns one typed, mode-independent result (`cli/result.rs`) regardless
of `--output`. Standout serializes it once, then either emits it directly
(json/yaml/xml/csv) or renders a MiniJinja template with it. **Structured output
never touches a template or the CSS**, which is what keeps terminal artifacts out
of it.

### Key files

| Purpose | Location |
| ------- | -------- |
| Templates | `crates/padz/src/cli/templates/*.jinja` |
| Layout + glyph constants | `crates/padz/src/cli/templates/_layout.jinja` |
| Stylesheet (CSS) | `crates/padz/src/styles/default.css` |
| View data derivation | `crates/padz/src/cli/render.rs` |
| App wiring (templates, styles, providers) | `crates/padz/src/cli/commands.rs` |
| Typed handler results | `crates/padz/src/cli/result.rs` |

There is no `templates.rs`, no `styles.rs`, and no `create_renderer()`. Templates
and styles are embedded by `embed_templates!("src/cli/templates")` and
`embed_styles!("src/styles")` in `commands.rs`, and the theme is selected with
`.default_theme("default")` (the file stem of `default.css`).

## Output modes (`--output`)

```bash
padz list --output=auto       # Default: colors for TTY, plain for pipes
padz list --output=term       # Force the terminal path
padz list --output=text       # Plain text, no escapes
padz list --output=term-debug # Shows [style-name]text[/style-name]
padz list --output=json       # Machine-readable
padz list --output=yaml       # Same data as JSON
padz list --output=xml        # Same data, element-per-field
padz list --output=csv        # Flattened dotted-path row (lossy; see below)
```

The mode set is **standout's**, not padz's: `parse_cli` (`cli/setup.rs`) delegates
to `App::extract_output_mode`. Do not hand-write a `match` over mode strings — padz
used to, the copy only knew `json`, and `yaml`/`xml`/`csv` silently fell through to
`Auto` and rendered human text to callers asking for data.

CSV is standout's generic flattening of the one result value: a single header row
and a single data row of dotted paths (`pads.0.pad.metadata.title`), where nesting
deepens the column name rather than adding rows. It is lossy by design; JSON/YAML
are the shapes agents should read. `tests/structured_output_e2e.rs` pins all of it.

## Templates

### Where template data comes from

Templates see the handler's serialized result at the top level. Two templates also
read a **context provider** — a render-time-only derivation registered with
`context_fn` in `commands.rs`, which standout resolves *only* on the template path:

| Template | Reads | Provider |
| -------- | ----- | -------- |
| `list.jinja` | `list_view` | `render::list_view_provider` |
| `modification_result.jinja` | `modification_view` | `render::modification_view_provider` |
| *(all templates)* | `terminal.width` | `render::terminal_provider` |

Every other template reads the handler result directly.

See [TEMPLATE_VARIABLES.md](references/TEMPLATE_VARIABLES.md) for the exact shapes.

### The templates

- `list.jinja` — listing; owns section breaks, separators, empty states
- `modification_result.jinja` — pads changed by a command; owns the action sentence
- `_pad_line.jinja` — one pad row; owns columns, glyphs, index format, style choice
- `_match_lines.jinja` — search hit lines; owns their truncation
- `_peek_content.jinja` — `--peek` preview
- `_deleted_help.jinja` — the restore/purge help block
- `_layout.jinja` — **imported, not included**: column widths, glyphs, section labels
- `view.jinja`, `full_pad.jinja`, `text_list.jinja`, `messages.jinja`,
  `path.jinja`, `uuid.jinja` — read their result directly

`_layout.jinja` is the one place a width or a glyph is named:

```jinja
{%- import "_layout.jinja" as L -%}
{{ L.COLS.index }}   {# 4 #}
{{ L.STATUS_GLYPH[pad.status] }}
{{ L.SECTION_TITLE["Deleted"] }}
```

### Filters standout provides

- `col(width, align=, truncate=)` — column layout; `width` may be `"fill"`
- `pad_left(n)` / `pad_right(n)` / `pad_center(n)` — pad to a width
- `truncate_at(width, position=, ellipsis=)` — truncate; **returns plain text**
- `display_width` — Unicode-aware width
- `style_as(name)` — emit `[name]value[/name]`
- `nl` — explicit newline
- `tabular(columns, separator=, width=)` — declarative columns; `.row([...])`

Gotchas that have already bitten this codebase:

- **The `style()` filter was removed in standout 1.0** and now hard-errors. Use
  BBCode tags: `[name]text[/name]`, or the `style_as` filter.
- `tabular(...).row_from(...)` is **not** callable from a template. Use `.row([...])`.
- `tabular(width=)` defaults to a hardcoded 80. Always pass `width=terminal.width`.
- A column spec's key is `key`, not `name`.
- `col`/`truncate_at` **drop style tags when they truncate**. Style *around* the
  filter, never inside the value.
- MiniJinja has no pluralization. Use an inline conditional:
  `{{ "pad" if count == 1 else "pads" }}`.

## Styles (CSS)

`crates/padz/src/styles/default.css`. Every selector is a **semantic class**: it
names what a thing is, never what it looks like. A template emits `[name]…[/name]`
and the tag name *is* the class name.

```css
.list-index { font-weight: bold; }

@media (prefers-color-scheme: light) { .list-index { color: #c48c00; } }
@media (prefers-color-scheme: dark)  { .list-index { color: #ffd60a; } }
```

A rule inside `@media` **merges onto** the base rule rather than replacing it — so
shared modifiers (bold/italic) are stated once in the base, and only colours are
restated per scheme. Base deliberately carries no colour: it is the fallback when
the scheme is unknown, and inheriting the light palette there would be wrong half
the time.

Because CSS has no aliases, the palette is expressed with grouped selectors
(`.info, .section-header, .empty-message { color: #737373; }`). Naming the concept
in a comment beats inventing `_secondary` classes no template may emit.

### Constraints of standout's CSS subset

Learned the hard way; all verified against standout 7.6.4:

- **Class selectors only.** `body { }` is a hard error. No pseudo-classes, IDs,
  combinators, or `:root`.
- **No aliases and no icons** in CSS (both are YAML-only features).
- **No `rgb()`, no integer colour indices, no CSS variables.** Use `#rrggbb`,
  named colours, or `cube(r%, g%, b%)`.
- **`opacity` is silently ignored.** Use `dim: true`.
- Supported: `color`, `background-color`, `font-weight: bold`, `font-style: italic`,
  `text-decoration: underline|line-through`, and the flag forms `bold|dim|italic|
  underline|blink|reverse|hidden|strikethrough: true`.
- Format is detected by **content, not extension**: a stylesheet must start with
  `.`, `/*`, or `@media` or it is parsed as YAML and fails.
- A user theme **fully replaces** `Theme::default()`; nothing is merged in.

See [STYLE_REFERENCE.md](references/STYLE_REFERENCE.md) for every class and its
light/dark values.

### Adding a style

1. Add the class to `default.css` (base rule for modifiers, `@media` for colours).
2. Add its name to `REQUIRED` in `crates/padz/tests/theme.rs`.
3. Emit it from a template as `[my-style]text[/my-style]` or `| style_as("my-style")`.

A class a template emits but the theme lacks does **not** fail loudly — it renders
as `[my-style?]text[/my-style?]` in the user's terminal. Step 2 is what turns that
into a test failure.

## Testing output

- **Style placement** → `--output=term-debug` and assert the *tag name*. Asserting
  ANSI pins the palette and breaks on every retune.
- **Template policy** (wording, glyphs, labels, index formats) → `tests/harness.rs`.
  It drives the real app in-process, which is the only place that policy exists as
  behaviour. `render.rs`'s unit tests cannot reach it.
- **View derivation** (flatten/depth/section, `TimeAgo`, peek) → unit tests in
  `render.rs`.
- **Theme integrity** → `tests/theme.rs`.
- **Structured contract** → `tests/structured_output_e2e.rs`; parse with a real
  parser, never `contains`.

```text
$ padz list --output=term-debug
[pinned]⚲[/pinned] [list-index]p1.[/list-index] [list-title]My Pad[/list-title] [time] 3m ⏲[/time]
```

## References

- [TEMPLATE_VARIABLES.md](references/TEMPLATE_VARIABLES.md) — view data shapes
- [STYLE_REFERENCE.md](references/STYLE_REFERENCE.md) — every semantic class
