use colored::Colorize;
use padz::api::{CmdMessage, MessageLevel};

pub(super) fn print_messages(messages: &[CmdMessage]) {
    for message in messages {
        match message.level {
            MessageLevel::Info => println!("{}", message.content.dimmed()),
            MessageLevel::Success => println!("{}", message.content.green()),
            MessageLevel::Warning => println!("{}", message.content.yellow()),
            MessageLevel::Error => println!("{}", message.content.red()),
        }
    }
}
