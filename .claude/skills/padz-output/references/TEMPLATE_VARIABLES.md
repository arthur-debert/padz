# Template Variables Reference

What each template can read. Shapes are the Rust types' **serialized** form —
that is what a template actually sees.

Source of truth: `crates/padz/src/cli/result.rs` (handler results) and
`crates/padz/src/cli/render.rs` (render-time view data).

## Every template

`terminal` — from `render::terminal_provider`:

```jinja
{{ terminal.width }}   {# effective layout width; always pass to tabular(width=) #}
```

`terminal.width` resolves `$COLUMNS` → real terminal → 80, clamps to ≥30, then
subtracts 1 to pay back `⏲` (which `unicode-width` measures as 1 column but
terminals draw as 2).

## `list.jinja` — `list_view`

From `render::list_view_provider` (a `render::ListView`):

```text
list_view.rows          [PadRow]   flattened pad tree, in display order
list_view.filtered      bool       narrowed and matched nothing (vs. store empty)
list_view.sections      bool       label lifecycle blocks (--all)
list_view.show_status   bool       draw the status column
list_view.peek          bool       --peek was asked for
list_view.deleted_help  bool       append the restore/purge help block
list_view.help_text     string     grouped command help; only for an empty store
list_view.messages      [CmdMessage]
```

## `modification_result.jinja` — `modification_view`

From `render::modification_view_provider` (a `render::ModificationView`):

```text
modification_view.action       string   past-tense verb only ("Created", "Pinned")
modification_view.rows         [PadRow] affected pads, always flat (depth 0)
modification_view.show_status  bool
modification_view.messages     [CmdMessage]
```

The template builds the sentence, including the plural:

```jinja
[info]{{ view.action }} {{ count }} {{ "pad" if count == 1 else "pads" }}...[/info]
```

## `PadRow`

The unit both `_pad_line.jinja` and `_match_lines.jinja` render. Every field is
data — no widths, no glyphs, no style names, no rendered sentences.

```text
index        {type, value}   "Pinned"|"Regular"|"Archived"|"Deleted" + number
depth        int             0 for a root; the template decides what a level costs
section      string          the ROOT's bucket — see below
title        string          raw title
short_uuid   string?         present only under --uuid (absent otherwise)
tags         [string]
status       string          "Planned"|"InProgress"|"Done"
pinned       bool            the pad is pinned (true in BOTH blocks)
time         {value, unit}   e.g. {value: 3, unit: "m"} — never a sentence
matches      [SearchMatch]   search hits; line 0 (the title match) is excluded
peek         PeekResult?     present only under --peek AND when there is a body
```

### `section` vs `index.type` — read this before touching section logic

They are different questions, and confusing them is a real bug:

- `index.type` is **this row's own** display identifier.
- `section` is the bucket **this row's root** sits in.

`index_pads` gives a pinned root's children `Regular` indexes. A template that
drove section breaks off `index.type` would split the pinned block open at its
first child. Because `section` is constant across a root's whole subtree, a break
is one comparison at any depth:

```jinja
{%- set changed = loop.previtem is not defined or loop.previtem.section != pad.section -%}
```

### `SearchMatch`

```text
line_number  int             1-based; line 0 is filtered out by render.rs
segments     [{type, text}]  type is "Plain" or "Match"
```

`_match_lines.jinja` maps `Plain`→`info` and `Match`→`match`, and truncates the
run itself (styling each segment *after* cutting it, because `truncate_at` returns
plain text and would otherwise drop the tags).

### `PeekResult`

```text
opening_lines    string
truncated_count  int?
closing_lines    string?
```

## `view.jinja` / `full_pad.jinja` — `PadContentResult`

Read directly from the handler result:

```text
pads[].title    string   carries its tree indent in --indented mode
pads[].content  string   body (content minus the title line)
pads[].depth    int
pads[].uuid     string?  present only under --uuid
```

## `messages.jinja` — `MessagesResult`

```text
messages[].content  string
messages[].level    string   "info"|"success"|"warning"|"error"
```

`level` serializes lowercase **to match the theme's class names**, so it can be
used as a style directly: `{{ msg.content | style_as(msg.level) }}`.

## `path.jinja` / `uuid.jinja`

```text
paths  [string]
uuids  [string]
```

## `text_list.jinja`

```text
lines          [string]
empty_message  string
```

## Structured output

In json/yaml/xml/csv the handler result is serialized directly — templates and the
`context_fn` providers are **bypassed entirely**. Nothing on this page except the
handler results themselves (`result.rs`) appears in structured output; `PadRow`,
`TimeAgo` and `terminal.width` are render-only by construction.

`tests/harness.rs::structured_output_excludes_template_only_view_fields` pins that.
