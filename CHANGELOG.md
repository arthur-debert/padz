<!-- generated - do not edit. See CHANGELOG/README.txt -->

# Changelog

## Unreleased

- Migrate changelog handling to the fragment-directory model
  (arthur-debert/release#201). Future entries go in
  CHANGELOG/unreleased-<slug>.md fragments via `bin/changelog add`.

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.8.2] - 2026-05-01

### Changed

- **Release pipeline migrated to canonical reusable workflow at
  `arthur-debert/release/.github/workflows/rust-cli.yml@v1`.** padz's
  `.github/workflows/release.yml` is now a thin caller (~25 lines
  instead of 600+). Bug fixes and improvements propagate via a single
  bump of the action's `@v1` ref instead of hand-edits across 6
  rust-CLIs. dodot v2.0.0 was the first consumer of the new pipeline;
  padz is the second.
- **Intel-mac dropped from release artifacts** (`x86_64-apple-darwin`).
  Per canonical: arm64-only macOS. Existing v1.8.1 and earlier remain
  available for Intel users via direct GH release download.

## [1.8.1] - 2026-04-29

- **Fixed**
  - **`padz init --link` now rejects linking a project to itself.** Running `padz init --link .` (or pointing `--link` at the current project's own root) inside an already-initialized store would write `.padz/link` containing the project's own path, creating a self-loop. Subsequent `padz` invocations then failed with `Link target '…/.padz' is itself a link. Chained links are not supported.`, because the chain-detection check tripped on the very link file that had just been written. The link command now compares the canonicalized local and target `.padz/` paths up front and refuses with `Cannot link '…' to itself.` before writing anything.

## [1.8.0] - 2026-04-28

- **Changed**
  - **Releases now run end-to-end in CI via `scripts/release`.** Triggering a release with `scripts/release <version|major|minor|patch>` queues a `workflow_dispatch` run that performs the version bump, `## [Unreleased]` roll, commit, tag, GitHub Release, multi-platform build (mac arm64+x86_64, linux x86_64+arm64), `.deb` attach, crates.io publish, and Homebrew formula push — all in CI. Replaces the previous local `cargo release` + tag-push trigger model. The local `release.toml` remains for ad-hoc dry-runs but is no longer the supported release path.
  - **macOS binaries are now Developer ID signed and Apple-notarized.** Earlier `brew install padz` on macOS triggered a Gatekeeper "unidentified developer" warning on first run because release binaries were adhoc/linker-signed only. The release workflow now imports the Developer ID Application certificate into a temporary keychain, signs each macOS binary with `codesign --options runtime` (hardened runtime + secure timestamp), and submits to Apple's notarization service via App Store Connect API key — all in CI. Both `aarch64-apple-darwin` (Apple Silicon) and `x86_64-apple-darwin` (Intel) binaries are now properly signed.

## [1.7.0] - 2026-04-25

## [1.7.0] - 2026-04-25

## [1.6.0] - 2026-04-25

## [1.6.0] - 2026-04-25

- **Fixed**
  - **Natural-language pad matching now respects bucket scope.** When you reference a pad by title (e.g. `padz open for`), the title search used to sweep across active *and* archived *and* deleted pads, so a term that appears in no visible title could still return multiple matches (and block the command) because of hits inside the deleted bucket. Matching is now scoped to the bucket each command operates on: `open`/`view`/`edit`/`delete`/`archive`/`pin`/`move`/`tag`/… only consider active pads, `restore`/`purge` only consider deleted pads, and `unarchive` only considers archived pads. Nested cases are handled correctly too — a deleted child under an active parent (path `7.d3`) is no longer counted as an active match.
  - **Better ambiguity errors.** When a title matches more than one pad but five or fewer, the error now lists each matching pad's display index and title so you can retype a more specific term or just use the index directly. The display index is colored like the rest of the list view (accent/yellow), and the matching substring inside each title and inside the search term itself is highlighted with the same yellow-background style search results use. Above five matches the error falls back to reporting the count. Styling is suppressed automatically when stderr is not a TTY (piped/redirected), so machine consumers still see plain text.

## [1.5.0] - 2026-04-24

## [1.5.0] - 2026-04-24

- **Added**
  - **Configurable list ordering** — New `ordering` config key selects the sort used for Display Indexes. Default is `created_at` (unchanged: newest-created first). Set `ordering = "updated_at"` (or `padz config set ordering updated_at`) to list most-recently-modified first. Under `updated_at`, a parent's effective sort key is the maximum `updated_at` across its subtree, computed at index time — so editing a deeply nested child surfaces its ancestor to the top instead of leaving it buried, without mutating the ancestor's stored `updated_at` (which is reserved as the file-mtime proxy used by store reconciliation). Sibling sort uses deterministic tie-breakers (timestamp → other timestamp → UUID) so Display Indexes don't jitter across runs when timestamps match. Display Indexes are not comparable across ordering switches — the DI for pad "3" under `created_at` is a different pad than "3" under `updated_at` — which is expected for a shell tool where indexes reflect the current view.

## [1.4.2] - 2026-04-23

## [1.4.2] - 2026-04-23

- **Added**
  - **Debian/Ubuntu packages** — every release now produces `.deb` packages for `amd64` and `arm64` and attaches them to the GitHub Release. Install with `sudo dpkg -i padz_<version>-1_<arch>.deb` (or via `apt install ./padz_<version>-1_<arch>.deb` for dependency resolution). The package's postinst script generates bash/zsh/fish completion scripts from the installed binary into `/usr/share/bash-completion/completions/`, `/usr/share/zsh/site-functions/`, and `/usr/share/fish/vendor_completions.d/`, so completions work system-wide for all users with no extra setup.
  - **`install.sh` one-liner installer** — `curl -fsSL https://raw.githubusercontent.com/arthur-debert/padz/main/install.sh | sh` detects macOS/Linux + arm/x86, downloads the matching release tarball, installs `padz` into `~/.local/bin/`, and runs `padz completion install` automatically. Honors `VERSION=vX.Y.Z` and `PREFIX=/usr/local` overrides for pinned/system installs.

## [1.4.1] - 2026-04-23

## [1.4.1] - 2026-04-23

- **Fixed**
  - **`padz completion install`** now writes working scripts. The hand-written bash/zsh scripts referenced `_CLAP_COMPLETE=<shell>`, but clap_complete's dynamic completion handler (wired up in `main.rs`) checks `COMPLETE=<shell>` — so every script written by previous versions was a silent no-op. Scripts are now generated by re-invoking the binary with `COMPLETE=<shell>` set, which guarantees the installed script matches the wire protocol the binary actually implements.

- **Added**
  - **Fish shell completions** — `padz completion --shell fish install` drops a script at `$XDG_CONFIG_HOME/fish/completions/padz.fish`, which fish auto-loads. No shell-profile edits required.
  - **Homebrew distribution** — padz is now published to the `arthur-debert/homebrew-tools` tap and intended to be installable via `brew install arthur-debert/tools/padz` on macOS (Apple Silicon + Intel) and Linux (x86_64 + ARM64). The formula is regenerated on every release from `.github/homebrew-formula.rb.tmpl` and pushed to the tap automatically. It also installs bash/zsh/fish completions into Homebrew's standard completion directories for brew-managed setups.

- **Changed**
  - **zsh completion install path** — moved from `~/.zfunc/_padz` to the more standard XDG location `$XDG_DATA_HOME/zsh/site-functions/_padz`. The installer now probes zsh itself to check whether the directory is already on `$fpath`; the one-time `fpath=(... $fpath)` hint is only printed when it isn't (so package-manager installs that land in an fpath-covered dir will say "done" instead of suggesting a redundant edit).
  - **bash completion post-install messaging** — on macOS the installer now detects whether the `bash-completion` package is actually present and, if not, tells the user to `brew install bash-completion@2`. Previously it suggested sourcing the file without flagging that the OS bash wouldn't auto-load completions at all.
  - **Release workflow refactor (internal)** — parameterized `.github/workflows/release.yml` with a top-of-file `env:` block (`BIN_NAME`, `CRATE_NAME`, `EXTRA_CRATES`, `PUBLISH_WAIT_SECONDS`) so the same workflow can be reused across cargo projects by editing only that block. Replaced per-crate sed-based version bumps with `cargo set-version --workspace`, which handles workspace members and intra-workspace dep pins generically. Added `x86_64-apple-darwin` (Intel Mac) to the release build matrix. Each tarball now includes `README.md`, `CHANGELOG.md`, and any `LICENSE*` file alongside the binary.

## [1.4.0] - 2026-04-23

## [1.4.0] - 2026-04-23

- **Added**
  - **`padz export --json`** — Full-fidelity archive format. Produces a `.tar.gz` that preserves all metadata (timestamps, pinning, delete protection, status, tags, parent relationships) alongside the raw pad files. Round-trippable: `padz import` auto-detects the archive and restores pads + their referenced tag registry entries into the destination store. Import is defensive per metadata field — unknown or malformed fields become warnings and never block the pad from landing. Parent IDs that aren't present in the archive are orphaned to root (hierarchy is only preserved when the full subtree is exported). Conflicts with `--single-file`.
  - **`padz export --with-metadata`** — Embeds per-pad metadata inline in each exported file, using the format-native dialect: YAML frontmatter with `padz.*` keys for `.md`, top-of-document `:: padz.KEY :: VALUE` annotations for `.lex`. Files use each pad's native extension instead of being normalized to `.txt`. Pads in `.txt` format are exported without metadata (txt has no metadata format) and surfaced in a trailing warning. `padz import` auto-detects the metadata block from the file extension, applies it with the same defensive per-field machinery as `--json`, and strips the metadata from the stored content. Conflicts with `--json` and `--single-file`.
  - **`padz clone` and `padz migrate`** — Move pads between padz stores. `padz clone <id>... --to <path>` copies pads from the current store into the store at `<path>` (keeping source); `--from <path>` does the reverse. `padz migrate` uses the same syntax but removes successfully copied pads from the source. Both preserve metadata through the same defensive per-field pipeline as the JSON/inline-metadata importers, so a metadata hiccup never blocks the file copy. Parent IDs are preserved when both ends of the relationship are part of the transfer (or when the parent already exists at the destination); otherwise they orphan to root. Referenced tag registry entries are merged into the destination without overwriting existing tags. `<path>` is smart-resolved: it can point at a `.padz/` directory, its parent, or any directory where `.padz/` exists upward.

- **Changed**
  - **Metadata import/transfer layering** — Internal refactor on the export/import epic. Markdown frontmatter now goes through `serde_yaml` in both directions instead of a hand-rolled YAML subset parser (which also did its own quoting). Per-field defensive JSON-to-metadata application moves from the commands layer to `Metadata::apply_json_patch` on the model, with dedicated unit tests; `commands/metadata_apply.rs` becomes a thin adapter that prefixes warnings with a source label. `commands/transfer.rs::copy_one_pad` no longer round-trips live `Metadata` through a JSON value on each cross-store copy — it forwards the source `Pad` with a one-line parent-orphan check. Cross-version defensive parsing stays where it's actually needed (reading archives on disk). No user-visible behavior change.

## [1.3.0] - 2026-04-22

## [1.3.0] - 2026-04-22

- **Changed**
  - **Scope discovery split into read and write modes** — Previously, a directory was only treated as a project root if it contained both `.git` and `.padz`. That silently broke `.padz` directories in non-git parents: `padz init` succeeded but every subsequent command still used global because the detection step missed the new `.padz`. Discovery is now two independent algorithms. **Reads** (list, view, search, …) walk up looking for `.padz` alone — `.git` is irrelevant. **Writes** (create, import) use the same read discovery first; if no `.padz` is found upward, they walk up looking for `.git` and auto-create `.padz` at the git root so new pads land inside their enclosing project instead of silently going global. `padz init` continues to create `.padz` at cwd unconditionally — it is user intent and is never blocked.

## [1.2.0] - 2026-04-16

## [1.2.0] - 2026-04-16

- **Added**
  - **`--show-status` flag for `list`** — Displays status icons (planned/in-progress/done) even in notes mode.
  - **Status icons on `complete`/`reopen` feedback** — When completing or reopening pads, the confirmation output now always shows status icons regardless of mode.

## [1.1.1] - 2026-04-14

## [1.1.1] - 2026-04-14

- **Fixed**
  - **`delete --completed` crashes when parent and child are both Done** — `get_descendant_ids` re-indexes all buckets, so when a Done child is processed before its Done parent (HashMap iteration order is non-deterministic), the child moves to Deleted first. The parent then finds the child as a descendant and tries to re-move it from Active, failing with `Pad not found`. Fixed by filtering already-processed IDs from the move list.

## [1.1.0] - 2026-04-14

## [1.1.0] - 2026-04-14

- **Added**
  - **`--all` flag for `list` and `search`** — Shows all three shards (active, archived, deleted) in a single view, grouped with section headers. Each shard keeps its own index namespace (`1`, `ar1`, `d1`).
  - **`--deleted` and `--archived` flags on `search`** — Search can now target specific shards, matching `list` parity. Previously `search` was hardcoded to active pads only.

## [1.0.2] - 2026-04-14

## [1.0.2] - 2026-04-14

- **Fixed**
  - **Search match line numbers looked like pad indexes** — `padz search` displayed content line numbers (e.g., `19`) that users mistook for child pad indexes (e.g., `9.19`), leading to confusing "not found" errors on `padz view 9.19`. Line numbers now render as `19L` in italic, visually distinguishing them from pad indexes. Context ellipsis also changed from `...` (3 chars) to `…` (1 char).

## [1.0.1] - 2026-04-08

- **Fixed**
  - **Config with stale keys silently ignored** — `padz config set format lex` followed by `padz create` would still create `.txt` files. Clapfig's strict mode (the default) rejects unknown keys in config files, and `initialize()` swallowed the error via `unwrap_or_default()`, silently falling back to `format = "txt"`. Config files with stale keys from schema evolution (e.g. `modes` from an older version) triggered this. Fixed by setting `strict(false)` on both config loading paths so unknown keys are tolerated.

## [1.0.0] - 2026-04-06

## [1.0.0] - 2026-04-06

- **Fixed**
  - **`edit`/`open` now support title matching** — `padz open On Ids` and `padz edit My Note` now work, matching the behavior of `view`, `copy`, and all other commands. Previously, `split_indexes_and_content` classified non-index args as inline content, leaving no index selector and erroring with "No pad index provided". When no args parse as indexes, they are now treated as a title search term.
  - **`padz init` now uses cwd instead of upward discovery** — Previously, `padz init` used the same `find_project_root()` upward-walk as other commands, which could silently resolve to a parent directory's store instead of creating one in the current directory. Init is a creation operation ("create a store here"), so it now always uses `cwd/.padz` directly. All other commands continue using upward discovery to find existing stores.
  - **Fall back to global scope when no project store is found** — Running `padz` outside any project (no `.padz` found by upward walk) now transparently uses the global store instead of pointing at a nonexistent `cwd/.padz` and showing "No pads yet". The `-g` flag is no longer required outside project directories.

## [0.28.0] - 2026-04-03

## [0.28.0] - 2026-04-03

- **Added**
  - **Interchangeable UUIDs and Display Indices** — Commands that accept pad IDs now accept UUIDs (full or short hex prefix) interchangeably with Display Indices. For example, `padz view 1 4 766d5dab` mixes DIs and a short UUID. Short UUIDs are the hex prefixes shown by `--short-uuid` listings. DI formats (`1`, `p1`, `d2`, `ar3`) take priority; hex strings that don't match a DI pattern are treated as UUID prefixes. Ambiguous prefixes (matching multiple pads) produce a clear error.
  - **Nested pad output** — `view`, `copy`, and `export` commands now recursively include children by default (`--tree`). Use `--flat` for the previous behavior (selected pad only) or `--indented` for 4-space indentation per nesting level. Centralized tree-walking logic ensures consistent behavior across all content-output commands.

- **Fixed**
  - **Nested pad creation via editor lost parent relationship** — `padz create -i <parent> -e` would create the child pad, but the parent-child link was silently destroyed before the editor opened. The root cause: `create::run` called `propagate_status_change` immediately after saving the (empty) pad, which triggered `list_pads` → reconciliation → garbage collection of the empty content file and its index entry (including `parent_id`). The pad was later recovered as an orphan with no parent. Fix: `create::run` no longer calls propagation; the CLI handler calls it after the pad has real content (post-editor or post-pipe).

## [0.27.1] - 2026-03-28

## [0.27.1] - 2026-03-28

## [0.27.0] - 2026-03-28

## [0.27.0] - 2026-03-28

- **Changed**
  - **Dynamic terminal width** — List and search output now adapts to the actual terminal width instead of being fixed at 100 columns. Minimum width is 30 characters; below that the output wraps. Titles truncate to fit narrow terminals.

## [0.26.0] - 2026-03-27

## [0.26.0] - 2026-03-27

- **Added**
  - **`padz copy` command** — Copy one or more pads to the clipboard without printing to stdout. Supports multiple IDs and `--peek` flag, just like `view`. Alias: `cp`.

## [0.25.2] - 2026-03-15

## [0.25.2] - 2026-03-15

- **Changed**
  - **Renamed `file_ext` to `format`** — The config key, CLI option, and internal APIs now use `format` instead of `file_ext`. Accepts aliases: `markdown` → `md`, `text` → `txt`. Breaking change in config (`padz.toml`): rename `file_ext = "md"` to `format = "md"`.
  - **`--format` flag on `create`** — Override the global format for a single pad: `padz create --format md "My Note"`. The override only affects the new pad; subsequent pads use the global setting.
  - **Mixed-format data store** — The store now fully supports pads with different file extensions. Changing the format setting does not migrate or rename existing pads. Reading/editing pads works regardless of their original extension.
  - **Extension-preserving updates** — Editing or updating an existing pad preserves its original file extension, even if the global format has changed since creation.

## [0.25.1] - 2026-03-09

## [0.25.1] - 2026-03-09

- **Fixed**
  - **Double title in `view` output** — `padz view` was rendering the title twice because `pad.content` (which includes the title) was passed directly to the template that also renders the title separately. Now extracts just the body before passing to the template.

## [0.25.0] - 2026-03-02

## [0.25.0] - 2026-03-02

- **Added**
  - **`padz delete --completed`** — Soft-delete all pads marked as done/completed in a single command. Replaces the previous `--done` flag. Logic moved from CLI handler to command layer for proper testability.
  - **`--completed` flag on `list` and `search`** — Filter to show only completed pads. Replaces the previous `--done` flag on `list` and adds the filter to `search` for the first time.

## [0.24.0] - 2026-03-01

## [0.24.0] - 2026-03-01

- **Fixed**
  - **Empty create no longer fills in "Untitled"** — `padz create` with no title now starts with a blank document. Quitting the editor without typing correctly aborts instead of creating a junk "Untitled" pad. Applies to quick-create, piped stdin, and interactive editor paths.
  - **Clipboard no longer duplicates the title** — `view`, `open`, and `create` (interactive editor) commands were copying title + full content (which already includes the title) to the clipboard. Now correctly extracts title and body before formatting.

## [0.23.0] - 2026-03-01

## [0.23.0] - 2026-03-01

- **Added**
  - **UUID support** — Pads can now be referenced by their UUID in any command that accepts pad IDs. UUIDs are parsed before range detection so hyphens in UUIDs don't confuse the range parser.
  - **`padz uuid <id>...`** — New command to print full UUIDs, one per line. Supports ranges (e.g. `padz uuid 1-3`). Clean output for scripting/piping.
  - **`--uuid` flag** on `list`, `search`, `peek`, and `view` — Shows short 8-char UUID prefix next to pad titles in list views, and full UUID in view output.

## [0.22.0] - 2026-02-28

## [0.22.0] - 2026-02-28

## [0.21.0] - 2026-02-28

## [0.21.0] - 2026-02-28

- **Added**
  - **`padz create -e / --editor`** — Force editor mode. Opens the editor regardless of mode (notes or todos). Opposite of `--no-editor`. Conflicts with `--no-editor` if both are specified.

## [0.20.0] - 2026-02-17

## [0.20.0] - 2026-02-17

- **Added**
  - **`padz init --link <PATH>`** — Persistent data directory linking. Allows multiple directories to share the same padz data store without passing `--data` on every invocation. Creates a `.padz/link` file that redirects to the target project's data. Supports `--unlink` to remove the link. Validates target is initialized and rejects chained links.

## [0.19.2] - 2026-02-16

## [0.19.2] - 2026-02-16

- **Changed**
  - Unified `padz tag` subcommand replacing `add-tag`, `remove-tag`, and `tags` commands
  - `padz tag add <id>... <tag>...` with positional args and auto-creation of tags
  - `padz tag list` and `padz tag list <id>...` now output just tag names (consistent format)

## [0.16.0] - 2026-01-30

## [0.16.0] - 2026-01-30

## [0.15.1] - 2026-01-30

## [0.15.1] - 2026-01-30

## [0.15.0] - 2026-01-30

## [0.15.0] - 2026-01-30

- **Added**
  - **Piped content support** - Create and update pads from shell pipelines:
    - `cat file.txt | padz create` - Create pad with piped content
    - `cat file.txt | padz open <id>` - Update existing pad with piped content
    - `cat file.txt | padz` - Naked invocation with pipe expands to create
  - **PADZ_GLOBAL_DATA environment variable** - Override global data directory (useful for testing)
  - **Tag support** - Organize pads with tags. Create tags with `padz tags create`, assign with `padz add-tag`, filter lists with `--tag`. Tags are scoped (project or global) and displayed inline in list views.
  - **Attribute abstraction layer** - Unified system for metadata attributes (pinned, deleted, status, tags). Provides `get_attr()`/`set_attr()` methods on Metadata with automatic handling of coupled fields (e.g., pinning sets both `is_pinned` and `delete_protected`). Includes `AttrFilter` for generic filtering, replacing separate filter functions.

- **Changed**
  - Refactored pin/unpin, delete/restore, and tagging commands to use the new attribute API
  - Unified `filter_by_todo_status()` and `filter_by_tags()` into generic `apply_attr_filters()`

- **Fixed**
  - Pad name matching now only searches titles, not content (e.g., `padz view About` no longer matches pads where "About" only appears in the body)

## [0.12.1] - 2026-01-15

## [0.12.1] - 2026-01-15

- **Added**
  - **Unified release workflow** - GitHub Actions workflow with dual triggers (workflow_dispatch or tag push), cross-platform binary builds (macOS ARM, Linux x64/ARM), automatic GitHub Releases with changelog-based release notes, and idempotent crates.io publishing

## [0.12.0] - 2026-01-15

## [0.12.0] - 2026-01-15

- **Added**
  - **Data path override** - `--data <PATH>` option to explicitly specify the data directory, useful for git worktrees or working from temp directories. Automatically appends `.padz` if the path doesn't end with it.

## [0.10.2] - 2026-01-12

## [0.10.2] - 2026-01-12

- **Updated outstanding to v0.12.0** - Compile-time embedding with macros
- **File-based YAML stylesheet** - Styles defined in `styles/default.yaml` with adaptive light/dark support
- **Compile-time embedding** - Using `embed_styles!` and `embed_templates!` macros for zero-overhead resource loading
- **Template extension** - Renamed templates from `.tmpl` to `.jinja` (new standard)

## [0.10.1] - 2026-01-11

- Switched to outstanding's `col()` filter for declarative table layout

## [0.10.0] - 2026-01-10

- **Added**
  - **Move command** - Move pads to new parent pads with `padz move`
  - **JSON output support** - All commands now support `--output json` for structured output
  - **Three-layer styling architecture** - Semantic style layer for templates

  - Migrated create/edit commands to declarative handler pattern
  - Moved shell completion hint to API layer
  - Updated outstanding-clap to v0.4.0 with --output flag
  - Refactored templates with includes for better readability

- **Fixed**
  - Require confirmation parameter for purge command
  - Status icon indentation alignment

## [0.9.8] - 2026-01-08

- **Changed**
  - **Tree support (nesting)** - Pads can now have parent-child relationships
  - **Todo status** - Notes as Todos with recursive status propagation
  - **Complete/reopen commands** - Mark pads as done or reopen them
  - **Recursive purge** - `--recursive` flag for safe subtree deletion
  - Improved shell completion with clap_complete
  - Comprehensive test coverage for API facade, store, and commands

  - Refactored Store to Backend pattern with PadStore/FsBackend separation
  - Unified pad representation to always use DisplayPad

- **Fixed**
  - Recursive tree filtering for status filters
  - Store content_path handling for scope errors

## [0.9.7] - 2026-01-07

- **Renamed crates** - CLI is now `padz`, library is `padzapp`
- Updated documentation paths for workspace structure

## [0.9.6] - 2026-01-07

- **Fixed**
  - Improved already-published detection in CI workflow

## [0.9.5] - 2026-01-07

- **Fixed**
  - Handle already-published crates in CI workflow

## [0.9.4] - 2026-01-06

- **Workspace conversion** - Converted to cargo workspace with separate lib and CLI crates

## [0.9.3] - 2026-01-06

- **Fixed**
  - Output padding for listing pads

## [0.9.2] - 2026-01-06

- **Added**
  - **Help topics system** - Project-vs-global help topic and comprehensive help output
  - Clipboard and piped input support for create command
  - Delete protection for pads
  - Improved nested repo detection for datastore

  - Replaced custom CLI help with outstanding-clap
  - Moved truncate_to_width to outstanding crate

- **Fixed**
  - Removed filesystem-touching integration test

## [0.9.1] - 2026-01-02

- **Added**
  - **Restore command** - Restore soft-deleted pads
  - Release bumping with cargo-release

- **Fixed**
  - Pre-commit hook to fail on clippy warnings

## [0.9.0] - 2026-01-02

- **Added**
  - **Range support** - Multi-ID commands now support ranges (e.g., `1-5`)
  - Git hash and commit date shown in version for non-release builds

- **Fixed**
  - Version output split into two lines for dev builds

## [0.8.10] - 2026-01-01

- Updated outstanding to 0.2.1 for nl filter

## [0.8.9] - 2025-12-31

- **Added**
  - **Peek option** - `--peek` flag for list and view commands to show content preview

- **Fixed**
  - Template whitespace handling with explicit newlines
  - Peek spacing and duplicate title display

## [0.8.7] - 2025-12-31

- **Added**
  - Ellipsis for truncated titles
  - List-title style for pad titles in list view

  - Improved listing styles

## [0.8.6] - 2025-12-31

- **Added**
  - **Single-file export** - `--single-file` option to merge pads into one document

## [0.8.5] - 2025-12-07

- **Fixed**
  - Allow dirty Cargo.lock in publish workflow

## [0.8.4] - 2025-12-07

- **Fixed**
  - Publish workflow fixes

## [0.8.3] - 2025-12-07

- **Added**
  - GitHub Actions workflow for crates.io publishing

## [0.8.2] - 2025-12-07

- **Fixed**
  - Yanked dependency version number

## [0.8.1] - 2025-12-07

- **Added**
  - **Title/search reference** - Reference pads by title or search term

## [0.8.0] - 2025-12-07

- **Added**
  - **Outstanding template system** - Styled CLI template rendering
  - **Purge command** - Remove soft-deleted pads permanently
  - **Export command** - Export pads to files
  - **Import command** - Import pads from files
  - **Doctor command** - Check and repair pad store consistency
  - Grouped help output using clap hooks
  - Adaptive color support with dark/light mode detection
  - Clipboard copy on view/edit/open/create
  - Help shown when no pads exist

  - Renamed binary from 'pa' to 'padz'
  - Unified list and search commands
  - Implemented lazy reconciler architecture
  - Centralized config key handling

- **Fixed**
  - Duplicate title in clipboard copy
  - Vertical alignment of time units in list output

## [0.6.0] - 2025-12-06

- **Added**
  - Dynamic shell completion support
  - Modular API facade architecture
  - CI workflow for tests
  - Multi-pad command support
  - Configuration system with file-ext setting
  - Path command to print pad file path
  - Clipboard support and open command
  - Edit command and update_pad API method
  - Editor support for create command with --no-editor bypass

  - Reverse chronological ordering (newest first)
  - Reorganized crate layout under src/padz

- **Fixed**
  - Timestamp alignment using unicode-width
  - Pinned pads appearing in both pinned and regular lists

## [0.3.1] - 2025-12-06

- **Added**
  - Initial release to crates.io
  - Basic pad creation, listing, and viewing
  - Demo flow verification script
  - Live testing shell

[1.7.0]: https://github.com/arthur-debert/padz/compare/v1.6.0...v1.7.0
[1.6.0]: https://github.com/arthur-debert/padz/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/arthur-debert/padz/compare/v1.4.2...v1.5.0
[1.4.2]: https://github.com/arthur-debert/padz/compare/v1.4.1...v1.4.2
[1.4.1]: https://github.com/arthur-debert/padz/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/arthur-debert/padz/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/arthur-debert/padz/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/arthur-debert/padz/compare/v1.1.1...v1.2.0
[1.1.1]: https://github.com/arthur-debert/padz/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/arthur-debert/padz/compare/v1.0.2...v1.1.0
[1.0.2]: https://github.com/arthur-debert/padz/compare/v1.0.1...v1.0.2
[1.0.0]: https://github.com/arthur-debert/padz/compare/v1.0.0-rc.1...v1.0.0
[0.28.0]: https://github.com/arthur-debert/padz/compare/v0.27.1...v0.28.0
[0.27.1]: https://github.com/arthur-debert/padz/compare/v0.27.0...v0.27.1
[0.27.0]: https://github.com/arthur-debert/padz/compare/v0.26.0...v0.27.0
[0.26.0]: https://github.com/arthur-debert/padz/compare/v0.25.2...v0.26.0
[0.25.2]: https://github.com/arthur-debert/padz/compare/v0.25.1...v0.25.2
[0.25.1]: https://github.com/arthur-debert/padz/compare/v0.25.0...v0.25.1
[0.25.0]: https://github.com/arthur-debert/padz/compare/v0.24.0...v0.25.0
[0.24.0]: https://github.com/arthur-debert/padz/compare/v0.23.0...v0.24.0
[0.23.0]: https://github.com/arthur-debert/padz/compare/v0.22.0...v0.23.0
[0.22.0]: https://github.com/arthur-debert/padz/compare/v0.21.0...v0.22.0
[0.21.0]: https://github.com/arthur-debert/padz/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/arthur-debert/padz/compare/v0.19.2...v0.20.0
[0.19.2]: https://github.com/arthur-debert/padz/compare/v0.19.1...v0.19.2
[0.16.0]: https://github.com/arthur-debert/padz/compare/v0.15.1...v0.16.0
[0.15.1]: https://github.com/arthur-debert/padz/compare/v0.15.0...v0.15.1
[0.15.0]: https://github.com/arthur-debert/padz/compare/v0.14.0...v0.15.0
[0.12.1]: https://github.com/arthur-debert/padz/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/arthur-debert/padz/compare/v0.11.0...v0.12.0
[0.10.2]: https://github.com/arthur-debert/padz/compare/v0.10.1...v0.10.2
[0.10.1]: https://github.com/arthur-debert/padz/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/arthur-debert/padz/compare/v0.9.8...v0.10.0
[0.9.8]: https://github.com/arthur-debert/padz/compare/v0.9.7...v0.9.8
[0.9.7]: https://github.com/arthur-debert/padz/compare/v0.9.6...v0.9.7
[0.9.6]: https://github.com/arthur-debert/padz/compare/v0.9.5...v0.9.6
[0.9.5]: https://github.com/arthur-debert/padz/compare/v0.9.4...v0.9.5
[0.9.4]: https://github.com/arthur-debert/padz/compare/v0.9.3...v0.9.4
[0.9.3]: https://github.com/arthur-debert/padz/compare/v0.9.2...v0.9.3
[0.9.2]: https://github.com/arthur-debert/padz/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/arthur-debert/padz/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/arthur-debert/padz/compare/v0.8.10...v0.9.0
[0.8.10]: https://github.com/arthur-debert/padz/compare/v0.8.9...v0.8.10
[0.8.9]: https://github.com/arthur-debert/padz/compare/v0.8.7...v0.8.9
[0.8.7]: https://github.com/arthur-debert/padz/compare/v0.8.6...v0.8.7
[0.8.6]: https://github.com/arthur-debert/padz/compare/v0.8.5...v0.8.6
[0.8.5]: https://github.com/arthur-debert/padz/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/arthur-debert/padz/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/arthur-debert/padz/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/arthur-debert/padz/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/arthur-debert/padz/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/arthur-debert/padz/compare/v0.6.0...v0.8.0
[0.6.0]: https://github.com/arthur-debert/padz/compare/v0.3.1...v0.6.0
[0.3.1]: https://github.com/arthur-debert/padz/releases/tag/v0.3.1
