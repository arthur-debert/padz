//! Clone and migrate commands — move pads between padz stores.
//!
//! Both commands share a pipeline:
//! 1. Resolve selectors against the source store to UUIDs.
//! 2. Open a destination [`FileStore`] at the target path (smart-resolved to
//!    `.padz/` via [`resolve_target_dir`]).
//! 3. For each pad, read from source, apply metadata defensively to a fresh
//!    `Pad` on the destination side, save. `parent_id`s that point outside
//!    the move set are orphaned.
//! 4. Merge referenced tag registry entries into the destination (no
//!    overwriting existing entries).
//! 5. For migrate: delete each successfully copied pad from the source.
//!
//! The "content copy" is treated as the critical path: if it fails, we
//! report the pad as failed and skip it. Metadata field failures are
//! per-pad warnings — the file still lands.
//!
//! ## Path resolution
//!
//! A `--to/--from <path>` argument is resolved to a `.padz/` directory with
//! the same logic the CLI uses for reads:
//! - If `<path>` itself is a `.padz/` dir, use it
//! - Else if `<path>/.padz/` exists, use that
//! - Else walk up from `<path>` looking for `.padz/`
//! - Else error: the target is not a padz store
//!
//! This means `padz clone --to /tmp/work` works whether the user points at
//! `/tmp/work`, `/tmp/work/.padz`, or a subdirectory of the project.

use crate::commands::{CmdMessage, CmdResult};
use crate::config::PadzConfig;
use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, PadSelector};
use crate::init::{find_padz_root, resolve_link};
use crate::model::{Pad, Scope};
use crate::store::fs::FileStore;
use crate::store::{Bucket, DataStore};
use crate::tags::TagEntry;
use clapfig::{Clapfig, SearchMode, SearchPath};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::helpers::{indexed_pads, resolve_selectors};

/// Whether the source keeps or loses the pads after a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    /// Clone: copy to destination, keep in source.
    Clone,
    /// Migrate: copy to destination, delete from source on success.
    Migrate,
}

impl TransferMode {
    fn verb(self) -> &'static str {
        match self {
            TransferMode::Clone => "Cloned",
            TransferMode::Migrate => "Migrated",
        }
    }
}

/// Resolve a user-supplied path into the `.padz/` directory at or above it.
///
/// Accepts three shapes:
/// 1. `<path>` is itself a `.padz` directory → use it
/// 2. `<path>/.padz` exists → use that
/// 3. Walk upward from `<path>` for `.padz`
///
/// If the resolved `.padz/` contains a `link` file, follow it through
/// `init::resolve_link` — matches the CLI's read-side discovery so a
/// linked store is treated the same whether accessed via `clone`/`migrate`
/// or the normal command pipeline.
pub fn resolve_target_dir(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .map_err(|e| PadzError::Api(format!("Target path '{}': {}", path.display(), e)))?;

    let padz_dir = if path.file_name().is_some_and(|n| n == ".padz") && path.is_dir() {
        path
    } else if path.join(".padz").is_dir() {
        path.join(".padz")
    } else if let Some(root) = find_padz_root(&path) {
        root.join(".padz")
    } else {
        return Err(PadzError::Api(format!(
            "No padz store found at or above '{}'",
            path.display()
        )));
    };

    // Follow .padz/link — writes to the wrong store are silently destructive,
    // so propagate link errors instead of swallowing them.
    match resolve_link(&padz_dir) {
        Ok(Some(linked)) => Ok(linked),
        Ok(None) => Ok(padz_dir),
        Err(e) => Err(e),
    }
}

/// Open a read-write FileStore pointing at a `.padz/` directory.
///
/// The scope is always `Scope::Project` (a `.padz/` dir is project-scoped
/// by definition). The store's default format is loaded from its
/// `padz.toml` if present so newly-written pads match the destination's
/// configured format. Individual pads already on disk keep their own
/// extensions (FileStore scans for matching files regardless).
pub fn open_target_store(padz_dir: &Path) -> Result<FileStore> {
    if !padz_dir.exists() {
        return Err(PadzError::Api(format!(
            "Target '{}' does not exist",
            padz_dir.display()
        )));
    }

    if !padz_dir.is_dir() {
        return Err(PadzError::Api(format!(
            "Target '{}' is not a directory",
            padz_dir.display()
        )));
    }

    let active_dir = padz_dir.join("active");
    if !active_dir.is_dir() {
        return Err(PadzError::Api(format!(
            "Target '{}' is not an initialized padz store (missing '{}'). Run `padz init` there first.",
            padz_dir.display(),
            active_dir.display()
        )));
    }

    // Load the target's config so new files match its configured format.
    // Failure to load is not fatal — fall back to the default `.txt`, same
    // as the main CLI path in `init::initialize`.
    let format_ext = Clapfig::builder::<PadzConfig>()
        .app_name("padz")
        .file_name("padz.toml")
        .search_paths(vec![SearchPath::Path(padz_dir.to_path_buf())])
        .search_mode(SearchMode::Merge)
        .strict(false)
        .load()
        .map(|c| c.format_ext())
        .unwrap_or_else(|_| ".txt".to_string());

    Ok(
        FileStore::new_fs(Some(padz_dir.to_path_buf()), padz_dir.to_path_buf())
            .with_format(&format_ext),
    )
}

