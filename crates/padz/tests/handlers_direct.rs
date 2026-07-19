//! Direct typed-handler tests — layer 2 of the pyramid (see `src/lib.rs`).
//!
//! # The seam this file protects
//!
//! A padz handler is an **adapter**: it takes typed arguments, calls the API, and
//! returns a typed, mode-independent result. These tests call those functions with
//! real Rust values — no `ArgMatches`, no clap, no rendering — so a failure here
//! means *the mapping is wrong*, and can't mean a template moved or a flag was
//! renamed.
//!
//! ## What is deliberately not tested here
//!
//! - **Domain behavior** (does archiving actually move the file?) belongs to
//!   `padzapp`'s own tests. These tests assert the *mapping*: that `archive`
//!   reports the `"Archived"` action for the pads the user selected.
//! - **Flag-to-argument wiring** (does `--peek` reach the `peek` parameter?) is a
//!   clap concern and is proven at the harness seam, which parses real argv.
//! - **Rendering** of any kind. A handler that returns the right result and a
//!   template that draws it wrong are two different bugs.
//!
//! These tests need no `#[serial]`: the fixture hands the store in as a value, so
//! nothing here touches process-global state.

mod support;

use padz::cli::handlers;
use padz::cli::input::{RequestContent, CREATE_CONTENT, EDIT_CONTENT};
use padz::cli::result::{
    CopyResult, CreateAbortKindResult, CreateAbortReasonResult, CreateResult, DoctorResult,
    ExportFormat, ExportStatus, ExportWarning, ImportDiagnosticResult, ImportResult,
    ImportSourceKindResult, ImportSourceStatusResult, ImportStatusResult, InitializationResult,
    MetadataWarningReasonResult, ModificationActionResult, ModificationNoticeResult,
    ModificationResult, MutationOutcomeResult, MutationStatusResult, PadContentResult,
    PadListResult, PathResult, PurgeResult, TagCatalogResult, TagRegistryResult, TaggingResult,
    TransferDirectionResult, TransferOperationResult, TransferResult, TransferSelectionResult,
    TransferStatusResult, UpdateKindResult, UuidResult,
};
use padzapp::commands::NestingMode;
use standout::cli::Output;
use support::Fixture;

/// Unwraps the `Output::Render` payload every padz read/modify handler returns.
#[track_caller]
fn rendered<T>(out: Result<Output<T>, anyhow::Error>) -> T
where
    T: serde::Serialize,
{
    match out.expect("handler returned an error") {
        Output::Render(value) => value,
        Output::Silent => panic!("expected Output::Render, got Silent"),
        Output::Binary { .. } => panic!("expected Output::Render, got Binary"),
        Output::Artifact(_) => panic!("expected Output::Render, got Artifact"),
        _ => panic!("expected Output::Render, got a newer output variant"),
    }
}

/// The titles a listing result carries, in the order the handler returned them.
fn titles(result: &PadListResult) -> Vec<String> {
    result
        .pads
        .iter()
        .map(|p| p.pad.metadata.title.clone())
        .collect()
}

/// Unwraps the successful modification arm of create without changing its public
/// serialized shape.
#[track_caller]
fn created(out: Result<Output<CreateResult>, anyhow::Error>) -> ModificationResult {
    match rendered(out) {
        CreateResult::Created(result) => result,
        CreateResult::Aborted(abort) => panic!("expected created pad, got {abort:?}"),
    }
}

// =============================================================================
// Listing family — list / peek / search
// =============================================================================

#[test]
fn list_maps_no_filters_to_every_active_pad() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "first", "body one");
    fx.seed_pad(&state, "second", "body two");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::list(
        &ctx,
        vec![],
        None,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        vec![],
        false,
        false,
    ));

    let mut got = titles(&result);
    got.sort();
    assert_eq!(got, vec!["first", "second"]);
}

#[test]
fn list_maps_search_argument_to_a_filtered_result() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "alpha", "");
    fx.seed_pad(&state, "beta", "");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::list(
        &ctx,
        vec![],
        Some("alpha".to_string()),
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        vec![],
        false,
        false,
    ));

    assert_eq!(titles(&result), vec!["alpha"]);
    // `filtered` is what tells the template "nothing matched" vs "no pads yet".
    assert!(
        result.request.filtered,
        "a search narrows the listing, so the result must report itself as filtered"
    );
}

