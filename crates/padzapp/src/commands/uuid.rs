//! Resolve pad selectors to their durable UUIDs.
//!
//! Selector parsing and canonical display ordering remain application concerns;
//! presentation clients decide how to render or serialize the resolved values.

use crate::error::Result;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;
use uuid::Uuid;

use super::helpers::{resolve_selectors, TitleBucket};

/// Return the selected UUIDs in selector order, expanding ranges in canonical
/// display order.
pub fn run<S: DataStore>(store: &S, scope: Scope, selectors: &[PadSelector]) -> Result<Vec<Uuid>> {
    let resolved = resolve_selectors(store, scope, selectors, false, TitleBucket::Active)?;
    Ok(resolved.into_iter().map(|(_, uuid)| uuid).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn test_uuid_single_pad() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let created =
            create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        let expected_uuid = created.affected_pads[0].pad.metadata.id;

        let result = run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        assert_eq!(result, vec![expected_uuid]);
    }

    #[test]
    fn test_uuid_multiple_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let first = create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None)
            .unwrap()
            .affected_pads[0]
            .pad
            .metadata
            .id;
        let second = create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None)
            .unwrap()
            .affected_pads[0]
            .pad
            .metadata
            .id;

        let result = run(
            &store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
            ],
        )
        .unwrap();

        assert_eq!(result, vec![first, second]);
    }

    #[test]
    fn test_uuid_range() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let first = create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None)
            .unwrap()
            .affected_pads[0]
            .pad
            .metadata
            .id;
        let second = create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None)
            .unwrap()
            .affected_pads[0]
            .pad
            .metadata
            .id;
        let third = create::run(&mut store, Scope::Project, "Pad C".into(), "".into(), None)
            .unwrap()
            .affected_pads[0]
            .pad
            .metadata
            .id;

        let result = run(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(3)],
            )],
        )
        .unwrap();

        assert_eq!(result, vec![third, second, first]);
    }
}
