use super::render::render_messages;
use padz::api::CmdMessage;

pub(super) fn print_messages(messages: &[CmdMessage]) {
    let output = render_messages(messages);
    if !output.is_empty() {
        print!("{}", output);
    }
}