#[test]
fn list_maps_peek_flag_onto_the_request_not_the_pads() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "notes", "the body");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::list(
        &ctx,
        vec![],
        None,
        false,
        false,
        false,
        true, // peek
        false,
        false,
        false,
        vec![],
        false,
        false,
    ));

    assert!(
        result.request.peek,
        "--peek is a request fact, carried on the result for the view builder to read"
    );
}

#[test]
fn peek_is_list_with_previews_requested() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "notes", "the body");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::peek(&ctx, vec![], vec![], false));

    assert_eq!(titles(&result), vec!["notes"]);
    assert!(result.request.peek, "peek always requests previews");
}

#[test]
fn search_maps_its_term_to_matching_pads_only() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "shopping list", "");
    fx.seed_pad(&state, "meeting notes", "");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::search(
        &ctx,
        "meeting".to_string(),
        false,
        false,
        false,
        false,
        vec![],
        false,
    ));

    assert_eq!(titles(&result), vec!["meeting notes"]);
}

// =============================================================================
// Content family — view
// =============================================================================

#[test]
fn view_maps_a_selector_to_that_pads_title_and_body() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "recipe", "mix and bake");
    let ctx = support::ctx_with_state(state);

    let result: PadContentResult = rendered(handlers::view(
        &ctx,
        vec!["1".to_string()],
        false,
        false,
        false,
        false,
        false,
    ));

    assert_eq!(result.pads.len(), 1);
    assert_eq!(result.pads[0].title, "recipe");
    assert!(result.pads[0].content.contains("mix and bake"));
    assert!(
        result.pads[0].uuid.is_none(),
        "uuid is omitted unless --uuid asked for it"
    );
}

#[test]
fn view_includes_the_uuid_only_when_requested() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "recipe", "mix and bake");
    let ctx = support::ctx_with_state(state);

    let result: PadContentResult = rendered(handlers::view(
        &ctx,
        vec!["1".to_string()],
        false,
        true, // uuid
        false,
        false,
        false,
    ));

    assert!(result.pads[0].uuid.is_some());
}

#[test]
fn indented_view_returns_raw_content_plus_nesting_facts() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "parent", "parent body");
    fx.seed_child(&state, "1", "child", "child body\nsecond line");
    let ctx = support::ctx_with_state(state);

    let result: PadContentResult = rendered(handlers::view(
        &ctx,
        vec!["1".to_string()],
        false,
        false,
        false,
        false,
        true,
    ));

    assert_eq!(result.nesting, NestingMode::Indented);
    assert_eq!(result.pads[1].depth, 1);
    assert_eq!(result.pads[1].title, "child");
    assert_eq!(result.pads[1].content, "child body\nsecond line");
    assert!(!result.pads[1].title.starts_with(' '));
    assert!(!result.pads[1].content.starts_with(' '));
}

#[test]
fn view_of_an_unknown_selector_is_an_error_not_an_empty_result() {
    let fx = Fixture::new();
    let ctx = fx.ctx();

    let err = handlers::view(
        &ctx,
        vec!["99".to_string()],
        false,
        false,
        false,
        false,
        false,
    )
    .expect_err("viewing a pad that does not exist must fail");

    // An empty result would render as "nothing to show" — a silent lie about a
    // selector the user got wrong.
    assert!(
        err.to_string().to_lowercase().contains("99")
            || err.to_string().to_lowercase().contains("not found"),
        "error should name the bad selector, got: {err}"
    );
}

// =============================================================================
// Content family — copy
// =============================================================================

#[test]
fn copy_maps_a_single_root_to_typed_facts_and_one_semantic_write() {
    let fx = Fixture::new();
    let (state, clipboard) = fx.app_state_with_recording_clipboard_for(&["copy", "1"]);
    fx.seed_pad(&state, "single", "body");
    let ctx = support::ctx_with_state(state);

    let result: CopyResult = rendered(handlers::copy(
        &ctx,
        vec!["1".to_string()],
        false,
        false,
        false,
        false,
    ));

    assert_eq!(
        result,
        CopyResult {
            root_pad_count: 1,
            titles: vec!["single".to_string()],
        }
    );
    assert_eq!(clipboard.writes(), vec!["single\n\nbody"]);
}

