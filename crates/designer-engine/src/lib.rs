//! Designer document sessions.
//!
//! [`Session`] owns an open document: it commits transactions, keeps the
//! undo/redo history and knows the file of the document. [`dsg`] reads and
//! writes `.dsg` files. There is no UI or GPU dependency.
//!
//! Not implemented yet: assets, a write lock, recovery snapshots, revision
//! checks between several clients and the IPC used by agents.

pub mod dsg;
mod session;

pub use dsg::DsgError;
pub use session::{MAX_HISTORY, Session};