/// Run a clone or migrate operation between two `DataStore` instances.
///
/// - `source`: the store we read pads from
/// - `source_scope`: Project or Global on the source side
/// - `dest`: the store we write pads to
/// - `dest_scope`: Project or Global on the dest side
/// - `selectors`: pad selectors, resolved against the source
/// - `summary_path`: human-readable destination location (used in the
///   trailing success message; usually a path, can be anything)
/// - `mode`: Clone (keep source) or Migrate (delete source on success)
pub fn run<Src: DataStore, Dst: DataStore>(
    source: &mut Src,
    source_scope: Scope,
    dest: &mut Dst,
    dest_scope: Scope,
    selectors: &[PadSelector],
    summary_path: &Path,
    mode: TransferMode,
) -> Result<CmdResult> {
    let mut result = CmdResult::default();

    // 1. Resolve selectors on the source. Empty selectors mean "all non-
    //    deleted pads" (active + archived), matching `padz export`'s default
    //    set so the two commands behave consistently.
    let resolved = if selectors.is_empty() {
        default_non_deleted_ids(source, source_scope)?
    } else {
        resolve_selectors(source, source_scope, selectors, false)?
    };
    if resolved.is_empty() {
        result.add_message(CmdMessage::info("No pads to transfer."));
        return Ok(result);
    }

    // 2. Build the move set. Pads whose parent lives outside the move set
    //    AND outside the destination get orphaned to root. An inability to
    //    enumerate the destination surfaces as a warning rather than being
    //    silently swallowed — an incomplete `dest_ids` set can incorrectly
    //    orphan parent relationships.
    let move_set: HashSet<Uuid> = resolved.iter().map(|(_, uuid)| *uuid).collect();
    let mut dest_ids_warnings = Vec::new();
    let dest_ids = collect_all_ids(dest, dest_scope, &mut dest_ids_warnings);
    for w in dest_ids_warnings {
        result.add_message(w);
    }
    let known_ids: HashSet<Uuid> = move_set.union(&dest_ids).copied().collect();

    // 3. Transfer pad-by-pad. `copy_one_pad` returns the source pad's tags so
    //    we can merge the registry without re-reading.
    let mut copied: Vec<Uuid> = Vec::new();
    let mut referenced_tags: HashSet<String> = HashSet::new();

    for (_, id) in &resolved {
        match copy_one_pad(source, source_scope, dest, dest_scope, *id, &known_ids) {
            Ok(CopyOutcome { mut warnings, tags }) => {
                copied.push(*id);
                for t in tags {
                    referenced_tags.insert(t);
                }
                result.messages.append(&mut warnings);
            }
            Err(e) => {
                result.add_message(CmdMessage::warning(format!(
                    "Failed to {} pad {}: {}",
                    match mode {
                        TransferMode::Clone => "clone",
                        TransferMode::Migrate => "migrate",
                    },
                    id,
                    e
                )));
            }
        }
    }

    // 4. Merge the referenced subset of the source's tag registry into dest.
    if !copied.is_empty() {
        if let Err(e) = merge_tag_registry(source, source_scope, dest, dest_scope, &referenced_tags)
        {
            result.add_message(CmdMessage::warning(format!(
                "Tag registry merge failed: {}",
                e
            )));
        }
    }

    // 5. For migrate: delete copies from source.
    if mode == TransferMode::Migrate {
        for id in &copied {
            if let Err(e) = delete_from_source(source, source_scope, *id) {
                result.add_message(CmdMessage::warning(format!(
                    "Copied but failed to remove from source, pad {}: {}",
                    id, e
                )));
            }
        }
    }

    if copied.is_empty() {
        result.add_message(CmdMessage::warning(format!(
            "No pads were {} to {}",
            mode.verb().to_lowercase(),
            summary_path.display()
        )));
    } else {
        result.add_message(CmdMessage::success(format!(
            "{} {} pad(s) to {}",
            mode.verb(),
            copied.len(),
            summary_path.display()
        )));
    }
    Ok(result)
}