#[test]
fn copy_maps_multiple_roots_in_display_order() {
    let fx = Fixture::new();
    let (state, clipboard) = fx.app_state_with_recording_clipboard_for(&["copy", "1", "2"]);
    fx.seed_pad(&state, "first", "one");
    fx.seed_pad(&state, "second", "two");
    let ctx = support::ctx_with_state(state);

    let result: CopyResult = rendered(handlers::copy(
        &ctx,
        vec!["1".to_string(), "2".to_string()],
        false,
        false,
        false,
        false,
    ));

    assert_eq!(result.root_pad_count, 2);
    assert_eq!(result.titles, vec!["second", "first"]);
    assert_eq!(
        clipboard.writes(),
        vec!["second\n\ntwo\n---\n\nfirst\n\none"]
    );
}

#[test]
fn copy_keeps_nested_content_in_the_payload_but_counts_only_the_root() {
    let fx = Fixture::new();
    let (state, clipboard) = fx.app_state_with_recording_clipboard_for(&["copy", "1"]);
    fx.seed_pad(&state, "parent", "parent body");
    fx.seed_child(&state, "1", "child", "child body");
    let ctx = support::ctx_with_state(state);

    let result: CopyResult = rendered(handlers::copy(
        &ctx,
        vec!["1".to_string()],
        false,
        false,
        false,
        false,
    ));

    assert_eq!(result.root_pad_count, 1);
    assert_eq!(result.titles, vec!["parent"]);
    assert_eq!(
        clipboard.writes(),
        vec!["parent\n\nparent body\n\nchild\n\nchild body"]
    );
}

// =============================================================================
// Modification family — the semantic action each maps to
// =============================================================================

/// Every modification handler's contract is the same shape: act on the selected
/// pads and report a semantic action. Checking the family in one table is what
/// keeps a new action from being added without a test.
#[test]
fn modification_handlers_report_their_semantic_action() {
    type Call =
        fn(&standout_dispatch::CommandContext) -> Result<Output<ModificationResult>, anyhow::Error>;

    let cases: Vec<(&str, ModificationActionResult, Call)> = vec![
        ("pin", ModificationActionResult::Pin, |ctx| {
            handlers::pin(ctx, vec!["1".to_string()])
        }),
        ("delete", ModificationActionResult::Delete, |ctx| {
            handlers::delete(ctx, vec!["1".to_string()], false)
        }),
        ("archive", ModificationActionResult::Archive, |ctx| {
            handlers::archive(ctx, vec!["1".to_string()])
        }),
        ("complete", ModificationActionResult::Complete, |ctx| {
            handlers::complete(ctx, vec!["1".to_string()])
        }),
    ];

    for (name, expected_action, call) in cases {
        let fx = Fixture::new();
        let state = fx.app_state();
        fx.seed_pad(&state, "target", "body");
        let ctx = support::ctx_with_state(state);

        let result = rendered(call(&ctx));

        assert_eq!(
            result.action, expected_action,
            "{name} should report the {expected_action:?} action"
        );
        assert_eq!(
            result.pads.len(),
            1,
            "{name} should report the pad it acted on"
        );
        assert_eq!(result.pads[0].pad.metadata.title, "target");
    }
}

#[test]
fn status_changing_handlers_request_status_icons_regardless_of_mode() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "task", "");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::complete(&ctx, vec!["1".to_string()]));

    // `complete` changes status, so the result asks for icons even in notes mode —
    // otherwise the user changes a status and sees no sign of it.
    assert!(result.request.status);
}

#[test]
fn plain_modification_handlers_do_not_request_status_icons_in_notes_mode() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "note", "");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::pin(&ctx, vec!["1".to_string()]));

    assert!(!result.request.status);
}

#[test]
fn repeated_pin_maps_the_core_notice_without_parsing_prose() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "note", "");
    let ctx = support::ctx_with_state(state);

    rendered(handlers::pin(&ctx, vec!["1".to_string()]));
    let result = rendered(handlers::pin(&ctx, vec!["p1".to_string()]));

    assert_eq!(
        result.notices,
        vec![ModificationNoticeResult::AlreadyPinned {
            path: vec![padzapp::index::DisplayIndex::Pinned(1)]
        }]
    );
}

