# Standout Upgrade Feedback

Upstream feedback gathered while upgrading padz across Standout generations.
Newest first.

- [6.2 -> 7.6](#62---76) — the current generation.
- [outstanding v0.14 -> standout v1.0](#outstanding-v014---standout-v10) — historical.

---

## 6.2 -> 7.6

Feedback from upgrading padz from `standout` 6.2.0 to 7.6.4 (STD01/WS01).

### What Changed (and what it cost)

The API surface held up well: three call sites needed edits.

| 6.2 | 7.6 | Notes |
| --- | --- | --- |
| `App::with_registry(reg)` | `App::new()` + `*app.registry_mut() = reg` | Constructor removed; the registry field is private. |
| `app.augment_command(cmd)` | `app.augment_command_with_help(cmd)` | Rename. |
| implicit help handling | `.help_handling(true)` | Now opt-in; `build()` rejects topics/command groups without it. |
| `RunResult::Handled("Error: ...")` | `RunResult::Error(msg)` | Native error variant; enum is now `#[non_exhaustive]`. |

`RunResult::Error` is a genuine improvement: padz previously had to sniff
`output.starts_with("Error:")` on the `Handled` payload to detect failures, which
would misfire on any command that legitimately rendered text starting with
`Error:`. That workaround is now deleted.

### Gap: global args missing from rendered help

**Problem:** Standout's help renderer omits clap `global = true` args.

padz declares `-g/--global`, `-v/--verbose` and `--data` as clap globals on the
root `Cli`. clap propagates globals into each subcommand during its internal
build, but `extract_help_data` reads the subcommand's *own* arg list, which at
that point does not contain them. `--output` / `--output-file-path`, which
Standout itself injects, are absent for the same reason.

In 6.2 this was mostly invisible: only `padz help create` went through
Standout's renderer, while `padz create --help` was rendered by clap and showed
the full set. 7.6's `help_handling(true)` also intercepts `--help`/`-h`, so both
paths now render through Standout and the flags disappear from both:

```text
# 6.2: `padz create --help` (clap-rendered)
Options:
  -e, --editor                   Force opening the editor (even in todos mode)
      --no-editor                Skip opening the editor
  -i, --inside <INSIDE>          Create inside another pad
  -f, --format <FORMAT>          File format for this pad
  -g, --global                   Operate on global pads          <-- gone in 7.6
  -v, --verbose                  Verbose output                  <-- gone in 7.6
      --data <PATH>              Override data directory path    <-- gone in 7.6
      --output <MODE>            Output format                   <-- gone in 7.6
      --output-file-path <PATH>  Write output to file            <-- gone in 7.6
  -h, --help                     Print help                      <-- gone in 7.6

# 7.6: `padz create --help` (standout-rendered)
OPTIONS
  -e, --editor  Force opening the editor (even in todos mode)
  --no-editor   Skip opening the editor
  -i, --inside  Create inside another pad (parent selector, e.g. 1 or p1)
  -f, --format  File format for this pad (e.g., md, txt, markdown, text)
  title         Title words (joined with spaces, optional)
```

The flags are still **accepted** — this is a documentation gap, not a parsing
regression (pinned by `test_global_flags_still_accepted_on_subcommands` in
`crates/padz/tests/help_e2e.rs`). Note the upside: `create --help` and
`help create` now agree, where 6.2 rendered them differently.

**Suggestion:** call clap's build (or `render_usage`-style propagation) on the
target `Command` before `extract_help_data`, so `global = true` args and
Standout's own injected flags appear. Failing that, document that Standout help
does not list global args, so applications can decide between per-subcommand
duplication and living with the omission.

We deliberately did **not** work around this in padz (no hand-maintained global
flag list in help output), per the "record the gap rather than bypass Standout"
rule.

---

## outstanding v0.14 -> standout v1.0

Feedback from migrating padz from `outstanding` (v0.14) to `standout` (v1.0 @ b1644ed).

### What Went Well

1. **The guides are excellent** - `intro-to-standout.md` and `intro-to-tabular.md` are well-written and comprehensive for new users.

2. **BBCode tag syntax is intuitive** - Once understood, `[style]content[/style]` is cleaner than `{{ x | style("name") }}`.

3. **The `style_as` filter** for dynamic styles works well.

4. **StylesheetRegistry API** - Loading themes from embedded styles via `embed_styles!` → `StylesheetRegistry` → `.get("default")` worked smoothly.

### Pain Points & Documentation Gaps

#### 1. No Migration Guide from `outstanding`

**Problem:** There's no documentation explaining what changed from `outstanding` to `standout`. I had to discover changes through trial and error.

**Suggestion:** Add a `migrating-from-outstanding.md` guide covering:

- Package rename: `outstanding` → `standout`, `outstanding-clap` → `standout` with `clap` feature
- Import path changes
- API renames (`OutstandingApp` → `App`, `RenderSetup` → `App::builder()`)
- Template syntax changes (`style()` filter → BBCode tags)

#### 2. `App.render()` vs `Renderer.render()` - Critical Difference Undocumented

**Problem:** `App.render()` does NOT apply styles - it only does template rendering. Styles are only applied by:

- `Renderer.render()`
- `render_auto()`
- `App.run_command()` (which calls `render_auto` internally)

I spent significant time debugging why BBCode tags were appearing literally in output when using `App.render()`.

**Suggestion:** Add prominent documentation explaining:

- `App` is primarily for CLI integration with clap (help, topics, command dispatch)
- For pure rendering with styles, use `Renderer` or standalone `render`/`render_auto` functions
- `App.render()` is a low-level method that skips style application

#### 3. Template Includes Require Explicit Extensions

**Problem:** `{% include "_pad_line" %}` failed silently. Had to change to `{% include "_pad_line.jinja" %}`.

The `embed_templates!` docs mention extensionless lookup works, but this doesn't apply to Jinja `{% include %}` statements.

**Suggestion:** Document that `{% include %}` requires the full filename with extension.

#### 4. `Renderer` Has Fixed OutputMode

**Problem:** `Renderer` takes `OutputMode` at construction time and has no setter. Our code needed per-render output mode control, which forced us to create a new `Renderer` for each render call (inefficient).

**Suggestion:** Either:

- Add `renderer.set_output_mode(mode)` method
- Or document this limitation clearly and recommend `render_auto()` for per-call mode control

#### 5. `render_auto()` Doesn't Support Includes

**Problem:** The standalone `render_auto(template_content, data, theme, mode)` takes template content as a string, which means `{% include %}` won't work (no template registry).

**Suggestion:** Document which render functions support includes and which don't:

- `Renderer.render()` ✓ supports includes (has template registry)
- `render_auto()` ✗ no includes (takes raw template string)
- `render()` ✗ no includes

#### 6. `render-only.md` Topic Was Helpful But Incomplete

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

### Minor Issues

1. **`EmbeddedTemplates` type confusion** - `embed_templates!` returns `EmbeddedSource<TemplateResource>` but `Renderer.with_embedded()` takes `HashMap<String, String>`. The conversion isn't obvious.

2. **Error messages could be better** - "unknown filter: filter style is unknown" doesn't hint that `style()` was replaced with BBCode tags.

### Summary

The library is well-designed and the new BBCode tag syntax is cleaner. The main gaps are:

1. **Migration documentation** for users coming from `outstanding`
2. **Clearer separation** between `App` (CLI integration) and `Renderer` (pure rendering)
3. **Documentation of which render functions support what features** (includes, per-call modes, etc.)

Happy to discuss any of these points further!
