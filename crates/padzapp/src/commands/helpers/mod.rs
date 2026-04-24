use crate::index::DisplayIndex;

mod indexing;
mod nesting;
mod pad_fetching;
mod selector_resolve;
mod tree_search;

pub use indexing::{bucket_for_index, indexed_pads};
pub use nesting::{collect_nested_pads, NestedPad};
pub use pad_fetching::pads_by_selectors;
pub use selector_resolve::resolve_selectors;
pub use tree_search::{find_pad_by_uuid, get_descendant_ids};

pub fn fmt_path(path: &[DisplayIndex]) -> String {
    let s: Vec<String> = path.iter().map(|idx| idx.to_string()).collect();
    s.join(".")
}