#[test]
fn unpin_reverses_pin_and_reports_its_own_verb() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "note", "");
    let ctx = support::ctx_with_state(state);

    rendered(handlers::pin(&ctx, vec!["1".to_string()]));
    let result = rendered(handlers::unpin(&ctx, vec!["p1".to_string()]));

    assert_eq!(result.action, ModificationActionResult::Unpin);
    assert!(!result.pads[0].pad.metadata.is_pinned);
}

#[test]
fn restore_maps_a_deleted_selector_back_to_active() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "gone", "");
    let ctx = support::ctx_with_state(state);

    rendered(handlers::delete(&ctx, vec!["1".to_string()], false));
    let result = rendered(handlers::restore(&ctx, vec!["d1".to_string()]));

    assert_eq!(result.action, ModificationActionResult::Restore);
    assert_eq!(result.pads[0].pad.metadata.title, "gone");
}

#[test]
fn move_without_root_needs_a_source_and_a_destination() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "only", "");
    let ctx = support::ctx_with_state(state);

    let err = handlers::move_pads(&ctx, vec!["1".to_string()], false)
        .expect_err("a one-argument move has no destination and must fail");

    assert!(
        err.to_string().contains("at least 2"),
        "the error should say what the user must supply, got: {err}"
    );
}

#[test]
fn move_to_root_needs_only_the_sources() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "parent", "");
    fx.seed_pad(&state, "child", "");
    let ctx = support::ctx_with_state(state);

    // Nest child under parent, then pull it back out with --root.
    rendered(handlers::move_pads(
        &ctx,
        vec!["1".to_string(), "2".to_string()],
        false,
    ));
    let result = rendered(handlers::move_pads(&ctx, vec!["1.1".to_string()], true));

    assert_eq!(result.action, ModificationActionResult::Move);
    assert!(
        result.pads[0].pad.metadata.parent_id.is_none(),
        "--root detaches the pad from its parent"
    );
}

#[test]
fn same_parent_move_maps_the_nested_no_op_path() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "parent", "");
    fx.seed_child(&state, "1", "child", "");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::move_pads(
        &ctx,
        vec!["1.1".to_string(), "1".to_string()],
        false,
    ));

    assert!(result.pads.is_empty());
    assert_eq!(
        result.notices,
        vec![ModificationNoticeResult::AlreadyAtDestination {
            path: vec![
                padzapp::index::DisplayIndex::Regular(1),
                padzapp::index::DisplayIndex::Regular(1),
            ],
        }]
    );
    assert!(result.outcomes.is_empty());
}

#[test]
fn mixed_complete_maps_changed_pads_and_requested_status_no_ops() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "changed", "");
    fx.seed_pad(&state, "no op", "");
    let ctx = support::ctx_with_state(state);
    rendered(handlers::complete(&ctx, vec!["1".to_string()]));

    let result = rendered(handlers::complete(
        &ctx,
        vec!["1".to_string(), "2".to_string()],
    ));

    assert_eq!(result.pads.len(), 2);
    assert_eq!(
        result.notices,
        vec![ModificationNoticeResult::AlreadyInStatus {
            path: vec![padzapp::index::DisplayIndex::Regular(1)],
            status: MutationStatusResult::Done,
        }]
    );
    assert_eq!(
        result.outcomes,
        vec![MutationOutcomeResult::StatusChanged {
            path: vec![padzapp::index::DisplayIndex::Regular(2)],
            status: MutationStatusResult::Done,
        }]
    );
}

#[test]
fn empty_delete_completed_maps_the_core_no_op() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "still open", "");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::delete(&ctx, vec![], true));

    assert!(result.pads.is_empty());
    assert_eq!(
        result.notices,
        vec![ModificationNoticeResult::NoCompletedPads]
    );
}

// =============================================================================
// Tag catalog and mutation outcomes
// =============================================================================

