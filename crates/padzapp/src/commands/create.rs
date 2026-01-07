use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
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
    result.affected_pads.push(pad.clone());
    result.add_message(CmdMessage::success(format!(
        "Pad created: {}",
        pad.metadata.title
    )));
    Ok(result)
}
