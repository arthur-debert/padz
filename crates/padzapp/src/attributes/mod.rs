//! # Attribute System
//!
//! A unified abstraction layer for pad metadata attributes.
//!
//! ## Design Motivation
//!
//! Padz metadata fields (pinned, deleted, status, tags, parent_id) were originally
//! handled ad-hoc, with each command directly manipulating struct fields. This led to:
//!
//! - **Scattered coupling logic**: Pinning required setting 3 fields (`is_pinned`,
//!   `pinned_at`, `delete_protected`) in multiple places
//! - **Inconsistent patterns**: Tags used a registry, status propagated up, deletion
//!   had protection checks—each with its own implementation
//! - **Duplicate filter code**: `filter_by_todo_status()` and `filter_by_tags()` were
//!   nearly identical tree-recursive functions
//!
//! The attribute system addresses this by providing:
//!
//! 1. **Centralized coupling**: `set_attr("pinned", true)` handles all 3 fields
//! 2. **Unified type system**: `AttrValue` represents any attribute value
//! 3. **Generic filtering**: One `apply_attr_filters()` replaces multiple filter functions
//! 4. **Declarative specs**: `ATTRIBUTES` registry describes each attribute's behavior
//!
//! ## Architecture Decisions
//!
//! ### Why Not a Full ORM/Schema System?
//!
//! Padz is a simple note-taking app. A full schema migration system or SQL-like
//! query language would be over-engineering. The attribute system is intentionally
//! minimal: it provides structure without framework overhead.
//!
//! ### Why Keep the Flat Metadata Struct?
//!
//! The `Metadata` struct remains flat with individual fields rather than a
//! `HashMap<String, AttrValue>` for several reasons:
//!
//! - **Type safety**: Compile-time field access catches typos
//! - **Serde compatibility**: JSON serialization stays simple and backwards-compatible
//! - **Performance**: No runtime lookups for field access
//! - **Gradual migration**: Commands can transition incrementally
//!
//! The `get_attr()`/`set_attr()` methods are a façade over the flat struct, not
//! a replacement for it.
//!
//! ### Why AttrSideEffect Instead of Automatic Propagation?
//!
//! When `set_attr("status", ...)` is called, the system could automatically
//! propagate the change to parent pads. Instead, it returns `AttrSideEffect::PropagateStatusUp`
//! and lets the caller handle it. This is intentional:
//!
//! - **Store access**: Propagation requires loading/saving other pads, which needs
//!   store access that `set_attr()` (a method on Metadata) doesn't have
//! - **Transaction boundaries**: The caller controls when propagation happens
//! - **Testability**: Side effects are explicit and can be verified
//!
//! ### Display Index Filtering vs Attribute Filtering
//!
//! `PadStatusFilter` (Active/Deleted/Pinned) operates on display indexes, not
//! metadata attributes. A "pinned" pad has `DisplayIndex::Pinned(n)`, which is
//! a view concern, not a data concern. This is why `filter_tree()` remains
//! separate from `apply_attr_filters()`.
//!
//! ## Module Structure
//!
//! - [`spec`]: Attribute specifications and the `ATTRIBUTES` registry
//! - [`value`]: Runtime value types (`AttrValue`) and side effects
//! - [`filter`]: Filter predicates for querying by attribute

mod filter;
mod spec;
mod value;

pub use filter::{AttrFilter, FilterOp};
pub use spec::{filterable_attrs, get_spec, AttributeKind, AttributeSpec, ATTRIBUTES};
pub use value::{AttrSideEffect, AttrValue};
