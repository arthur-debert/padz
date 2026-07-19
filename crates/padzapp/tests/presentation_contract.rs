//! Core-level proof that command results expose facts, not presentation prose.

use padzapp::commands::{create, pinning, CmdNotice};
use padzapp::index::{DisplayIndex, PadSelector};
use padzapp::model::Scope;
use padzapp::store::bucketed::BucketedStore;
use padzapp::store::mem_backend::MemBackend;

fn store() -> BucketedStore<MemBackend> {
    BucketedStore::new(
        MemBackend::new(),
        MemBackend::new(),
        MemBackend::new(),
        MemBackend::new(),
    )
}

#[test]
fn repeated_pin_exposes_the_no_op_as_typed_facts() {
    let mut store = store();
    create::run(&mut store, Scope::Project, "target".into(), "".into(), None).unwrap();
    let regular = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
    pinning::pin(&mut store, Scope::Project, &[regular]).unwrap();

    let pinned = PadSelector::Path(vec![DisplayIndex::Pinned(1)]);
    let result = pinning::pin(&mut store, Scope::Project, &[pinned]).unwrap();

    assert_eq!(
        result.notices,
        vec![CmdNotice::AlreadyPinned {
            path: vec![DisplayIndex::Pinned(1)],
        }]
    );
}

#[test]
fn reusable_source_has_no_generic_message_interface() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![source_root];
    let mut offenders = Vec::new();

    while let Some(path) = stack.pop() {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                for forbidden in ["CmdMessage", "MessageLevel", "trailing_messages"] {
                    if source.contains(forbidden) {
                        offenders.push(format!("{} contains {forbidden}", path.display()));
                    }
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "generic presentation plumbing returned to padzapp: {offenders:#?}"
    );
}
