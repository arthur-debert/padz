use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad};
use crate::model::{Pad, Scope};
use crate::store::DataStore;

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    title: String,
    content: String,
) -> Result<CmdResult> {
    let pad = Pad::new(title, content);
    store.save_pad(&pad, scope)?;

    let mut result = CmdResult::default();
    // New pad is always the newest, so it gets index 1
    let display_pad = DisplayPad {
        pad: pad.clone(),
        index: DisplayIndex::Regular(1),
        matches: None,
    };
    result.affected_pads.push(display_pad);
    result.add_message(CmdMessage::success(format!(
        "Pad created: {}",
        pad.metadata.title
    )));
    Ok(result)
}
