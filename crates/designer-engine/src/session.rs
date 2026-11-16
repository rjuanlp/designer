//! An open document with its history and file.

use std::path::{Path, PathBuf};

use designer_core::{CommandError, Document, ModelError, NodeId, Operation};

use crate::dsg::{self, DsgError};

/// Most transactions that can be undone. Older ones are forgotten.
pub const MAX_HISTORY: usize = 200;

/// A document state together with an identity that survives undo and redo,
/// so "is this what was saved?" does not need a deep comparison.
#[derive(Clone, Debug)]
struct State {
    id: u64,
    document: Document,
}

/// An open document: current state, undo/redo history and its file.
///
/// Every change goes through [`Session::apply`], which commits a whole
/// transaction or nothing. Each committed transaction is one undo step.
#[derive(Debug)]
pub struct Session {
    current: State,
    undo: Vec<State>,
    redo: Vec<State>,
    next_state_id: u64,
    saved_state: Option<u64>,
    revision: u64,
    path: Option<PathBuf>,
}

impl Session {
    /// A session for a document that has no file yet. It counts as saved
    /// until it is changed.
    pub fn new(document: Document) -> Self {
        Self {
            current: State { id: 0, document },
            undo: Vec::new(),
            redo: Vec::new(),
            next_state_id: 1,
            saved_state: Some(0),
            revision: 0,
            path: None,
        }
    }

    /// Open a `.dsg` file.
    pub fn open(path: &Path) -> Result<Self, DsgError> {
        let mut session = Self::new(dsg::open(path)?);
        session.path = Some(path.to_owned());
        Ok(session)
    }

    /// The current document.
    pub fn document(&self) -> &Document {
        &self.current.document
    }

    /// Number of changes since the session started, counting undo and redo.
    /// It only grows, so an unchanged value means an unchanged document.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The file this document was opened from or last saved to.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Whether the document differs from what was last saved or opened.
    pub fn is_dirty(&self) -> bool {
        self.saved_state != Some(self.current.id)
    }

    /// Whether [`Self::undo`] would change the document.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether [`Self::redo`] would change the document.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Commit operations as one atomic, undoable transaction.
    ///
    /// Returns the identifiers of the nodes created, changed or deleted. On
    /// error the document and the history are unchanged.
    pub fn apply(&mut self, operations: &[Operation]) -> Result<Vec<NodeId>, CommandError> {
        let (document, affected) = self.current.document.transact(operations)?;
        let next = State {
            id: self.next_state_id,
            document,
        };
        self.next_state_id += 1;
        self.undo.push(std::mem::replace(&mut self.current, next));
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
        Ok(affected)
    }

    /// Replace the document as one undoable change.
    pub fn replace_document(&mut self, document: Document) -> Result<(), ModelError> {
        document.validate()?;
        if document == self.current.document {
            return Ok(());
        }
        let next = State {
            id: self.next_state_id,
            document,
        };
        self.next_state_id += 1;
        self.undo.push(std::mem::replace(&mut self.current, next));
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
        Ok(())
    }

