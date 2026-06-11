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

use super::helpers::{indexed_pads, resolve_selectors, TitleBucket};

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
        resolve_selectors(source, source_scope, selectors, false, TitleBucket::Any)?
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
/// valid [`crate::model::Metadata`], so we just forward it. The only policy here is
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
    use crate::commands::{create, MessageLevel};
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

    // ------------------------------------------------------------------------
    // Pipeline tests — `run` end-to-end with two in-memory stores.
    // ------------------------------------------------------------------------

    /// Helper: initialize a `.padz/` layout (`active`, `archived`, `deleted`)
    /// inside `dir` so it looks like an initialized store on disk.
    fn init_layout(dir: &std::path::Path) {
        std::fs::create_dir_all(dir.join("active")).unwrap();
        std::fs::create_dir_all(dir.join("archived")).unwrap();
        std::fs::create_dir_all(dir.join("deleted")).unwrap();
    }

    #[test]
    fn test_transfer_mode_verb() {
        // Direct check on the only entry point that uses verb().
        assert_eq!(TransferMode::Clone.verb(), "Cloned");
        assert_eq!(TransferMode::Migrate.verb(), "Migrated");
    }

    #[test]
    fn test_run_no_pads_returns_info_message() {
        // Empty source + explicit selectors that can't resolve to anything.
        let mut src = store();
        let mut dst = store();
        // An empty source with empty selectors triggers the "no pads" branch
        // through `default_non_deleted_ids` (which returns an empty Vec).
        let summary = std::path::PathBuf::from("/tmp/nowhere");
        let result = run(
            &mut src,
            Scope::Project,
            &mut dst,
            Scope::Project,
            &[],
            &summary,
            TransferMode::Clone,
        )
        .unwrap();

        assert!(
            result
                .messages
                .iter()
                .any(|m| m.content.contains("No pads to transfer")),
            "expected info message about no pads; got {:?}",
            result.messages
        );
    }

    #[test]
    fn test_run_clone_copies_pads_and_keeps_source() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "Alpha".into(), "A".into(), None).unwrap();
        create::run(&mut src, Scope::Project, "Beta".into(), "B".into(), None).unwrap();

        let mut dst = store();
        let summary = std::path::PathBuf::from("/tmp/dest");
        let result = run(
            &mut src,
            Scope::Project,
            &mut dst,
            Scope::Project,
            &[],
            &summary,
            TransferMode::Clone,
        )
        .unwrap();

        let success_message = result
            .messages
            .iter()
            .find(|m| matches!(m.level, MessageLevel::Success))
            .expect("clone should emit a success message");
        assert!(success_message.content.contains("Cloned 2 pad(s)"));
        assert!(success_message.content.contains("/tmp/dest"));

        // Both pads on destination, both still on source.
        assert_eq!(
            dst.list_pads(Scope::Project, Bucket::Active).unwrap().len(),
            2
        );
        assert_eq!(
            src.list_pads(Scope::Project, Bucket::Active).unwrap().len(),
            2
        );
    }

    #[test]
    fn test_run_migrate_deletes_source_after_copy() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "Solo".into(), "".into(), None).unwrap();

        let mut dst = store();
        let summary = std::path::PathBuf::from("/tmp/dest");
        let result = run(
            &mut src,
            Scope::Project,
            &mut dst,
            Scope::Project,
            &[],
            &summary,
            TransferMode::Migrate,
        )
        .unwrap();

        assert!(
            result
                .messages
                .iter()
                .any(|m| m.content.contains("Migrated 1 pad(s)")),
            "expected Migrated success message; got {:?}",
            result.messages
        );

        // Migrate clears the source.
        assert!(src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .is_empty());
        assert_eq!(
            dst.list_pads(Scope::Project, Bucket::Active).unwrap().len(),
            1
        );
    }

    #[test]
    fn test_run_default_selection_skips_deleted() {
        // Active + archived go; deleted stays. Mirrors `padz export`'s default
        // set so the two commands are consistent.
        let mut src = store();
        create::run(&mut src, Scope::Project, "Live".into(), "".into(), None).unwrap();
        create::run(&mut src, Scope::Project, "Archived".into(), "".into(), None).unwrap();
        create::run(&mut src, Scope::Project, "Trashed".into(), "".into(), None).unwrap();

        let pads = src.list_pads(Scope::Project, Bucket::Active).unwrap();
        let archived = pads
            .iter()
            .find(|p| p.metadata.title == "Archived")
            .unwrap();
        let trashed = pads.iter().find(|p| p.metadata.title == "Trashed").unwrap();
        src.move_pad(
            &archived.metadata.id,
            Scope::Project,
            Bucket::Active,
            Bucket::Archived,
        )
        .unwrap();
        src.move_pad(
            &trashed.metadata.id,
            Scope::Project,
            Bucket::Active,
            Bucket::Deleted,
        )
        .unwrap();

        let mut dst = store();
        let summary = std::path::PathBuf::from("/tmp/dest");
        run(
            &mut src,
            Scope::Project,
            &mut dst,
            Scope::Project,
            &[],
            &summary,
            TransferMode::Clone,
        )
        .unwrap();

        let active = dst.list_pads(Scope::Project, Bucket::Active).unwrap();
        let archived_dst = dst.list_pads(Scope::Project, Bucket::Archived).unwrap();
        let deleted_dst = dst.list_pads(Scope::Project, Bucket::Deleted).unwrap();
        assert_eq!(active.len(), 1, "active pad should land on dest");
        assert_eq!(archived_dst.len(), 1, "archived pad should land on dest");
        assert!(
            deleted_dst.is_empty(),
            "deleted pad should NOT be transferred"
        );
    }

    #[test]
    fn test_run_explicit_selector_resolves_against_source() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut src, Scope::Project, "Second".into(), "".into(), None).unwrap();

        let mut dst = store();
        let summary = std::path::PathBuf::from("/tmp/dest");
        // The newest pad gets index 1 (which is "Second"). Asking for "1"
        // should clone exactly one pad: the most recently created.
        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        let result = run(
            &mut src,
            Scope::Project,
            &mut dst,
            Scope::Project,
            &selectors,
            &summary,
            TransferMode::Clone,
        )
        .unwrap();

        assert!(result
            .messages
            .iter()
            .any(|m| m.content.contains("Cloned 1 pad(s)")));
        let dst_pads = dst.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(dst_pads.len(), 1);
        assert_eq!(dst_pads[0].metadata.title, "Second");
    }

    #[test]
    fn test_run_merges_referenced_tags_into_destination() {
        use crate::tags::TagEntry;

        let mut src = store();
        create::run(&mut src, Scope::Project, "Tagged".into(), "".into(), None).unwrap();

        // Stamp a tag on the pad and register it in the source registry.
        let pads = src.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads.into_iter().next().unwrap();
        pad.metadata.tags = vec!["work".into()];
        src.save_pad(&pad, Scope::Project, Bucket::Active).unwrap();
        src.save_tags(
            Scope::Project,
            &[TagEntry::new("work".into()), TagEntry::new("unused".into())],
        )
        .unwrap();

        let mut dst = store();
        let summary = std::path::PathBuf::from("/tmp/dest");
        run(
            &mut src,
            Scope::Project,
            &mut dst,
            Scope::Project,
            &[],
            &summary,
            TransferMode::Clone,
        )
        .unwrap();

        // Only the referenced tag should be merged — `unused` stays behind so we
        // don't pollute the destination's registry with tags the moved pads
        // don't actually use.
        let dst_tags = dst.load_tags(Scope::Project).unwrap();
        let names: std::collections::HashSet<String> =
            dst_tags.iter().map(|t| t.name.clone()).collect();
        assert!(names.contains("work"), "referenced tag should be merged");
        assert!(
            !names.contains("unused"),
            "unreferenced tags must not leak across stores"
        );
    }

    #[test]
    fn test_run_reports_per_pad_failure_on_dest_write_error() {
        // The destination is rigged to fail all writes. The pipeline must
        // still complete, surface a per-pad warning, and end with "No pads
        // were ... " — not crash, not silently succeed.
        let mut src = store();
        create::run(&mut src, Scope::Project, "Alpha".into(), "".into(), None).unwrap();

        // BucketedStore<MemBackend> with all four backends rigged to fail
        // writes. Use a fresh handle so the simulate flag is set before any
        // writes are attempted.
        let dst_active = MemBackend::new();
        dst_active.set_simulate_write_error(true);
        let dst_archived = MemBackend::new();
        dst_archived.set_simulate_write_error(true);
        let dst_deleted = MemBackend::new();
        dst_deleted.set_simulate_write_error(true);
        let dst_tags = MemBackend::new();
        dst_tags.set_simulate_write_error(true);
        let mut dst = BucketedStore::new(dst_active, dst_archived, dst_deleted, dst_tags);

        let summary = std::path::PathBuf::from("/tmp/dest");
        let result = run(
            &mut src,
            Scope::Project,
            &mut dst,
            Scope::Project,
            &[],
            &summary,
            TransferMode::Clone,
        )
        .unwrap();

        // Per-pad warning naming the failed action.
        assert!(
            result.messages.iter().any(|m| {
                matches!(m.level, MessageLevel::Warning)
                    && m.content.contains("Failed to clone pad")
            }),
            "expected per-pad clone failure warning; got {:?}",
            result.messages
        );

        // And the trailing "no pads were cloned" message.
        assert!(
            result
                .messages
                .iter()
                .any(|m| m.content.contains("No pads were cloned")),
            "expected 'No pads were cloned' trailer; got {:?}",
            result.messages
        );
    }

    #[test]
    fn test_run_does_not_overwrite_existing_destination_tag() {
        use crate::tags::TagEntry;

        let mut src = store();
        create::run(&mut src, Scope::Project, "Tagged".into(), "".into(), None).unwrap();
        let mut pad = src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        pad.metadata.tags = vec!["work".into()];
        src.save_pad(&pad, Scope::Project, Bucket::Active).unwrap();
        let src_tag = TagEntry::new("work".into());
        src.save_tags(Scope::Project, std::slice::from_ref(&src_tag))
            .unwrap();

        // Dest already has a "work" tag with a different created_at. The
        // merge must not clobber it.
        let mut dst = store();
        let mut dst_tag = TagEntry::new("work".into());
        dst_tag.created_at = src_tag.created_at - chrono::Duration::days(7);
        dst.save_tags(Scope::Project, &[dst_tag.clone()]).unwrap();

        let summary = std::path::PathBuf::from("/tmp/dest");
        run(
            &mut src,
            Scope::Project,
            &mut dst,
            Scope::Project,
            &[],
            &summary,
            TransferMode::Clone,
        )
        .unwrap();

        let dst_tags = dst.load_tags(Scope::Project).unwrap();
        assert_eq!(dst_tags.len(), 1);
        assert_eq!(
            dst_tags[0].created_at, dst_tag.created_at,
            "existing destination tag must not be overwritten by merge"
        );
    }

    // ------------------------------------------------------------------------
    // Pure helpers
    // ------------------------------------------------------------------------

    #[test]
    fn test_collect_all_ids_spans_all_buckets() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut src, Scope::Project, "B".into(), "".into(), None).unwrap();
        create::run(&mut src, Scope::Project, "C".into(), "".into(), None).unwrap();

        // Push one pad into archived and one into deleted.
        let pads = src.list_pads(Scope::Project, Bucket::Active).unwrap();
        let to_archive = pads
            .iter()
            .find(|p| p.metadata.title == "B")
            .unwrap()
            .metadata
            .id;
        let to_delete = pads
            .iter()
            .find(|p| p.metadata.title == "C")
            .unwrap()
            .metadata
            .id;
        src.move_pad(
            &to_archive,
            Scope::Project,
            Bucket::Active,
            Bucket::Archived,
        )
        .unwrap();
        src.move_pad(&to_delete, Scope::Project, Bucket::Active, Bucket::Deleted)
            .unwrap();

        let mut warnings = Vec::new();
        let ids = collect_all_ids(&src, Scope::Project, &mut warnings);
        assert!(warnings.is_empty(), "no errors expected on in-memory store");
        assert_eq!(ids.len(), 3, "collect_all_ids should span all buckets");
        assert!(ids.contains(&to_archive));
        assert!(ids.contains(&to_delete));
    }

    #[test]
    fn test_default_non_deleted_ids_excludes_deleted_bucket() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "Keep".into(), "".into(), None).unwrap();
        create::run(&mut src, Scope::Project, "Trash".into(), "".into(), None).unwrap();

        let trash = src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .into_iter()
            .find(|p| p.metadata.title == "Trash")
            .unwrap()
            .metadata
            .id;
        src.move_pad(&trash, Scope::Project, Bucket::Active, Bucket::Deleted)
            .unwrap();

        let resolved = default_non_deleted_ids(&src, Scope::Project).unwrap();
        let ids: HashSet<Uuid> = resolved.iter().map(|(_, u)| *u).collect();
        assert_eq!(ids.len(), 1, "deleted pad must not be in default set");
        assert!(!ids.contains(&trash));
    }

    #[test]
    fn test_read_source_pad_any_bucket_finds_archived() {
        let mut src = store();
        create::run(&mut src, Scope::Project, "Archived".into(), "".into(), None).unwrap();
        let id = src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .metadata
            .id;
        src.move_pad(&id, Scope::Project, Bucket::Active, Bucket::Archived)
            .unwrap();

        let (pad, bucket) = read_source_pad_any_bucket(&src, Scope::Project, id).unwrap();
        assert_eq!(pad.metadata.id, id);
        assert_eq!(bucket, Bucket::Archived);
    }

    #[test]
    fn test_read_source_pad_any_bucket_missing_uuid_errors() {
        let src = store();
        let missing = Uuid::new_v4();
        let err = read_source_pad_any_bucket(&src, Scope::Project, missing).unwrap_err();
        assert!(err.to_string().contains("not found in any bucket"));
    }

    #[test]
    fn test_merge_tag_registry_no_referenced_tags_is_noop() {
        let src = store();
        let mut dst = store();
        let empty: HashSet<String> = HashSet::new();
        merge_tag_registry(&src, Scope::Project, &mut dst, Scope::Project, &empty).unwrap();
        assert!(dst.load_tags(Scope::Project).unwrap().is_empty());
    }

    // ------------------------------------------------------------------------
    // Path resolution: `resolve_target_dir` + `open_target_store`
    // ------------------------------------------------------------------------

    #[test]
    fn test_resolve_target_dir_accepts_padz_dir_directly() {
        let temp = tempfile::tempdir().unwrap();
        let padz_dir = temp.path().join(".padz");
        init_layout(&padz_dir);

        let resolved = resolve_target_dir(&padz_dir).unwrap();
        assert_eq!(resolved, padz_dir.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_target_dir_accepts_parent_of_padz_dir() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        init_layout(&project.join(".padz"));

        let resolved = resolve_target_dir(&project).unwrap();
        assert_eq!(resolved, project.join(".padz").canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_target_dir_walks_up_for_padz_dir() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("proj");
        let nested = project.join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        init_layout(&project.join(".padz"));

        let resolved = resolve_target_dir(&nested).unwrap();
        assert_eq!(resolved, project.join(".padz").canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_target_dir_missing_path_errors() {
        let err = resolve_target_dir(std::path::Path::new("/definitely/does/not/exist/anywhere"))
            .unwrap_err();
        // Either "Target path '…': No such file" or "No padz store found" —
        // both signal the same user-visible failure to locate a store.
        let msg = err.to_string();
        assert!(
            msg.contains("Target path") || msg.contains("No padz store"),
            "unexpected error: {msg}"
        );
    }

    /// `FileStore` does not implement `Debug`, so we can't use `unwrap_err()`
    /// directly. Extract the error or panic with a descriptive message.
    fn expect_err<T>(r: Result<T>) -> PadzError {
        match r {
            Err(e) => e,
            Ok(_) => panic!("expected Err, got Ok"),
        }
    }

    #[test]
    fn test_open_target_store_missing_dir_errors() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("nope");
        let err = expect_err(open_target_store(&missing));
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn test_open_target_store_not_a_directory_errors() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("not-a-dir");
        std::fs::write(&file_path, b"hello").unwrap();
        let err = expect_err(open_target_store(&file_path));
        assert!(err.to_string().contains("is not a directory"));
    }

    #[test]
    fn test_open_target_store_missing_active_errors() {
        let temp = tempfile::tempdir().unwrap();
        let padz_dir = temp.path().join(".padz");
        std::fs::create_dir_all(&padz_dir).unwrap();
        // Note: no active/ subdir.
        let err = expect_err(open_target_store(&padz_dir));
        let msg = err.to_string();
        assert!(
            msg.contains("not an initialized padz store") || msg.contains("padz init"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_open_target_store_succeeds_with_initialized_dir() {
        let temp = tempfile::tempdir().unwrap();
        let padz_dir = temp.path().join(".padz");
        init_layout(&padz_dir);
        let store = open_target_store(&padz_dir).unwrap();
        // Default format ext when no padz.toml is present.
        assert_eq!(store.format_ext(), ".txt");
    }
}
