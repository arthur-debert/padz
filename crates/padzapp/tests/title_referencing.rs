use padzapp::api::PadzApi;
use padzapp::commands::PadzPaths;
use padzapp::model::Scope;
use padzapp::store::memory::InMemoryStore;

fn setup() -> PadzApi<InMemoryStore> {
    let store = InMemoryStore::new();
    let paths = PadzPaths {
        project: Some(std::path::PathBuf::from(".padz")),
        global: std::path::PathBuf::from(".padz"),
    };
    let mut api = PadzApi::new(store, paths);

    // Create some pads
    api.create_pad(
        Scope::Project,
        "Groceries".to_string(),
        "Milk, Eggs".to_string(),
    )
    .unwrap();
    api.create_pad(
        Scope::Project,
        "Grocery List".to_string(),
        "Bread, Butter".to_string(),
    )
    .unwrap();
    api.create_pad(Scope::Project, "Gold".to_string(), "Au".to_string())
        .unwrap();

    api
}

#[test]
fn test_referencing_by_index() {
    let api = setup();
    // 1 -> Gold (newest/shortest title logic? Default is newest first)
    // Created Order: Groceries, Grocery List, Gold.
    // Indexing: Gold (1), Grocery List (2), Groceries (3).

    let res = api.view_pads(Scope::Project, &["1"]).unwrap();
    assert_eq!(res.listed_pads.len(), 1);
    assert_eq!(res.listed_pads[0].pad.metadata.title, "Gold");

    let res = api.view_pads(Scope::Project, &["3"]).unwrap();
    assert_eq!(res.listed_pads.len(), 1);
    assert_eq!(res.listed_pads[0].pad.metadata.title, "Groceries");
}

#[test]
fn test_referencing_multiple_indexes() {
    let api = setup();
    let res = api.view_pads(Scope::Project, &["1", "2"]).unwrap();
    assert_eq!(res.listed_pads.len(), 2);
    // Gold and Grocery List
}

#[test]
fn test_referencing_by_title_exact() {
    let api = setup();
    let res = api.view_pads(Scope::Project, &["Gold"]).unwrap();
    assert_eq!(res.listed_pads.len(), 1);
    assert_eq!(res.listed_pads[0].pad.metadata.title, "Gold");
}

#[test]
fn test_referencing_by_title_partial() {
    let api = setup();
    // "Gold" is matched by "old"
    let res = api.view_pads(Scope::Project, &["old"]).unwrap();
    assert_eq!(res.listed_pads.len(), 1);
    assert_eq!(res.listed_pads[0].pad.metadata.title, "Gold");
}

#[test]
fn test_referencing_by_title_multi_word_arg() {
    let api = setup();
    // "Grocery List" matched by "Grocery List" (passed as separate args by shell simulation)
    // Actually view_pads takes &[String]. The CLI passes ["Grocery", "List"].
    let res = api.view_pads(Scope::Project, &["Grocery", "List"]).unwrap();
    assert_eq!(res.listed_pads.len(), 1);
    assert_eq!(res.listed_pads[0].pad.metadata.title, "Grocery List");
}

#[test]
fn test_referencing_ambiguous() {
    let api = setup();
    // "Gro" matches "Groceries" and "Grocery List"
    let res = api.view_pads(Scope::Project, &["Gro"]);
    assert!(res.is_err());
    let err = res.err().unwrap().to_string();
    assert!(err.contains("matches multiple paths"));
    assert!(err.contains("matched 2 pads"));
}

#[test]
fn test_referencing_mixed_treated_as_title() {
    let api = setup();
    // "1" and "Gold". "1" is index, "Gold" is title.
    // Should be treated as title search "1 Gold".
    // "Gold" pad content is "Au". Title "Gold".
    // Search "1 Gold" -> No match.

    let res = api.view_pads(Scope::Project, &["1", "Gold"]);
    assert!(res.is_err());
    let err = res.err().unwrap().to_string();
    assert!(err.contains("No pad found matching \"1 Gold\""));
}

#[test]
fn test_referencing_mixed_no_match() {
    let api = setup();
    let res = api.view_pads(Scope::Project, &["1", "Grocery"]);
    assert!(res.is_err());
}