#[test]
fn tag_list_maps_empty_and_ordered_catalog_states() {
    let empty = Fixture::new();
    let result = rendered(handlers::tag::list(&empty.ctx(), vec![]));
    assert_eq!(result, TagCatalogResult::Empty { tags: vec![] });

    let fx = Fixture::new();
    let state = fx.app_state();
    state
        .with_api(|api| api.create_tag(state.scope, "work"))
        .unwrap();
    state
        .with_api(|api| api.create_tag(state.scope, "rust"))
        .unwrap();
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::tag::list(&ctx, vec![]));
    assert_eq!(
        result,
        TagCatalogResult::Listed {
            tags: vec!["work".into(), "rust".into()]
        }
    );
}

#[test]
fn tag_list_maps_selected_pad_tags_to_a_singleton_catalog() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.add_tags_to_pads(state.scope, &["1"], &["work".into()]))
        .unwrap();
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::tag::list(&ctx, vec!["1".into()]));
    assert_eq!(
        result,
        TagCatalogResult::Listed {
            tags: vec!["work".into()]
        }
    );
}

#[test]
fn tag_assignment_maps_requested_tags_counts_and_no_op_kind() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    let ctx = support::ctx_with_state(state);
    let args = vec!["1".into(), "work".into(), "rust".into()];

    let changed = rendered(handlers::tag::add(&ctx, args.clone()));
    match changed {
        TaggingResult::Assigned {
            requested_tags,
            modified_pads,
            pads,
        } => {
            assert_eq!(requested_tags, vec!["work", "rust"]);
            assert_eq!(modified_pads, 1);
            assert_eq!(pads.len(), 1);
        }
        other => panic!("expected assigned outcome, got {other:?}"),
    }

    let no_op = rendered(handlers::tag::add(&ctx, args));
    match no_op {
        TaggingResult::AllAlreadyPresent {
            requested_tags,
            modified_pads,
            pads,
        } => {
            assert_eq!(requested_tags, vec!["work", "rust"]);
            assert_eq!(modified_pads, 0);
            assert_eq!(pads.len(), 1);
        }
        other => panic!("expected all-already-present outcome, got {other:?}"),
    }
}

#[test]
fn tag_removal_maps_requested_tags_counts_and_no_op_kind() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    let ctx = support::ctx_with_state(state);
    let args = vec!["1".into(), "work".into()];
    rendered(handlers::tag::add(&ctx, args.clone()));

    let changed = rendered(handlers::tag::remove(&ctx, args.clone()));
    assert!(matches!(
        changed,
        TaggingResult::Removed {
            requested_tags,
            modified_pads: 1,
            ..
        } if requested_tags == vec!["work"]
    ));

    let no_op = rendered(handlers::tag::remove(&ctx, args));
    assert!(matches!(
        no_op,
        TaggingResult::NonePresent {
            requested_tags,
            modified_pads: 0,
            ..
        } if requested_tags == vec!["work"]
    ));
}

#[test]
fn tag_registry_handlers_map_names_and_affected_pad_counts() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.create_tag(state.scope, "old"))
        .unwrap();
    state
        .with_api(|api| api.add_tags_to_pads(state.scope, &["1"], &["old".into()]))
        .unwrap();
    let ctx = support::ctx_with_state(state);

    let renamed = rendered(handlers::tag::rename(&ctx, "old".into(), "new".into()));
    assert_eq!(
        renamed,
        TagRegistryResult::Renamed {
            old_name: "old".into(),
            new_name: "new".into(),
            affected_pads: 1,
        }
    );

    let deleted = rendered(handlers::tag::delete(&ctx, "new".into()));
    assert_eq!(
        deleted,
        TagRegistryResult::Deleted {
            name: "new".into(),
            affected_pads: 1,
        }
    );
}

// =============================================================================
// Selector families — path / uuid
// =============================================================================

#[test]
fn path_maps_selectors_to_one_filesystem_path_each() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "first", "");
    fx.seed_pad(&state, "second", "");
    let ctx = support::ctx_with_state(state);

    let result: PathResult = rendered(handlers::path(&ctx, vec!["1".to_string(), "2".to_string()]));

    assert_eq!(result.paths.len(), 2);
    for path in &result.paths {
        assert!(
            std::path::Path::new(path).exists(),
            "path handler should return a real file, got {path:?}"
        );
    }
}