fn collect_all_ids<S: DataStore>(
    store: &S,
    scope: Scope,
    warnings: &mut Vec<CmdMessage>,
) -> HashSet<Uuid> {
    let mut ids = HashSet::new();
    for bucket in [Bucket::Active, Bucket::Archived, Bucket::Deleted] {
        match store.list_pads(scope, bucket) {
            Ok(pads) => {
                for p in pads {
                    ids.insert(p.metadata.id);
                }
            }
            Err(e) => {
                warnings.push(CmdMessage::warning(format!(
                    "Could not enumerate destination bucket {:?}: {}. \
Parent relationships that cross this bucket may be orphaned.",
                    bucket, e
                )));
            }
        }
    }
    ids
}

/// Default selection when the user passes no indexes: active + archived
/// pads (matches `padz export`'s default set).
fn default_non_deleted_ids<S: DataStore>(
    store: &S,
    scope: Scope,
) -> Result<Vec<(Vec<DisplayIndex>, Uuid)>> {
    let pads = indexed_pads(store, scope)?;
    let mut out = Vec::new();
    let mut seen: HashSet<Uuid> = HashSet::new();
    for dp in pads {
        if matches!(dp.index, DisplayIndex::Deleted(_)) {
            continue;
        }
        if !seen.insert(dp.pad.metadata.id) {
            continue;
        }
        out.push((vec![dp.index.clone()], dp.pad.metadata.id));
    }
    Ok(out)
}

/// Result of a successful per-pad copy: carries warnings from defensive
/// metadata application plus the source pad's tags (so the caller can
/// merge the tag registry without another source read).
struct CopyOutcome {
    warnings: Vec<CmdMessage>,
    tags: Vec<String>,
}

/// Read a pad from whichever bucket it lives in on the source side. Returns
/// the pad + its original bucket so we can restore on the destination.
fn read_source_pad_any_bucket<S: DataStore>(
    source: &S,
    scope: Scope,
    id: Uuid,
) -> Result<(Pad, Bucket)> {
    for bucket in [Bucket::Active, Bucket::Archived, Bucket::Deleted] {
        if let Ok(pad) = source.get_pad(&id, scope, bucket) {
            return Ok((pad, bucket));
        }
    }
    Err(PadzError::Api(format!(
        "Pad {} not found in any bucket",
        id
    )))
}

/// Copy a single pad from source to dest.
///
/// Live store-to-store transfer: the source hands us a valid [`Pad`] with
/// valid [`Metadata`], so we just forward it. The only policy here is
/// parent-orphan: if the pad's `parent_id` points outside the known set
/// (the pads being moved + those already at the destination), we drop the
/// link so the destination never has a dangling reference.
///
/// Writing to the destination is the critical path; failure surfaces as
/// `Err` and the caller reports the pad as failed.
///
/// Cross-version defensive parsing (field-by-field tolerance) is reserved
/// for reading *archives* on disk — see
/// [`crate::model::Metadata::apply_json_patch`] — not needed for live
/// same-version transfers.
fn copy_one_pad<Src: DataStore, Dst: DataStore>(
    source: &Src,
    source_scope: Scope,
    dest: &mut Dst,
    dest_scope: Scope,
    id: Uuid,
    known_ids: &HashSet<Uuid>,
) -> Result<CopyOutcome> {
    let (mut pad, bucket) = read_source_pad_any_bucket(source, source_scope, id)?;
    let tags = pad.metadata.tags.clone();

    let mut warnings = Vec::new();
    if let Some(pid) = pad.metadata.parent_id {
        if !known_ids.contains(&pid) {
            pad.metadata.parent_id = None;
            warnings.push(CmdMessage::info(format!(
                "Pad {}: parent not in move set, orphaned to root",
                id
            )));
        }
    }

    dest.save_pad(&pad, dest_scope, bucket)
        .map_err(|e| PadzError::Api(format!("Writing pad {} to destination failed: {}", id, e)))?;

    Ok(CopyOutcome { warnings, tags })
}

fn delete_from_source<S: DataStore>(source: &mut S, scope: Scope, id: Uuid) -> Result<()> {
    // Find the pad's bucket on the source side again (it may live in any of
    // active/archived/deleted) and delete from there.
    for bucket in [Bucket::Active, Bucket::Archived, Bucket::Deleted] {
        if source.get_pad(&id, scope, bucket).is_ok() {
            return source.delete_pad(&id, scope, bucket);
        }
    }
    Err(PadzError::Api(format!(
        "Pad {} not found for source delete",
        id
    )))
}

