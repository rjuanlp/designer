//! Recovery snapshots of documents with unsaved changes.
//!
//! While a document has unsaved changes the application writes it, at most
//! every [`AUTOSAVE_INTERVAL`], to `<data_dir>/recovery/<id>.dsg`, followed by
//! a JSON sidecar `<id>.json` that names the document and its file. Each
//! running instance holds an OS lock on `<id>.lock` while its snapshot exists,
//! so another instance never mistakes a live snapshot for an abandoned one.
//! Snapshots are removed when the document is saved or closed normally.
//!
//! At start, snapshots whose lock is free were left by an instance that did
//! not close normally. They are offered to the user, who recovers or discards
//! each one. Nothing here deletes a snapshot on its own, including one that
//! fails validation.

use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use designer_core::Document;
use serde::{Deserialize, Serialize};

use crate::{
    dsg::{self, DsgError, write_atomic},
    identity,
};

/// Shortest time between two snapshots of the same session.
pub const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(30);
/// Schema version of the sidecar written by this build.
pub const SIDECAR_VERSION: u32 = 1;

/// What the sidecar of a snapshot records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotInfo {
    /// Sidecar schema version.
    pub version: u32,
    /// Name of the document.
    pub document_name: String,
    /// File the document was opened from or saved to, if any.
    pub original_path: Option<PathBuf>,
    /// When the snapshot was written, in milliseconds since the Unix epoch.
    pub saved_at_unix_ms: u64,
}

/// A snapshot left by an instance that did not close normally.
#[derive(Debug)]
pub struct Orphan {
    /// Identifier of the snapshot, the stem of its files.
    pub id: String,
    /// The sidecar, or why it could not be read.
    pub info: Result<SnapshotInfo, String>,
    /// The validated document, or why it could not be read.
    pub document: Result<Document, DsgError>,
    /// The snapshot file.
    pub path: PathBuf,
}

impl Orphan {
    /// When the snapshot was written, if the sidecar says.
    pub fn saved_at_unix_ms(&self) -> Option<u64> {
        self.info.as_ref().ok().map(|info| info.saved_at_unix_ms)
    }
}

/// Where one session writes its snapshot. It can be moved to a background
/// thread; the lock stays with the [`RecoveryStore`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotTarget {
    directory: PathBuf,
    id: String,
}

impl SnapshotTarget {
    /// Write `document` and then its sidecar, each atomically. A crash
    /// between the two leaves a snapshot without a sidecar, which is still
    /// offered for recovery.
    pub fn write(&self, document: &Document, original: Option<&Path>) -> Result<(), DsgError> {
        dsg::save(document, &self.directory.join(format!("{}.dsg", self.id)))?;
        let info = SnapshotInfo {
            version: SIDECAR_VERSION,
            document_name: document.name.clone(),
            original_path: original.map(|path| std::path::absolute(path).unwrap_or(path.into())),
            saved_at_unix_ms: unix_ms(SystemTime::now()),
        };
        let bytes = serde_json::to_vec_pretty(&info).map_err(io::Error::other)?;
        write_atomic(&self.directory.join(format!("{}.json", self.id)), &bytes)?;
        Ok(())
    }
}

/// The recovery directory as seen by one running instance.
#[derive(Debug)]
pub struct RecoveryStore {
    directory: PathBuf,
    id: String,
    lock: Option<File>,
}

impl RecoveryStore {
    /// The store under `data_dir`, with a snapshot identifier unique to this
    /// process. Nothing is written until the first snapshot.
    pub fn new(data_dir: &Path) -> Self {
        Self::with_id(
            data_dir,
            format!("{}-{}", std::process::id(), unix_ms(SystemTime::now())),
        )
    }

    fn with_id(data_dir: &Path, id: String) -> Self {
        Self {
            directory: data_dir.join(identity::RECOVERY_DIRECTORY),
            id,
            lock: None,
        }
    }