#[test]
fn uuid_maps_single_multiple_and_range_selectors_in_order() {
    let fx = Fixture::new();
    let state = fx.app_state();
    let first = state
        .with_api(|api| api.create_pad(state.scope, "first".into(), "".into(), None))
        .unwrap()
        .affected_pads[0]
        .pad
        .metadata
        .id;
    let second = state
        .with_api(|api| api.create_pad(state.scope, "second".into(), "".into(), None))
        .unwrap()
        .affected_pads[0]
        .pad
        .metadata
        .id;
    let ctx = support::ctx_with_state(state);

    let single: UuidResult = rendered(handlers::uuid(&ctx, vec!["1".to_string()]));
    assert_eq!(single.uuids, vec![second.to_string()]);

    let multiple: UuidResult =
        rendered(handlers::uuid(&ctx, vec!["2".to_string(), "1".to_string()]));
    assert_eq!(multiple.uuids, vec![first.to_string(), second.to_string()],);

    let range: UuidResult = rendered(handlers::uuid(&ctx, vec!["1-2".to_string()]));
    assert_eq!(range.uuids, vec![second.to_string(), first.to_string()]);
}

// =============================================================================
// Export artifacts
// =============================================================================

#[test]
fn export_maps_core_bytes_suggestion_and_report_without_writing() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "plain text", "body");
    let ctx = support::ctx_with_state(state);

    let Output::Artifact(artifact) =
        handlers::export(&ctx, None, false, true, vec![], false, false, false)
            .expect("export handler failed")
    else {
        panic!("expected an artifact");
    };

    assert_eq!(&artifact.bytes()[..2], &[0x1f, 0x8b], "tar.gz magic");
    assert!(artifact
        .suggested_destination()
        .is_some_and(|path| path.to_string_lossy().ends_with(".meta.gz")));

    let report = artifact.report().expect("artifact report");
    assert_eq!(report.status, ExportStatus::Exported);
    assert_eq!(report.format, ExportFormat::MetadataArchive);
    assert_eq!(report.exported, 1);
    assert!(matches!(
        &report.warnings[0],
        ExportWarning::MetadataUnavailable {
            count: 1,
            additional: 0,
            ..
        }
    ));
}

#[test]
fn empty_export_stays_a_non_artifact_result() {
    let fx = Fixture::new();
    let ctx = fx.ctx();

    let report = rendered(handlers::export(
        &ctx,
        None,
        false,
        false,
        vec![],
        false,
        false,
        false,
    ));

    assert_eq!(report.status, ExportStatus::Empty);
    assert_eq!(report.format, ExportFormat::Archive);
    assert_eq!(report.exported, 0);
}

// =============================================================================
// Semantic import reports
// =============================================================================

#[test]
fn import_maps_core_source_and_metadata_warning_facts() {
    let fx = Fixture::new();
    let source = fx.root().join("bad.md");
    std::fs::write(
        &source,
        "---\npadz.status: NotAThing\n---\n\nImported title\n\nBody",
    )
    .unwrap();
    let state = fx.app_state_for(&["import", source.to_str().unwrap()]);
    let ctx = support::ctx_with_state(state);

    let result: ImportResult = rendered(handlers::import(&ctx, vec![source.display().to_string()]));

    assert_eq!(result.status, ImportStatusResult::PartialSuccess);
    assert_eq!(result.total_imported, 1);
    assert_eq!(result.sources[0].source, source.display().to_string());
    assert_eq!(result.sources[0].source_kind, ImportSourceKindResult::File);
    assert_eq!(result.sources[0].status, ImportSourceStatusResult::Imported);
    assert!(result.sources[0]
        .diagnostics
        .iter()
        .any(|diagnostic| matches!(
            diagnostic,
            ImportDiagnosticResult::MetadataWarning { warning }
                if warning.reason == MetadataWarningReasonResult::InvalidValue
                    && warning.field.as_deref() == Some("status")
        )));
}

// =============================================================================
// Cross-store transfer reports
// =============================================================================

