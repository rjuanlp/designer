//! An open document with its history and file.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Debug)]
struct DocumentLock {
    file: File,
    path: PathBuf,
}
impl DocumentLock {
    fn acquire(path: &Path, document_path: &Path) -> Result<Self, DsgError> {
        let mut name = path.as_os_str().to_owned();
        name.push(".lock");
        let path = PathBuf::from(name);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => DsgError::Locked {
                path: document_path.to_owned(),
            },
            std::fs::TryLockError::Error(error) => DsgError::Io(error),
        })?;
        file.set_len(0)?;
        writeln!(file, "{}", std::process::id())?;
        Ok(Self { file, path })
    }
}
impl Drop for DocumentLock {
    fn drop(&mut self) {
        // Keep the OS lock held during best-effort removal.
        #[cfg(windows)]
        let _ = std::fs::remove_file(&self.path);
        let _ = &self.path;
        let _ = &self.file;
    }
}
fn normalized_path(path: &Path) -> Result<PathBuf, DsgError> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    let absolute = std::path::absolute(path)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent"))?
        .canonicalize()?;
    Ok(parent.join(
        absolute
            .file_name()
            .ok_or_else(|| std::io::Error::other("missing file name"))?,
    ))
}

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
    file_lock: Option<DocumentLock>,
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
            file_lock: None,
        }
    }

    /// Open a `.dsg` file.
    pub fn open(path: &Path) -> Result<Self, DsgError> {
        let lock = DocumentLock::acquire(&normalized_path(path)?, path)?;
        let mut session = Self::new(dsg::open(path)?);
        session.file_lock = Some(lock);
        session.path = Some(path.to_owned());
        Ok(session)
    }

    /// A session for a document recovered from a snapshot. It starts with
    /// unsaved changes. When `original` can be locked it becomes the file of
    /// the document, so a plain save writes back to it; otherwise the
    /// document has no file and must be saved under a new name.
    pub fn recovered(document: Document, original: Option<&Path>) -> Self {
        let mut session = Self::new(document);
        session.saved_state = None;
        if let Some(path) = original
            && let Ok(lock) = normalized_path(path)
                .and_then(|normalized| DocumentLock::acquire(&normalized, path))
        {
            session.file_lock = Some(lock);
            session.path = Some(path.to_owned());
        }
        session
    }

    /// Reload a path, retaining ownership when reopening the current file.
    pub fn reopen(&mut self, path: &Path) -> Result<Self, DsgError> {
        if self
            .path
            .as_deref()
            .map(normalized_path)
            .transpose()?
            .as_ref()
            == Some(&normalized_path(path)?)
        {
            let mut session = Self::new(dsg::open(path)?);
            session.path = Some(path.to_owned());
            session.file_lock = self.file_lock.take();
            Ok(session.succeeding(self))
        } else {
            Self::open(path).map(|session| session.succeeding(self))
        }
    }

    /// Prepare this session to replace `previous` without changing its document,
    /// saved state, file ownership or undo/redo history.
    /// Its revision is strictly greater than the replaced session's revision,
    /// so requests read from the previous document cannot pass revision checks.
    ///
    /// # Panics
    /// Panics if `previous` has exhausted the revision counter.
    pub fn succeeding(mut self, previous: &Session) -> Self {
        self.revision = self.revision.max(
            previous
                .revision
                .checked_add(1)
                .expect("session revision exhausted"),
        );
        self
    }

    /// The current document.
    pub fn document(&self) -> &Document {
        &self.current.document
    }

    /// Monotonic document revision, counting changes, undo and redo.
    /// It only grows, so an unchanged value means an unchanged document.
    /// Replacements prepared with [`Self::succeeding`] have a strictly greater
    /// revision than the session they replace.
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
        let normalized = normalized_path(path)?;
        let lock = if self
            .path
            .as_deref()
            .map(normalized_path)
            .transpose()?
            .as_ref()
            == Some(&normalized)
        {
            None
        } else {
            Some(DocumentLock::acquire(&normalized, path)?)
        };
        dsg::save(&self.current.document, path)?;
        if let Some(lock) = lock {
            self.file_lock = Some(lock);
        }
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
    fn replacement_revisions_reject_requests_from_previous_documents() {
        use crate::bridge::{SessionRequest, execute_session_request};

        for edits in [0, 3] {
            let mut previous = Session::new(Document::empty("previous"));
            for index in 0..edits {
                previous.apply(&[add(f64::from(index))]).expect("edit");
            }
            let mut replacement =
                Session::new(Document::empty("replacement")).succeeding(&previous);
            let revision = replacement.revision();
            assert!(revision > previous.revision());
            assert!(!replacement.is_dirty());
            assert!(!replacement.can_undo() && !replacement.can_redo());
            let request = |expected_revision| SessionRequest::ApplyTransaction {
                expected_revision,
                operations: vec![add(0.0)],
            };
            let error = execute_session_request(&mut replacement, request(previous.revision()))
                .expect_err("old document revision must conflict");
            assert!(error.contains("revision conflict"));
            assert_eq!(replacement.revision(), revision);
            assert_eq!(count(&replacement), 0);
            execute_session_request(&mut replacement, request(revision)).expect("current revision");
            assert!(replacement.is_dirty() && replacement.can_undo());
            assert!(replacement.undo());
            assert!(!replacement.is_dirty());
        }
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

        assert!(matches!(Session::open(&path), Err(DsgError::Locked { .. })));
        let document = session.document().clone();
        drop(session);
        let reopened = Session::open(&path).expect("opens");
        assert_eq!(reopened.document(), &document);
        assert!(!reopened.is_dirty() && !reopened.can_undo());
        drop(reopened);
        std::fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn locks_preserve_sessions_and_repeated_saves_accept_stale_sidecars() {
        let directory = std::env::temp_dir().join(format!("dsg-locks-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("directory");
        let a = directory.join("a.dsg");
        let b = directory.join("b.dsg");
        std::fs::write(directory.join("a.dsg.lock"), "stale PID").expect("stale sidecar");
        let mut first = Session::new(Document::empty("first"));
        first.save_as(&a).expect("stale sidecar does not block");
        first.save().expect("own save");
        first.save_as(&a).expect("own save as");
        let mut second = Session::new(Document::empty("second"));
        second.save_as(&b).expect("second file");
        let before = std::fs::read(&a).expect("bytes");
        assert!(matches!(second.save_as(&a), Err(DsgError::Locked { .. })));
        assert_eq!(std::fs::read(&a).expect("bytes"), before);
        assert_eq!(second.path(), Some(b.as_path()));
        assert_eq!(first.path(), Some(a.as_path()));
        assert_eq!(second.document().name, "second");
        assert_eq!(first.document().name, "first");
        assert!(matches!(Session::open(&a), Err(DsgError::Locked { .. })));
        drop(first);
        let reopened = Session::open(&a).expect("released lock");
        drop(reopened);
        drop(second);
        std::fs::remove_dir_all(directory).expect("cleanup");
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

    #[test]
    fn reopening_own_path_reloads_and_retains_lock() {
        let path = std::env::temp_dir().join(format!("reload-{}.dsg", std::process::id()));
        let mut session = Session::new(Document::empty("saved"));
        session.save_as(&path).expect("save");
        session.apply(&[add(0.0)]).expect("edit");
        let previous_revision = session.revision();
        session = session.reopen(&path).expect("reload own file");
        assert!(session.revision() > previous_revision);
        assert_eq!(count(&session), 0);
        assert!(!session.is_dirty());
        let error = Session::open(&path).expect_err("still locked");
        assert!(matches!(error, DsgError::Locked { path: ref p } if p == &path));
        drop(session);
        drop(Session::open(&path).expect("released"));
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn a_recovered_session_is_unsaved_and_writes_back_only_when_it_can_lock() {
        let path = std::env::temp_dir().join(format!("recovered-{}.dsg", std::process::id()));
        let mut owner = Session::new(Document::empty("owner"));
        owner.save_as(&path).expect("save");
        let mut document = Document::empty("recovered");
        document.name = "recovered".into();

        // Another session holds the file: the recovered one has no file.
        let blocked = Session::recovered(document.clone(), Some(&path));
        assert!(blocked.is_dirty());
        assert_eq!(blocked.path(), None);

        drop(owner);
        let mut recovered = Session::recovered(document, Some(&path));
        assert!(recovered.is_dirty());
        assert_eq!(recovered.path(), Some(path.as_path()));
        assert!(recovered.save().expect("saved"));
        assert!(!recovered.is_dirty());
        assert_eq!(dsg::open(&path).expect("read").name, "recovered");
        drop(recovered);
        std::fs::remove_file(path).expect("cleanup");
    }
}
