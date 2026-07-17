# Style Reference

All styles are defined in `crates/padz/src/styles/default.css`.

Every name below is a **semantic class**: it says what a thing is, never what it
looks like. A template emits `[name]text[/name]` (or `| style_as("name")`) and the
tag name *is* the CSS class name.

## Semantic styles (use these in templates)

| Style Name           | Light            | Dark             | Modifiers |
| -------------------- | ---------------- | ---------------- | --------- |
| **Core**             |                  |                  |           |
| `title`              | Black            | White            | bold      |
| `time`               | #737373          | #B4B4B4          | italic    |
| `hint`               | #ADADAD          | #6E6E6E          | -         |
| **List**             |                  |                  |           |
| `list-index`         | #C48C00          | #FFD60A          | -         |
| `list-title`         | Black            | White            | -         |
| `pinned`             | #C48C00          | #FFD60A          | bold      |
| `deleted`            | #BA212D          | #FF8A80          | -         |
| `deleted-index`      | #BA212D          | #FF8A80          | -         |
| `deleted-title`      | #737373          | #B4B4B4          | -         |
| `status-icon`        | #737373          | #B4B4B4          | -         |
| **Search**           |                  |                  |           |
| `highlight`          | Black on #FFEB3B | Black on #E5B900 | -         |
| `match`              | Black on #FFEB3B | Black on #E5B900 | -         |
| `line-number`        | #737373          | #8A8A8A          | italic    |
| **Tags**             |                  |                  |           |
| `tag`                | Black on #FFEB3B | Black on #E5B900 | bold      |
| **Messages**         |                  |                  |           |
| `error`              | #BA212D          | #FF8A80          | bold      |
| `warning`            | #C48C00          | #FFD60A          | bold      |
| `success`            | #008000          | #90EE90          | -         |
| `info`               | #737373          | #B4B4B4          | -         |
| **Help**             |                  |                  |           |
| `help-header`        | Black            | White            | bold      |
| `help-section`       | #C48C00          | #FFD60A          | bold      |
| `help-command`       | #008000          | #90EE90          | -         |
| `help-desc`          | #737373          | #B4B4B4          | -         |
| `help-usage`         | Cyan             | Cyan             | -         |
| **Template content** |                  |                  |           |
| `help-text`          | #ADADAD          | #6E6E6E          | -         |
| `section-header`     | #737373          | #B4B4B4          | -         |
| `empty-message`      | #737373          | #B4B4B4          | -         |
| `preview`            | #ADADAD          | #6E6E6E          | -         |
| `truncation`         | #737373          | #B4B4B4          | -         |
| `separator`          | #ADADAD          | #6E6E6E          | -         |

The message names (`error`/`warning`/`success`/`info`) are exactly `CmdMessage`'s
lowercase levels, which is what lets a template write
`{{ msg.content | style_as(msg.level) }}` instead of mapping level→style.

`help-usage` is the only scheme-independent style: cyan reads on both.

## The palette

CSS has no aliases, so the shared colours are expressed as **grouped selectors**
rather than an indirection layer. There is no `_primary`/`_secondary`/`_accent`
class any more — those were YAML-only rungs no template could emit.

| Concept | Light | Dark | Classes |
| ------- | ----- | ---- | ------- |
| primary | Black | White | `title`, `help-header`, `list-title` |
| secondary | #737373 | #B4B4B4 | `time`, `info`, `help-desc`, `section-header`, `empty-message`, `truncation`, `deleted-title`, `status-icon` |
| tertiary | #ADADAD | #6E6E6E | `hint`, `help-text`, `preview`, `separator` |
| accent | #C48C00 | #FFD60A | `list-index`, `pinned`, `help-section`, `warning` |
| danger | #BA212D | #FF8A80 | `deleted`, `deleted-index`, `error` |
| success | #008000 | #90EE90 | `success`, `help-command` |
| highlight | Black on #FFEB3B | Black on #E5B900 | `highlight`, `match`, `tag` |

`line-number` is secondary in light but a touch dimmer in dark (#8A8A8A), so it
carries its own rule.

## CSS syntax

Modifiers go in the base rule, colours in the `@media` rules — a media rule
**merges onto** the base rather than replacing it:

```css
.my-style {
    font-weight: bold;
}

@media (prefers-color-scheme: light) { .my-style { color: #c48c00; } }
@media (prefers-color-scheme: dark)  { .my-style { color: #ffd60a; } }
```

Base deliberately carries no colour: it is the fallback when the terminal's scheme
is unknown, and inheriting the light palette there would be wrong half the time.

### What standout's CSS subset supports

Verified against standout 7.6.4:

**Selectors:** class selectors only (`.name`), and comma-separated lists. An element
selector (`body { }`) is a **hard error**. No pseudo-classes, IDs, combinators,
`:root`, or CSS variables. No cascade or specificity — same-name rules merge
attribute-wise, last wins per property.

**Properties:**

| CSS | Value |
| --- | ----- |
| `color`, `fg` | a colour |
| `background-color`, `background`, `bg` | a colour |
| `font-weight` | `bold` only |
| `font-style` | `italic` only |
| `text-decoration` | `underline` or `line-through` |
| `visibility` | `hidden` only |
| `bold` `dim` `italic` `underline` `blink` `reverse` `hidden` `strikethrough` | `true` |

**Colours:** named (`black`, `white`, `red`, `green`, `blue`, `cyan`, `magenta`,
`yellow`, `gray`/`grey`, `bright_*`), hex (`#rgb`, `#rrggbb`), or `cube(r%, g%, b%)`.

### Traps

- **`opacity` is silently ignored** — an invalid declaration is skipped, not
  reported. Use `dim: true`. (standout's own docs use `opacity: 0.5`; it is a no-op.)
- **`rgb()` and integer colour indices are not accepted in CSS** — hex or `cube()`.
  The old YAML's `fg: [196, 140, 0]` arrays became `#c48c00` in the migration.
- **Aliases and icons are YAML-only.** Neither can be expressed in CSS.
- **Format is detected by content, not extension.** A stylesheet must start with
  `.`, `/*`, or `@media`, or it is parsed as YAML and fails.
- **A user theme fully replaces `Theme::default()`.** Nothing is merged in, so any
  framework style name padz still emits must be defined here.

## Adding a style

1. Add the class to `default.css`.
2. Add its name to `REQUIRED` in `crates/padz/tests/theme.rs`.
3. Emit it from a template.

Step 2 matters: a class a template emits but the theme lacks does **not** fail
loudly. It renders as `[my-style?]text[/my-style?]` in the user's terminal. The
test is what turns that into a build failure.

## Light/dark detection

Auto-detected at runtime by standout; the matching variant is resolved per render.
`tests/theme.rs` asserts that light and dark actually differ — a class that
resolved identically in both would mean its `@media` rules are dead.
