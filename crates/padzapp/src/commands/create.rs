use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::model::{Pad, Scope};
use crate::store::DataStore;

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    title: String,
    content: String,
    parent_selector: Option<crate::index::PadSelector>,
) -> Result<CmdResult> {
    let mut pad = Pad::new(title, content);

    if let Some(selector) = parent_selector {
        // Resolve parent
        let resolved = super::helpers::resolve_selectors(store, scope, &[selector], false)?;

        match resolved.len() {
            0 => {
                return Err(crate::error::PadzError::Api(
                    "Parent pad not found".to_string(),
                ))
            }
            1 => {
                let (_, parent_id) = resolved[0];
                pad.metadata.parent_id = Some(parent_id);
            }
            _ => {
                return Err(crate::error::PadzError::Api(
                    "Parent selector is ambiguous/multiple matches".to_string(),
                ))
            }
        }
    }

    store.save_pad(&pad, scope)?;

    let mut result = CmdResult::default();
    result.affected_pads.push(pad.clone());
    result.add_message(CmdMessage::success(format!(
        "Pad created: {}",
        pad.metadata.title
    )));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{DisplayIndex, PadSelector};
    use crate::store::memory::InMemoryStore;

    #[test]
    fn creates_nested_pad() {
        let mut store = InMemoryStore::new();
        // Create parent
        run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();

        // Create child
        let parent_sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(parent_sel),
        )
        .unwrap();

        // Check relationship
        let pads = store.list_pads(Scope::Project).unwrap();
        let child = pads.iter().find(|p| p.metadata.title == "Child").unwrap();
        let parent = pads.iter().find(|p| p.metadata.title == "Parent").unwrap();

        assert_eq!(child.metadata.parent_id, Some(parent.metadata.id));
    }
}
