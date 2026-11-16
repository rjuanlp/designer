//! Windows GPU bridge between a wgpu Direct3D 12 device and a Direct3D 11
//! compositor on the same adapter.
//!
//! Vello renders into textures that live in shared Direct3D 12 heaps. The
//! compositor opens the same textures through NT handles and samples them, so
//! no pixel data is copied through the CPU. Two shared fences order the work:
//! one tells the compositor when a frame is complete, the other tells this
//! side when a texture may be written again.
//!
//! This crate deliberately does not depend on GPUI. It hands out plain handle
//! values, because GPUI and wgpu link different versions of the `windows`
//! crate and cannot exchange COM interface types.
//!
//! On other platforms the crate is empty.

#[cfg(windows)]
mod shared_canvas;

#[cfg(windows)]
pub use shared_canvas::{BridgeError, FencePoint, SharedCanvas, SharedFrame, TARGET_FORMAT};
