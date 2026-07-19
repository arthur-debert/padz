# Process smoke coverage map

The audit started with 22 subprocess tests in six `*_e2e.rs` files and ends
with exactly seven tests in `process_smoke_e2e.rs`: 15 process cases removed,
with four missing direct proofs added below the process seam. Each retained
test owns a fact that requires a real child process; all command semantics and
rendering breadth live at smaller seams.

| Behavior | Smallest owning seam | Process proof |
| --- | --- | --- |
| Root, command, short/long, and topic help render before dispatch; unknown help fails | `cli::setup` command/group tests | `help_render_and_exit_contracts_stay_at_the_process_boundary` observes render + exit without terminating the test runner |
| Global flags after a subcommand and `init --link/--unlink` conflicts | `cli::setup` parser tests | None: pure clap facts |
| `config set` writes a value that a later invocation reloads | Config types, `init::initialize`, and format-store tests | `config_set_persists_for_a_later_invocation` |
| Stale keys do not hide known config; changing the default format does not rename existing files | `init::initialize` and `padzapp/tests/format_behavior.rs` | None: loader/store facts |
| Created/updated ordering, stable tie-breaks, recursive child ordering, and descendant activity surfacing its parent | `padzapp::index` deterministic timestamp tests | None: ordering E2E count is zero |
| Missing/uninitialized link targets and unlink-without-link errors | `padzapp::commands::init` tests | None: core validation facts |
| A link written in one cwd redirects discovery in a later process launched from another cwd | `padzapp::init` link-resolution tests | `linked_working_directory_discovers_the_target_store_across_invocations` |
| Naked terminal/piped/empty-pipe and explicit-command precedence | `tests/harness.rs` input-resolution matrix | `naked_piped_invocation_uses_the_two_stage_resolver` proves the binary installs the resolver in both construction stages |
| Empty, singleton, multiple, and nested results | `structured_output_harness::empty_singleton_multiple_and_nested_results_remain_structured`; `handlers_direct::{copy_maps_a_single_root_to_typed_facts_and_one_semantic_write, copy_maps_multiple_roots_in_display_order, copy_keeps_nested_content_in_the_payload_but_counts_only_the_root}` | None: result-cardinality and nesting facts |
| Human text for singleton and plural results | `harness::{copy_template_preserves_human_wording_and_pluralization, empty_export_remains_non_artifact_output}` and `presentation_seams::pin_template_owns_the_compatible_human_sentence` | `human_success_reaches_stdout_with_clean_stderr` proves the final stdout/stderr/exit hop |
| Warning, partial-success, and failure results | `harness::{import_preserves_warning_text_and_exposes_partial_success_facts, clone_empty_and_parent_orphaning_are_explicit_states, artifact_write_failure_is_typed_and_emits_no_success_report}` | None: typed outcome and template facts |
| JSON/YAML/XML/CSV breadth and message-free structured schemas | `structured_output_harness::{every_read_command_serializes_in_every_structured_mode, mutating_commands_serialize_in_every_structured_mode, warning_and_failure_paths_use_structured_and_typed_harness_seams}` and `presentation_seams::typed_pin_handler_exposes_a_semantic_action_without_generic_messages` | `message_free_json_schema_reaches_structured_stdout` parses the final stdout and pins the WS10 top-level schema |
| Export empty/single/multiple/nested/warning results and typed final-write failures | Core export/import tests; `handlers_direct::{export_maps_core_bytes_suggestion_and_report_without_writing, empty_export_stays_a_non_artifact_result}`; harness artifact tests | `artifact_destination_round_trips_and_write_failure_is_truthful` proves compatible bytes, explicit placement, stderr, non-zero exit, and no false success |

The audit intentionally excludes the Bats/release-binary suite. Every retained
fixture supplies both an isolated cwd and `PADZ_GLOBAL_DATA`; no retained test
uses sleeps or the deprecated runtime `cargo_bin(name)` lookup.
