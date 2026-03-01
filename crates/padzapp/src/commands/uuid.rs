use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;

use super::helpers::resolve_selectors;

pub fn run<S: DataStore>(store: &S, scope: Scope, selectors: &[PadSelector]) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, false)?;
    let mut result = CmdResult::default();

    for (_, uuid) in resolved {
        result
            .messages
            .push(super::CmdMessage::info(uuid.to_string()));
    }

    Ok(result)
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

        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].content, expected_uuid.to_string());
    }

    #[test]
    fn test_uuid_multiple_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None).unwrap();

        let result = run(
            &store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
            ],
        )
        .unwrap();

        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn test_uuid_range() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad C".into(), "".into(), None).unwrap();

        let result = run(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(3)],
            )],
        )
        .unwrap();

        assert_eq!(result.messages.len(), 3);
    }
}
