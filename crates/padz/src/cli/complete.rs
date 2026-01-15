//! Shell completion support using clap_complete's dynamic completion.
//!
//! This module provides custom completers for pad indexes and titles.

use clap_complete::engine::{ArgValueCandidates, CompletionCandidate};
use padzapp::api::{PadFilter, PadStatusFilter, PadzApi};
use padzapp::init::initialize;
use padzapp::store::fs::FileStore;
use std::path::PathBuf;

/// Returns completion candidates for pad indexes and titles.
///
/// This completer:
/// - Detects --global/-g flag from command line to determine scope
/// - Returns both numeric indexes (1, 2, p1, d1) and titles
/// - Filters based on whether deleted pads should be included
fn get_pad_candidates(include_deleted: bool) -> Vec<CompletionCandidate> {
    // Parse args to detect --global flag
    let args: Vec<String> = std::env::args().collect();
    let is_global = args.iter().any(|a| a == "-g" || a == "--global");

    // Initialize context (completions don't support --data override)
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let ctx = initialize(&cwd, is_global, None);
    let api: PadzApi<FileStore> = ctx.api;

    // Query pads
    let filter = PadFilter {
        status: if include_deleted {
            PadStatusFilter::All
        } else {
            PadStatusFilter::Active
        },
        search_term: None,
        todo_status: None,
    };

    let Ok(result) = api.get_pads(ctx.scope, filter) else {
        return vec![];
    };

    let mut candidates = Vec::new();

    for dp in result.listed_pads {
        let index_str = dp.index.to_string();
        let title = &dp.pad.metadata.title;

        // Add index as candidate with title as help text
        candidates.push(
            CompletionCandidate::new(index_str.clone())
                .help(Some(title.clone().into()))
                .display_order(Some(0)),
        );

        // Add title as candidate (for title-based lookup)
        // Only add if title differs from index (avoid duplicates for numeric-only titles)
        if title != &index_str {
            candidates.push(
                CompletionCandidate::new(title.clone())
                    .help(Some(format!("({})", index_str).into()))
                    .display_order(Some(1)),
            );
        }
    }

    candidates
}

/// Completer for active pads (view, edit, open, delete, pin, unpin, path, export)
pub fn active_pads_completer() -> ArgValueCandidates {
    ArgValueCandidates::new(|| get_pad_candidates(false))
}

/// Completer for commands that can access deleted pads (view, open, path with --deleted context)
pub fn all_pads_completer() -> ArgValueCandidates {
    ArgValueCandidates::new(|| get_pad_candidates(true))
}

/// Completer for deleted-only pads (restore, purge)
pub fn deleted_pads_completer() -> ArgValueCandidates {
    ArgValueCandidates::new(|| {
        let args: Vec<String> = std::env::args().collect();
        let is_global = args.iter().any(|a| a == "-g" || a == "--global");

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let ctx = initialize(&cwd, is_global, None);
        let api: PadzApi<FileStore> = ctx.api;

        let filter = PadFilter {
            status: PadStatusFilter::Deleted,
            search_term: None,
            todo_status: None,
        };

        let Ok(result) = api.get_pads(ctx.scope, filter) else {
            return vec![];
        };

        result
            .listed_pads
            .into_iter()
            .map(|dp| {
                let index_str = dp.index.to_string();
                CompletionCandidate::new(index_str)
                    .help(Some(dp.pad.metadata.title.into()))
                    .display_order(Some(0))
            })
            .collect()
    })
}
