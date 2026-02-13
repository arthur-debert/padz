# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.19.0] - 2026-02-13

## [0.19.0] - 2026-02-13

- **Added**
  - **Todo mode toggle** - New `mode` config (`notes` | `todos`, default `notes`) that adapts padz for note-taking or task management:
    - **Display**: Todos mode shows status icons (⚪︎ ☉︎︎ ⚫︎); notes mode hides them and gives more title width
    - **Quick-create**: In todos mode, `padz create Buy Milk` skips the editor; supports `\n` for multi-line (`padz create 'Buy Groceries\nMilk\nEggs'`)
    - **Quick-edit**: In todos mode, `padz edit 1 Updated Title` updates directly without opening the editor
    - **Purge**: In todos mode, `padz purge` removes both Done and Deleted pads; in notes mode only Deleted

- **Changed**
  - **Compact timestamps** - List timestamps now use compact format (` 3d ⏲`, `23h ⏲`) instead of verbose (`3 days ago`), reclaiming ~9 chars for title display. Removed `timeago` dependency.
  - **Unified pin marker position** - Pin marker `⚲` now appears in the left column (col 0) for both pinned and regular sections, instead of appearing on the right side in the regular list
  - **Fixed pinned section indentation** - Children of pinned pads now indent correctly, matching the regular list layout. Removed the `right_pin` column.

## [0.18.0] - 2026-02-10

## [0.18.0] - 2026-02-10

- **Added**
  - **Peek command** - `padz peek` / `padz peek 2 3` shows pad listing with content previews. Shortcut for `padz list --peek`, supports ID filtering and tag filtering.

## [0.17.0] - 2026-02-10

## [0.17.0] - 2026-02-10

- **Added**
  - **List command ID filtering** - `padz list 2`, `padz list 3 5`, `padz list 1-3` to constrain which pads are shown. Selected pads include their full subtree of children. Uses existing `parse_selectors` infrastructure for ID resolution (paths, ranges, titles).
  - **Bats live-tests in CI** - Added bats live-tests to pre-commit hook and CI workflow

- **Changed**
  - **Upgraded standout to v6.0.2** - Major upgrade through 3.8.0 → 4.0.0 → 5.0.0 → 6.0.2, adopting unified single-threaded API, `#[handler]` proc macro, declarative `InputChain` for stdin/editor input, `Output::Render` for template rendering, `pipe_to_clipboard` dispatch attribute, and explicit template mappings for all commands
  - **ScopedApi pattern** - All handlers migrated to `ScopedApi` wrapper that binds scope, handles error conversion, and wraps results. Handler bodies reduced from ~10 lines to 1-2 lines (~200+ lines of boilerplate removed)
  - **`#[handler]` macro migration** - All handlers use standout's `#[handler]` macro with `#[dispatch(pure)]` for auto-extraction of CLI args via `#[arg]`, `#[flag]`, and `#[ctx]` annotations
  - **Unified create/open editor flow** - Extracted shared `edit_and_copy_pads` helper; create now opens the real pad file in editor (same as open) instead of using a temp file
  - **Consolidated edit/open handlers** - Open dispatches to edit handler; extracted `read_piped_input()` helper shared by create and edit
  - **Normalized naked invocation** - Extracted `build_dispatch_app()` and `handle_dispatch_result()` for a unified dispatch path; naked invocation injects synthetic command and uses same flow
  - **View uses template rendering** - View handler returns structured data for `view.jinja` template with automatic clipboard piping (ANSI stripped) instead of manual `println!` and clipboard code

- **Fixed**
  - View output no longer consumes stdout — clipboard copy is separate from terminal rendering
  - Handler errors now propagate as non-zero exit codes
  - Create title priority: CLI title always wins over piped content title
  - Empty/whitespace piped content in edit returns error instead of silent success

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

[Unreleased]: https://github.com/arthur-debert/padz/compare/v0.19.0...HEAD
[0.19.0]: https://github.com/arthur-debert/padz/compare/v0.18.0...v0.19.0
[0.19.0]: https://github.com/arthur-debert/padz/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/arthur-debert/padz/compare/v0.17.0...v0.18.0
[0.18.0]: https://github.com/arthur-debert/padz/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/arthur-debert/padz/compare/v0.16.0...v0.17.0
[0.17.0]: https://github.com/arthur-debert/padz/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/arthur-debert/padz/compare/v0.15.1...v0.16.0
[0.16.0]: https://github.com/arthur-debert/padz/compare/v0.15.1...v0.16.0
[0.15.1]: https://github.com/arthur-debert/padz/compare/v0.15.0...v0.15.1
[0.15.1]: https://github.com/arthur-debert/padz/compare/v0.15.0...v0.15.1
[0.15.0]: https://github.com/arthur-debert/padz/compare/v0.14.0...v0.15.0
[0.15.0]: https://github.com/arthur-debert/padz/compare/v0.14.0...v0.15.0
[0.12.1]: https://github.com/arthur-debert/padz/compare/v0.12.0...v0.12.1
[0.12.1]: https://github.com/arthur-debert/padz/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/arthur-debert/padz/compare/v0.11.0...v0.12.0
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
