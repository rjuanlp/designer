//! Designer document sessions.
//!
//! [`Session`] owns an open document: it commits transactions, keeps the
//! undo/redo history and knows the file of the document. [`dsg`] reads and
//! writes `.dsg` files. [`import`] handles supported structured Figma JSON.
//! There is no UI or GPU dependency.
//!
//! Not implemented yet: assets and recovery snapshots. `bridge` contains the
//! revision-checked session protocol used by the Windows GUI/MCP IPC.

pub mod bridge;
pub mod dsg;
pub mod import;
mod session;

pub use dsg::DsgError;
pub use session::{MAX_HISTORY, Session};
