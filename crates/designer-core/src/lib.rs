//! Designer document model.
//!
//! This crate has no UI, GPU or I/O dependency. It holds the typed node tree,
//! keyframe animation with deterministic evaluation, the typed operations that
//! change a document as atomic transactions, and a built-in sample document.
//! Layout, components and text shaping are not implemented yet. History and
//! `.dsg` persistence live in `designer-engine`.

mod animation;
mod command;
mod model;
mod sample;

pub use animation::{
    Animation, Easing, Keyframe, MAX_DURATION_MS, MAX_KEYS_PER_TRACK, Property, Track,
};
pub use command::{CommandError, MAX_NAME_CHARS, MAX_OPERATIONS, MAX_TEXT_CHARS, Operation};
pub use model::{Color, Document, ModelError, Node, NodeId, NodeKind, Page, PageId, Rect};
pub use sample::sample_document;
