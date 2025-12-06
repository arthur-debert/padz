use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::DisplayIndex;
use crate::index::DisplayPad;
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::Write;

use super::helpers::{indexed_pads, pads_by_indexes};

pub fn run<S: DataStore>(store: &S, scope: Scope, indexes: &[DisplayIndex]) -> Result<CmdResult> {
    // 1. Resolve pads
    let pads = resolve_pads(store, scope, indexes)?;

    if pads.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to export."));
        return Ok(res);
    }

    // 2. Prepare output file
    let now = Utc::now();
    let filename = format!("padz-{}.tar.gz", now.format("%Y-%m-%d_%H:%M:%S"));
    let file = File::create(&filename).map_err(PadzError::Io)?;

    // 3. Write archive
    write_archive(file, &pads)?;

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!("Exported to {}", filename)));
    Ok(result)
}

fn resolve_pads<S: DataStore>(
    store: &S,
    scope: Scope,
    indexes: &[DisplayIndex],
) -> Result<Vec<DisplayPad>> {
    if indexes.is_empty() {
        Ok(indexed_pads(store, scope)?
            .into_iter()
            .filter(|dp| !matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect())
    } else {
        pads_by_indexes(store, scope, indexes)
    }
}

fn write_archive<W: Write>(writer: W, pads: &[DisplayPad]) -> Result<()> {
    let enc = GzEncoder::new(writer, Compression::default());
    let mut tar = tar::Builder::new(enc);

    for dp in pads {
        let title = &dp.pad.metadata.title;
        let safe_title = sanitize_filename(title);
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
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn test_resolve_pads_exports_active_by_default() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Active".into(), "".into()).unwrap();

        let mut del_pad = crate::model::Pad::new("Deleted".into(), "".into());
        del_pad.metadata.is_deleted = true;
        store.save_pad(&del_pad, Scope::Project).unwrap();

        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].pad.metadata.title, "Active");
    }

    #[test]
    fn test_write_archive_produces_content() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Test".into(), "Content".into()).unwrap();
        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();

        let mut buf = Vec::new();
        write_archive(&mut buf, &pads).unwrap();

        assert!(!buf.is_empty());
        // Could verify tar content but that requires untarring.
        // Checking header magic? Gzip header is 1f 8b
        assert_eq!(buf[0], 0x1f);
        assert_eq!(buf[1], 0x8b);
    }

    #[test]
    fn test_sanitize() {
        assert_eq!(sanitize_filename("Hello World"), "Hello World");
        assert_eq!(sanitize_filename("foo/bar"), "foo_bar");
        assert_eq!(sanitize_filename("baz\\qux"), "baz_qux");
    }
}
