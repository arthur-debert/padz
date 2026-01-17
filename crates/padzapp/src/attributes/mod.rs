//! # Attribute System
//!
//! This module provides a unified abstraction for pad metadata attributes.
//! Instead of handling each attribute (pinned, deleted, status, tags, etc.) ad-hoc,
//! the attribute system provides:
//!
//! - **Type definitions**: What kinds of values attributes can hold
//! - **Specifications**: Metadata about each attribute (filterable, cascades, etc.)
//! - **Unified access**: `get_attr()` / `set_attr()` methods on Metadata
//! - **Filtering**: Generic filter predicates that work with any attribute
//!
//! ## Attribute Types
//!
//! | Kind | Examples | Description |
//! |------|----------|-------------|
//! | `Bool` | `delete_protected` | Simple true/false |
//! | `BoolWithTimestamp` | `pinned`, `deleted` | Flag + when it was set |
//! | `Enum` | `status` | Closed set of values |
//! | `List` | `tags` | Open set with optional registry |
//! | `Ref` | `parent_id` | Reference to another pad |
//!
//! ## Usage
//!
//! ```ignore
//! // Getting an attribute
//! let value = pad.metadata.get_attr("pinned");
//!
//! // Setting an attribute (handles coupled fields automatically)
//! let effects = pad.metadata.set_attr("pinned", AttrValue::Bool(true));
//!
//! // Filtering
//! let filter = AttrFilter::new("status", FilterOp::Eq, AttrValue::Enum("Done".into()));
//! if filter.matches(&pad.metadata) { ... }
//! ```

mod filter;
mod spec;
mod value;

pub use filter::{AttrFilter, FilterOp};
pub use spec::{filterable_attrs, get_spec, AttributeKind, AttributeSpec, ATTRIBUTES};
pub use value::{AttrSideEffect, AttrValue};