#[test]
fn clone_maps_operation_direction_peer_selection_and_copied_ids() {
    let source = Fixture::new();
    let peer = Fixture::new();
    let state = source.app_state();
    source.seed_pad(&state, "transfer me", "body");
    let ctx = support::ctx_with_state(state);

    let result: TransferResult = rendered(handlers::clone(
        &ctx,
        vec!["1".to_string()],
        Some(peer.project().display().to_string()),
        None,
    ));

    assert_eq!(result.status, TransferStatusResult::FullSuccess);
    assert_eq!(result.operation, TransferOperationResult::Clone);
    assert_eq!(result.direction, TransferDirectionResult::To);
    assert_eq!(
        result.peer_store,
        peer.project()
            .join(".padz")
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
    );
    assert_eq!(
        result.requested_selection,
        TransferSelectionResult::Explicit {
            selectors: vec!["1".to_string()]
        }
    );
    assert_eq!(result.copied_count, 1);
    assert_eq!(result.copied_pad_ids.len(), 1);
    assert!(uuid::Uuid::parse_str(&result.copied_pad_ids[0]).is_ok());
    assert!(result.diagnostics.is_empty());
}

// =============================================================================
// Initialization and maintenance outcomes
// =============================================================================

#[test]
fn initialization_maps_scope_and_store_path() {
    let fx = Fixture::new();
    let ctx = support::ctx_with_state(fx.app_state_for(&["init"]));

    let result: InitializationResult = rendered(handlers::init(&ctx, None, false));

    assert_eq!(
        result,
        InitializationResult::Initialized {
            scope: "project".to_string(),
            store_path: fx.project().join(".padz").display().to_string(),
        }
    );
}

#[test]
fn link_and_unlink_map_typed_actions_and_resolved_target() {
    let fx = Fixture::new();
    let target = fx.root().join("target");
    padzapp::init::create_bucket_layout(&target.join(".padz")).unwrap();
    let ctx = fx.ctx();

    let linked: InitializationResult = rendered(handlers::init(
        &ctx,
        Some(target.display().to_string()),
        false,
    ));
    assert_eq!(
        linked,
        InitializationResult::Linked {
            target: target
                .join(".padz")
                .canonicalize()
                .unwrap()
                .display()
                .to_string(),
        }
    );

    let unlinked: InitializationResult = rendered(handlers::init(&ctx, None, true));
    assert_eq!(unlinked, InitializationResult::Unlinked);
}

#[test]
fn doctor_maps_a_healthy_store_to_a_clean_result() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "note", "");
    let ctx = support::ctx_with_state(state);

    let result: DoctorResult = rendered(handlers::doctor(&ctx));

    assert_eq!(
        result,
        DoctorResult::Clean {
            missing_files: 0,
            recovered_files: 0,
        }
    );
}

#[test]
fn purge_maps_selected_pads_and_counts() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "gone", "");
    state
        .with_api(|api| api.delete_pads(state.scope, &["1"]))
        .unwrap();
    let ctx = support::ctx_with_state(state);

    let result: PurgeResult = rendered(handlers::purge(&ctx, vec![], true, false));

    let PurgeResult::Purged {
        selected_pads,
        total_purged,
        descendant_count,
    } = result
    else {
        panic!("expected a completed purge");
    };
    assert_eq!(selected_pads.len(), 1);
    assert_eq!(selected_pads[0].selector, "d1");
    assert_eq!(selected_pads[0].title, "gone");
    assert_eq!(total_purged, 1);
    assert_eq!(descendant_count, 0);
}

#[test]
fn purge_maps_a_nested_selection_with_its_complete_path() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "parent", "");
    fx.seed_child(&state, "1", "child", "");
    let ctx = support::ctx_with_state(state);

    let result: PurgeResult = rendered(handlers::purge(&ctx, vec!["1.1".to_string()], true, false));

    let PurgeResult::Purged { selected_pads, .. } = result else {
        panic!("expected a completed purge");
    };
    assert_eq!(selected_pads.len(), 1);
    assert_eq!(selected_pads[0].selector, "1.1");
    assert_eq!(selected_pads[0].title, "child");
}

// =============================================================================
// Input-chain consumers — create / edit
// =============================================================================
//
// The chain's *precedence* (args beat stdin, an empty pipe aborts) is proven at
// the harness seam against real piped stdin. What these test is the other half:
// given an already-resolved decision, does the handler do the right thing?

