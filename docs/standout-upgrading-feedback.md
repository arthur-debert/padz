# Standout Upgrade Feedback

Feedback from migrating padz from `outstanding` (v0.14) to `standout` (v1.0 @ b1644ed).

## What Went Well

1. **The guides are excellent** - `intro-to-standout.md` and `intro-to-tabular.md` are well-written and comprehensive for new users.

2. **BBCode tag syntax is intuitive** - Once understood, `[style]content[/style]` is cleaner than `{{ x | style("name") }}`.

3. **The `style_as` filter** for dynamic styles works well.

4. **StylesheetRegistry API** - Loading themes from embedded styles via `embed_styles!` → `StylesheetRegistry` → `.get("default")` worked smoothly.

## Pain Points & Documentation Gaps

### 1. No Migration Guide from `outstanding`

**Problem:** There's no documentation explaining what changed from `outstanding` to `standout`. I had to discover changes through trial and error.

**Suggestion:** Add a `migrating-from-outstanding.md` guide covering:

- Package rename: `outstanding` → `standout`, `outstanding-clap` → `standout` with `clap` feature
- Import path changes
- API renames (`OutstandingApp` → `App`, `RenderSetup` → `App::builder()`)
- Template syntax changes (`style()` filter → BBCode tags)

### 2. `App.render()` vs `Renderer.render()` - Critical Difference Undocumented

**Problem:** `App.render()` does NOT apply styles - it only does template rendering. Styles are only applied by:

- `Renderer.render()`
- `render_auto()`
- `App.run_command()` (which calls `render_auto` internally)

I spent significant time debugging why BBCode tags were appearing literally in output when using `App.render()`.

**Suggestion:** Add prominent documentation explaining:

- `App` is primarily for CLI integration with clap (help, topics, command dispatch)
- For pure rendering with styles, use `Renderer` or standalone `render`/`render_auto` functions
- `App.render()` is a low-level method that skips style application

### 3. Template Includes Require Explicit Extensions

**Problem:** `{% include "_pad_line" %}` failed silently. Had to change to `{% include "_pad_line.jinja" %}`.

The `embed_templates!` docs mention extensionless lookup works, but this doesn't apply to Jinja `{% include %}` statements.

**Suggestion:** Document that `{% include %}` requires the full filename with extension.

### 4. `Renderer` Has Fixed OutputMode

**Problem:** `Renderer` takes `OutputMode` at construction time and has no setter. Our code needed per-render output mode control, which forced us to create a new `Renderer` for each render call (inefficient).

**Suggestion:** Either:

- Add `renderer.set_output_mode(mode)` method
- Or document this limitation clearly and recommend `render_auto()` for per-call mode control

### 5. `render_auto()` Doesn't Support Includes

**Problem:** The standalone `render_auto(template_content, data, theme, mode)` takes template content as a string, which means `{% include %}` won't work (no template registry).

**Suggestion:** Document which render functions support includes and which don't:

- `Renderer.render()` ✓ supports includes (has template registry)
- `render_auto()` ✗ no includes (takes raw template string)
- `render()` ✗ no includes

### 6. `render-only.md` Topic Was Helpful But Incomplete

**Problem:** The `render-only.md` topic showed the basic pattern but didn't cover:

- How to handle structured output modes (JSON/YAML)
- How to use embedded templates with `Renderer`
- The relationship between `Theme`, `StylesheetRegistry`, and `embed_styles!`

**Suggestion:** Expand with a more complete example showing:

```rust
// Load theme from embedded styles
let styles = embed_styles!("src/styles");
let mut registry: StylesheetRegistry = styles.into();
let theme = registry.get("default")?;

// Create renderer with templates
let mut renderer = Renderer::with_output(theme, mode)?;
renderer.add_template("list", include_str!("templates/list.jinja"))?;
renderer.add_template("_partial.jinja", include_str!("templates/_partial.jinja"))?;

// Render
let output = renderer.render("list", &data)?;
```

## Minor Issues

1. **`EmbeddedTemplates` type confusion** - `embed_templates!` returns `EmbeddedSource<TemplateResource>` but `Renderer.with_embedded()` takes `HashMap<String, String>`. The conversion isn't obvious.

2. **Error messages could be better** - "unknown filter: filter style is unknown" doesn't hint that `style()` was replaced with BBCode tags.

## Summary

The library is well-designed and the new BBCode tag syntax is cleaner. The main gaps are:

1. **Migration documentation** for users coming from `outstanding`
2. **Clearer separation** between `App` (CLI integration) and `Renderer` (pure rendering)
3. **Documentation of which render functions support what features** (includes, per-call modes, etc.)

Happy to discuss any of these points further!