/// Merge the referenced subset of the source's tag registry into dest's
/// registry. Tags already present at dest are not overwritten.
fn merge_tag_registry<Src: DataStore, Dst: DataStore>(
    source: &Src,
    source_scope: Scope,
    dest: &mut Dst,
    dest_scope: Scope,
    referenced: &HashSet<String>,
) -> Result<()> {
    if referenced.is_empty() {
        return Ok(());
    }
    let source_tags = source.load_tags(source_scope).unwrap_or_default();
    let existing: HashMap<String, TagEntry> = dest
        .load_tags(dest_scope)
        .unwrap_or_default()
        .into_iter()
        .map(|t| (t.name.clone(), t))
        .collect();

    let mut merged: Vec<TagEntry> = existing.values().cloned().collect();
    let mut added = 0usize;
    for t in source_tags {
        if referenced.contains(&t.name) && !existing.contains_key(&t.name) {
            merged.push(t);
            added += 1;
        }
    }
    if added > 0 {
        dest.save_tags(dest_scope, &merged)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::index::{DisplayIndex, PadSelector};
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    fn store() -> BucketedStore<MemBackend> {
        BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        )
    }

    /// For unit tests we need a destination store that is `&mut dyn DataStore`.
    /// The production `run` takes a `FileStore` directly, so test the inner
    /// pipeline (copy_one_pad + delete_from_source) using two in-memory stores.
    fn test_copy_one_pad<D: DataStore>(
        src: &BucketedStore<MemBackend>,
        scope: Scope,
        dst: &mut D,
        id: Uuid,
        known: &HashSet<Uuid>,
    ) -> Result<()> {
        copy_one_pad(src, scope, dst, Scope::Project, id, known)?;
        Ok(())
    }

    #[test]
    fn test_copy_preserves_uuid_and_metadata() {
        let mut src = store();
        create::run(
            &mut src,
            Scope::Project,
            "Title".into(),
            "Body".into(),
            None,
        )
        .unwrap();
        let src_pad = src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let id = src_pad.metadata.id;

        let mut dst = store();
        let known: HashSet<Uuid> = [id].into_iter().collect();
        test_copy_one_pad(&src, Scope::Project, &mut dst, id, &known).unwrap();

        let dst_pad = dst
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(dst_pad.metadata.id, id);
        assert_eq!(dst_pad.metadata.title, "Title");
    }

    #[test]
    fn test_copy_orphans_parent_outside_move_set() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "Parent".into(), "P".into(), None).unwrap();
        create::run(
            &mut src,
            Scope::Project,
            "Child".into(),
            "C".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let pads = src.list_pads(Scope::Project, Bucket::Active).unwrap();
        let child = pads.iter().find(|p| p.metadata.title == "Child").unwrap();

        // Move only the child; parent stays behind
        let known: HashSet<Uuid> = [child.metadata.id].into_iter().collect();

        let mut dst = store();
        test_copy_one_pad(&src, Scope::Project, &mut dst, child.metadata.id, &known).unwrap();

        let dst_pads = dst.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(dst_pads.len(), 1);
        assert_eq!(
            dst_pads[0].metadata.parent_id, None,
            "child should be orphaned when parent is not in move set"
        );
    }

    #[test]
    fn test_copy_preserves_parent_inside_move_set() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "Parent".into(), "P".into(), None).unwrap();
        create::run(
            &mut src,
            Scope::Project,
            "Child".into(),
            "C".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let pads = src.list_pads(Scope::Project, Bucket::Active).unwrap();
        let parent = pads.iter().find(|p| p.metadata.title == "Parent").unwrap();
        let child = pads.iter().find(|p| p.metadata.title == "Child").unwrap();

        let known: HashSet<Uuid> = [parent.metadata.id, child.metadata.id]
            .into_iter()
            .collect();

        let mut dst = store();
        test_copy_one_pad(&src, Scope::Project, &mut dst, parent.metadata.id, &known).unwrap();
        test_copy_one_pad(&src, Scope::Project, &mut dst, child.metadata.id, &known).unwrap();

        let dst_pads = dst.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(dst_pads.len(), 2);
        let dst_child = dst_pads
            .iter()
            .find(|p| p.metadata.title == "Child")
            .unwrap();
        assert_eq!(dst_child.metadata.parent_id, Some(parent.metadata.id));
    }

    #[test]
    fn test_migrate_removes_source_after_copy() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "Alpha".into(), "".into(), None).unwrap();
        let id = src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .metadata
            .id;

        let mut dst = store();
        let known: HashSet<Uuid> = [id].into_iter().collect();
        test_copy_one_pad(&src, Scope::Project, &mut dst, id, &known).unwrap();

        // Simulate the migrate deletion step
        delete_from_source(&mut src, Scope::Project, id).unwrap();

        assert!(
            src.list_pads(Scope::Project, Bucket::Active)
                .unwrap()
                .is_empty(),
            "source should be empty after migrate"
        );
        assert_eq!(
            dst.list_pads(Scope::Project, Bucket::Active).unwrap().len(),
            1,
            "destination should have the pad"
        );
    }
}