#[test]
fn create_with_direct_content_splits_title_from_body() {
    let fx = Fixture::new();
    let ctx = support::ctx_with_input(
        fx.app_state_for(&["create"]),
        CREATE_CONTENT,
        RequestContent::Direct("the title\nthe body".to_string()),
    );

    let result = created(handlers::create(&ctx, None, None, vec![]));

    assert_eq!(result.action, ModificationActionResult::Create);
    assert_eq!(result.pads[0].pad.metadata.title, "the title");
    assert!(result.pads[0].pad.content.contains("the body"));
}

#[test]
fn create_with_an_empty_pipe_aborts_without_creating_a_pad() {
    let fx = Fixture::new();
    let state = fx.app_state_for(&["create"]);
    let ctx = support::ctx_with_input(state, CREATE_CONTENT, RequestContent::PipedEmpty);

    let result = rendered(handlers::create(&ctx, None, None, vec![]));

    let CreateResult::Aborted(abort) = result else {
        panic!("expected a semantic create abort")
    };
    assert_eq!(abort.kind, CreateAbortKindResult::Aborted);
    assert_eq!(abort.reason, CreateAbortReasonResult::EmptyContent);

    let listed = rendered(handlers::list(
        &ctx,
        vec![],
        None,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        vec![],
        false,
        false,
    ));
    assert!(listed.pads.is_empty());
}

#[test]
fn create_piped_content_takes_its_title_from_the_first_line() {
    let fx = Fixture::new();
    let ctx = support::ctx_with_input(
        fx.app_state_for(&["create"]),
        CREATE_CONTENT,
        RequestContent::Piped("piped title\npiped body".to_string()),
    );

    let result = created(handlers::create(&ctx, None, None, vec![]));

    assert_eq!(result.pads[0].pad.metadata.title, "piped title");
}

#[test]
fn create_prefers_the_title_argument_over_the_piped_title() {
    let fx = Fixture::new();
    let ctx = support::ctx_with_input(
        fx.app_state_for(&["create"]),
        CREATE_CONTENT,
        RequestContent::Piped("piped title\npiped body".to_string()),
    );

    let result = created(handlers::create(
        &ctx,
        None,
        None,
        vec!["argument".to_string(), "title".to_string()],
    ));

    assert_eq!(
        result.pads[0].pad.metadata.title, "argument title",
        "an explicit title argument overrides the one parsed from the pipe"
    );
}

#[test]
fn edit_without_a_selector_is_an_error() {
    let fx = Fixture::new();
    let ctx = support::ctx_with_input(
        fx.app_state(),
        EDIT_CONTENT,
        RequestContent::Direct("new text".to_string()),
    );

    let err = handlers::edit(&ctx, vec![]).expect_err("edit needs to know which pad to change");

    assert!(err.to_string().contains("No pad index"), "got: {err}");
}

#[test]
fn edit_with_direct_content_replaces_the_selected_pads_text() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "before", "old body");
    let ctx = support::ctx_with_input(
        state,
        EDIT_CONTENT,
        RequestContent::Direct("after\nnew body".to_string()),
    );

    let result = rendered(handlers::edit(&ctx, vec!["1".to_string()]));

    assert_eq!(result.pads[0].pad.metadata.title, "after");
    assert!(result.pads[0].pad.content.contains("new body"));
    assert_eq!(
        result.outcomes,
        vec![MutationOutcomeResult::Updated {
            path: vec![padzapp::index::DisplayIndex::Regular(1)],
            title: "after".to_string(),
            update_kind: UpdateKindResult::Content,
        }]
    );
}

#[test]
fn edit_maps_a_nested_canonical_path_without_parsing_prose() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "parent", "");
    fx.seed_child(&state, "1", "child", "");
    let ctx = support::ctx_with_input(
        state,
        EDIT_CONTENT,
        RequestContent::Direct("edited child".to_string()),
    );

    let result = rendered(handlers::edit(&ctx, vec!["1.1".to_string()]));

    assert_eq!(
        result.outcomes,
        vec![MutationOutcomeResult::Updated {
            path: vec![
                padzapp::index::DisplayIndex::Regular(1),
                padzapp::index::DisplayIndex::Regular(1),
            ],
            title: "edited child".to_string(),
            update_kind: UpdateKindResult::Content,
        }]
    );
}