    /// The recovery directory.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Identifier of this instance's snapshot.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Create the directory and take this instance's lock, then return where
    /// to write the snapshot.
    pub fn prepare(&mut self) -> io::Result<SnapshotTarget> {
        if self.lock.is_none() {
            fs::create_dir_all(&self.directory)?;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(self.file(&self.id, "lock"))?;
            file.try_lock().map_err(|error| match error {
                fs::TryLockError::WouldBlock => {
                    io::Error::other("recovery snapshot is locked by another process")
                }
                fs::TryLockError::Error(error) => error,
            })?;
            self.lock = Some(file);
        }
        Ok(SnapshotTarget {
            directory: self.directory.clone(),
            id: self.id.clone(),
        })
    }

    /// Remove this instance's snapshot, as after a save or a normal close.
    pub fn clear(&mut self) -> io::Result<()> {
        let Some(lock) = self.lock.take() else {
            return Ok(());
        };
        remove_if_present(&self.file(&self.id, "dsg"))?;
        remove_if_present(&self.file(&self.id, "json"))?;
        drop(lock);
        remove_if_present(&self.file(&self.id, "lock"))
    }

    /// Whether this instance currently holds a snapshot.
    pub fn has_snapshot(&self) -> bool {
        self.lock.is_some()
    }

    /// Snapshots left by instances that did not close normally, newest
    /// first. Snapshots locked by a running instance are skipped.
    pub fn orphans(&self) -> Vec<Orphan> {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                let extension = path.extension()?.to_str()?;
                (extension == "dsg" || extension == "json")
                    .then(|| path.file_stem()?.to_str().map(str::to_owned))
                    .flatten()
            })
            // Temporary files of an interrupted write start with a dot.
            .filter(|id| !id.starts_with('.') && *id != self.id)
            .collect();
        ids.sort();
        ids.dedup();
        let mut orphans: Vec<Orphan> = ids
            .into_iter()
            .filter(|id| !self.is_live(id))
            .map(|id| {
                let path = self.file(&id, "dsg");
                let info = fs::read(self.file(&id, "json"))
                    .map_err(|error| error.to_string())
                    .and_then(|bytes| {
                        serde_json::from_slice::<SnapshotInfo>(&bytes)
                            .map_err(|error| error.to_string())
                    });
                Orphan {
                    document: dsg::open(&path),
                    info,
                    path,
                    id,
                }
            })
            .collect();
        orphans.sort_by_key(|orphan| std::cmp::Reverse(orphan.saved_at_unix_ms()));
        orphans
    }

    /// Delete an abandoned snapshot. Only for an explicit user decision;
    /// snapshots locked by a running instance are refused.
    pub fn discard(&self, id: &str) -> io::Result<()> {
        if id == self.id || id.contains(['/', '\\']) || id.starts_with('.') {
            return Err(io::Error::other("not an abandoned recovery snapshot"));
        }
        if self.is_live(id) {
            return Err(io::Error::other(
                "the snapshot belongs to a running Designer window",
            ));
        }
        remove_if_present(&self.file(id, "dsg"))?;
        remove_if_present(&self.file(id, "json"))?;
        remove_if_present(&self.file(id, "lock"))
    }

    fn file(&self, id: &str, extension: &str) -> PathBuf {
        self.directory.join(format!("{id}.{extension}"))
    }

    /// Whether another running instance holds the lock of snapshot `id`.
    fn is_live(&self, id: &str) -> bool {
        let Ok(file) = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.file(id, "lock"))
        else {
            return false;
        };
        matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock))
    }
}

/// When a session should next write its snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutosaveClock {
    written_revision: Option<u64>,
    last_write: Option<Instant>,
}

impl AutosaveClock {
    /// Whether a snapshot is due for a session at `revision`. Clean sessions
    /// never need one; an unchanged revision is already written; otherwise
    /// writes are at least [`AUTOSAVE_INTERVAL`] apart.
    pub fn due(&self, dirty: bool, revision: u64, now: Instant) -> bool {
        dirty
            && self.written_revision != Some(revision)
            && self
                .last_write
                .is_none_or(|last| now.saturating_duration_since(last) >= AUTOSAVE_INTERVAL)
    }

    /// Record an attempt to write `revision` at `now`. A failed attempt is
    /// recorded with `None` so it is retried after the interval.
    pub fn record(&mut self, revision: Option<u64>, now: Instant) {
        self.written_revision = revision;
        self.last_write = Some(now);
    }

