use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::DisplayIndex;
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;

use super::helpers::{indexed_pads, pads_by_indexes};

pub fn run<S: DataStore>(store: &S, scope: Scope, indexes: &[DisplayIndex]) -> Result<CmdResult> {
    // 1. Resolve pads
    let pads = if indexes.is_empty() {
        // All non-deleted pads? Or all? "same as purge, if id(s) are passed, acts on these, else on all"
        // Purge on all acted on *deleted* pads.
        // Export on all likely means *active* pads (or all including deleted?).
        // Usually export implies backup, so maybe all active?
        // Let's assume ALL pads (active) by default.
        // Or if I want backup, maybe active + deleted?
        // "same as purge" refers to argument handling.
        // If I want to export deleted ones, I probably need to specify them or maybe it exports all?
        // Let's export *active* pads by default if no args.
        // If explicit args, can be anything.
        // Check `list` default. `list` is active only.
        indexed_pads(store, scope)?
            .into_iter()
            .filter(|dp| !matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect()
    } else {
        pads_by_indexes(store, scope, indexes)?
    };

    if pads.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to export."));
        return Ok(res);
    }

    // 2. Prepare output file
    let now = Utc::now();
    let filename = format!("padz-{}.tar.gz", now.format("%Y-%m-%d_%H:%M:%S"));
    let file = File::create(&filename).map_err(PadzError::Io)?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = tar::Builder::new(enc);

    // 3. Add files
    for dp in pads {
        let title = &dp.pad.metadata.title;
        let safe_title = sanitize_filename(title);
        // Ensure uniqueness?
        // Simple strategy: title.txt. If multiple pads have same title, last wins or overwrite?
        // To avoid collision, let's append ID suffix? or just hope?
        // "stand alone file" -> title is nice.
        // But Padz allows duplicate titles.
        // Let's use `Title (ID-short).txt` to be safe/unique?
        let entry_name = format!(
            "padz/{}-{}.txt",
            safe_title,
            &dp.pad.metadata.id.to_string()[..8]
        );

        let content = format!("{}\n\n{}", title, dp.pad.content);

        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        tar.append_data(&mut header, entry_name, content.as_bytes())
            .map_err(PadzError::Io)?;
    }

    tar.finish().map_err(PadzError::Io)?;

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!("Exported to {}", filename)));
    Ok(result)
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}
