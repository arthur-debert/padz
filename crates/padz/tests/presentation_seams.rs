//! Closing proof for the reusable-result/presentation seam.
//!
//! The core and typed-handler interfaces expose semantic facts. Standout's
//! template path owns the human sentence. This file keeps that cross-seam proof
//! isolated from the broad format matrix migrated by STD03-WS09.

mod support;

use padz::cli::handlers;
use padz::cli::result::{Modification, ModificationAction};
use standout::cli::Output;
use standout_test::{serial, TestHarness};
use support::Fixture;

fn rendered(out: Result<Output<Modification>, anyhow::Error>) -> Modification {
    match out.expect("handler returned an error") {
        Output::Render(value) => value,
        other => panic!("expected rendered modification result, got {other:?}"),
    }
}

#[test]
fn typed_pin_handler_exposes_a_semantic_action_without_generic_messages() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    let ctx = support::ctx_with_state(state);

    let result = rendered(handlers::pin(&ctx, vec!["1".to_string()]));

    assert_eq!(result.action, ModificationAction::Pin);
    assert_eq!(result.pads.len(), 1);
    assert!(result.notices.is_empty());
    assert!(result.outcomes.is_empty());

    let value = serde_json::to_value(result).unwrap();
    let mut top_level_keys: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    top_level_keys.sort_unstable();
    let actual = serde_json::json!({
        "action": value["action"],
        "top_level_keys": top_level_keys,
    });
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/presentation_seams/modification-pin.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
#[serial]
fn pin_template_owns_the_compatible_human_sentence() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    drop(state);
    let (app, command) = fx.read_app();

    let result = TestHarness::new().no_color().run(
        &app,
        command,
        fx.argv(&["pin", "1", "--output", "text"]),
    );

    result.assert_success();
    result.assert_stdout_contains("Pinned 1 pad...");
    result.assert_stdout_contains("p1. target");
}