    /// Go back one transaction. Returns whether anything changed.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.current, previous));
        self.revision += 1;
        true
    }

    /// Reapply the transaction that was undone last. Returns whether
    /// anything changed.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.current, next));
        self.revision += 1;
        true
    }

    /// Save to the current file. Returns `Ok(false)` without writing when
    /// the document has no file yet.
    pub fn save(&mut self) -> Result<bool, DsgError> {
        let Some(path) = self.path.clone() else {
            return Ok(false);
        };
        self.save_as(&path).map(|()| true)
    }

    /// Save to a file and make it the file of this document. On error the
    /// session keeps its previous file and stays unsaved.
    pub fn save_as(&mut self, path: &Path) -> Result<(), DsgError> {
        dsg::save(&self.current.document, path)?;
        self.path = Some(path.to_owned());
        self.saved_state = Some(self.current.id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use designer_core::{Color, NodeKind, PageId, Rect};

    use super::*;

    fn add(x: f64) -> Operation {
        Operation::AddNode {
            page: PageId(1),
            parent: None,
            name: "n".into(),
            kind: NodeKind::Ellipse,
            bounds: Rect::new(x, 0.0, 10.0, 10.0),
            fill: Some(Color::rgb(1, 2, 3)),
        }
    }

    fn count(session: &Session) -> usize {
        session.document().pages[0].nodes.len()
    }

    #[test]
    fn undo_and_redo_walk_the_history() {
        let mut session = Session::new(Document::empty("t"));
        assert!(!session.can_undo() && !session.undo() && !session.redo());
        assert_eq!(session.apply(&[add(0.0)]).expect("valid"), [NodeId(1)]);
        session.apply(&[add(1.0), add(2.0)]).expect("valid");
        assert_eq!((count(&session), session.revision()), (3, 2));

        // One transaction is one step, however many operations it has.
        assert!(session.undo());
        assert_eq!(count(&session), 1);
        assert!(session.undo());
        assert_eq!(session.document(), &Document::empty("t"));
        assert!(!session.undo());

        assert!(session.redo() && session.redo() && !session.redo());
        assert_eq!((count(&session), session.revision()), (3, 6));
    }

    #[test]
    fn a_new_change_discards_the_redo_branch_and_errors_change_nothing() {
        let mut session = Session::new(Document::empty("t"));
        session.apply(&[add(0.0)]).expect("valid");
        session.undo();
        session.apply(&[add(5.0)]).expect("valid");
        assert!(!session.can_redo());

        let before = session.document().clone();
        let revision = session.revision();
        assert!(session.apply(&[add(f64::NAN)]).is_err());
        assert_eq!(session.document(), &before);
        assert_eq!(session.revision(), revision);
        assert!(session.undo() && !session.undo());
    }

    #[test]
    fn replacing_a_document_is_validated_atomic_and_undoable() {
        let mut session = Session::new(Document::empty("before"));
        session
            .replace_document(Document::empty("after"))
            .expect("valid replacement");
        assert_eq!(session.document().name, "after");
        assert_eq!(session.revision(), 1);
        assert!(session.undo());
        assert_eq!(session.document().name, "before");
        assert!(session.redo());
        assert_eq!(session.document().name, "after");

        let before = session.document().clone();
        let revision = session.revision();
        let mut invalid = Document::empty("invalid");
        invalid.pages.clear();
        assert!(session.replace_document(invalid).is_err());
        assert_eq!(session.document(), &before);
        assert_eq!(session.revision(), revision);
    }

    #[test]
    fn dirty_follows_the_saved_state_through_undo_and_redo() {
        let directory = std::env::temp_dir().join(format!("dsg-session-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("temporary directory");
        let path = directory.join("dirty.dsg");

        let mut session = Session::new(Document::empty("t"));
        assert!(!session.is_dirty());
        assert!(!session.save().expect("no file is not an error"));
        session.apply(&[add(0.0)]).expect("valid");
        assert!(session.is_dirty());
        session.save_as(&path).expect("saves");
        assert!(!session.is_dirty() && session.path() == Some(path.as_path()));

        session.apply(&[add(1.0)]).expect("valid");
        assert!(session.is_dirty());
        session.undo();
        assert!(!session.is_dirty(), "back at the saved state");
        session.undo();
        assert!(session.is_dirty(), "before the saved state");
        session.redo();
        assert!(!session.is_dirty());

        let reopened = Session::open(&path).expect("opens");
        assert_eq!(reopened.document(), session.document());
        assert!(!reopened.is_dirty() && !reopened.can_undo());
        std::fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn history_is_bounded() {
        let mut session = Session::new(Document::empty("t"));
        for index in 0..MAX_HISTORY + 5 {
            session.apply(&[add(index as f64)]).expect("valid");
        }
        let mut undone = 0;
        while session.undo() {
            undone += 1;
        }
        assert_eq!(undone, MAX_HISTORY);
        assert_eq!(count(&session), 5);
    }
}