    /// Forget everything, as after the snapshot was removed.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn unix_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |elapsed| {
        elapsed.as_millis().try_into().unwrap_or(u64::MAX)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use designer_core::sample_document;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("designer-recovery-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_live_snapshot_is_hidden_and_an_abandoned_one_is_offered() {
        let dir = scratch("orphans");
        let document = sample_document();
        let mut first = RecoveryStore::with_id(&dir, "1-1".into());
        first
            .prepare()
            .expect("prepared")
            .write(&document, Some(Path::new("logo.dsg")))
            .expect("written");
        let second = RecoveryStore::with_id(&dir, "2-2".into());
        // The first instance still runs, so its snapshot is not abandoned.
        assert!(second.orphans().is_empty());
        assert!(second.discard("1-1").is_err());

        // Simulate a crash: the lock is released without clearing.
        drop(first);
        let orphans = second.orphans();
        assert_eq!(orphans.len(), 1);
        let orphan = &orphans[0];
        assert_eq!(orphan.id, "1-1");
        assert_eq!(orphan.document.as_ref().expect("valid"), &document);
        let info = orphan.info.as_ref().expect("sidecar");
        assert_eq!(info.document_name, document.name);
        assert!(
            info.original_path
                .as_deref()
                .is_some_and(|path| path.is_absolute() && path.ends_with("logo.dsg"))
        );

        second.discard("1-1").expect("discarded");
        assert!(second.orphans().is_empty());
        assert_eq!(fs::read_dir(dir.join("recovery")).expect("dir").count(), 0);
        fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn clearing_removes_every_file_of_the_snapshot() {
        let dir = scratch("clear");
        let mut store = RecoveryStore::with_id(&dir, "3-3".into());
        assert!(!store.has_snapshot());
        store.clear().expect("nothing to clear");
        store
            .prepare()
            .expect("prepared")
            .write(&sample_document(), None)
            .expect("written");
        assert!(store.has_snapshot());
        assert_eq!(fs::read_dir(store.directory()).expect("dir").count(), 3);
        store.clear().expect("cleared");
        assert!(!store.has_snapshot());
        assert_eq!(fs::read_dir(store.directory()).expect("dir").count(), 0);
        fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn invalid_or_incomplete_snapshots_are_reported_not_deleted() {
        let dir = scratch("invalid");
        let store = RecoveryStore::with_id(&dir, "self".into());
        let recovery = dir.join("recovery");
        fs::create_dir_all(&recovery).expect("dir");
        fs::write(recovery.join("4-4.dsg"), b"not a zip").expect("written");
        fs::write(recovery.join("4-4.json"), b"{}").expect("written");
        // A document whose sidecar was never written.
        dsg::save(&sample_document(), &recovery.join("5-5.dsg")).expect("saved");
        // A temporary file of an interrupted write is ignored.
        fs::write(recovery.join(".5-5.dsg.1.tmp"), b"partial").expect("written");

        let orphans = store.orphans();
        assert_eq!(orphans.len(), 2);
        let broken = orphans.iter().find(|o| o.id == "4-4").expect("listed");
        assert!(broken.document.is_err() && broken.info.is_err());
        let bare = orphans.iter().find(|o| o.id == "5-5").expect("listed");
        assert!(bare.document.is_ok() && bare.info.is_err());
        // Listing never removes anything.
        assert_eq!(fs::read_dir(&recovery).expect("dir").count(), 4);
        assert!(store.discard("../x").is_err());
        fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn autosave_waits_for_changes_and_the_interval() {
        let start = Instant::now();
        let mut clock = AutosaveClock::default();
        assert!(
            !clock.due(false, 1, start),
            "clean sessions need no snapshot"
        );
        assert!(
            clock.due(true, 1, start),
            "the first change is written at once"
        );
        clock.record(Some(1), start);
        assert!(!clock.due(true, 1, start + AUTOSAVE_INTERVAL * 2));
        assert!(!clock.due(true, 2, start + Duration::from_secs(29)));
        assert!(clock.due(true, 2, start + AUTOSAVE_INTERVAL));
        clock.record(None, start + AUTOSAVE_INTERVAL);
        assert!(!clock.due(true, 2, start + AUTOSAVE_INTERVAL));
        assert!(clock.due(true, 2, start + AUTOSAVE_INTERVAL * 2));
        clock.reset();
        assert!(clock.due(true, 2, start));
    }
}
