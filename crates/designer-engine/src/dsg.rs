//! Reading and writing `.dsg` files.
//!
//! A `.dsg` file is a ZIP archive with two UTF-8 JSON entries:
//! `manifest.json` identifies the format and its schema version, and
//! `document.json` holds the document. Sessions use OS sidecar locks; recovery
//! snapshots are stored separately under the per-user data directory. Assets
//! are not implemented yet.
//!
//! A reader never publishes a document it has not validated, and a failed
//! save leaves the previous file untouched.

use std::{
    error::Error,
    fmt, fs,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use designer_core::{Document, ModelError};
use serde::{Deserialize, Serialize};
use zip::{CompressionMethod, ZipArchive, ZipWriter, result::ZipError, write::SimpleFileOptions};

/// Value of `format` in the manifest.
pub const FORMAT: &str = "designer-dsg";
/// Newest schema version this build reads and writes.
pub const SCHEMA_VERSION: u32 = 4;
/// File extension, without the dot.
pub const EXTENSION: &str = "dsg";
/// Largest accepted `.dsg` file.
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// Largest accepted uncompressed `document.json`.
pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
/// Largest accepted uncompressed `manifest.json`.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
/// Most entries an archive may contain.
pub const MAX_ENTRIES: usize = 4096;

const MANIFEST_ENTRY: &str = "manifest.json";
const DOCUMENT_ENTRY: &str = "document.json";

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    schema_version: u32,
    #[serde(default)]
    generator: String,
}

/// Why a `.dsg` file could not be read or written.
#[derive(Debug)]
pub enum DsgError {
    /// Another session holds the operating-system lock.
    Locked {
        /// Document path supplied by the caller.
        path: PathBuf,
    },
    /// The file system refused the operation.
    Io(io::Error),
    /// The file is not a readable ZIP archive.
    NotAnArchive(String),
    /// A required entry is missing.
    MissingEntry(&'static str),
    /// An entry or the file is larger than this build accepts.
    TooLarge(&'static str),
    /// An entry name could escape the archive (absolute or with `..`).
    UnsafePath(String),
    /// The manifest is not valid JSON or does not name this format.
    BadManifest(String),
    /// The file was written by a newer, incompatible schema.
    UnsupportedVersion(u32),
    /// `document.json` does not describe a document.
    BadDocument(String),
    /// The document was read but breaks a model invariant.
    Invalid(ModelError),
}

impl fmt::Display for DsgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locked { path } => write!(
                f,
                "document is locked by another session: {}",
                path.display()
            ),
            Self::Io(error) => write!(f, "file system error: {error}"),
            Self::NotAnArchive(reason) => write!(f, "not a .dsg archive: {reason}"),
            Self::MissingEntry(name) => write!(f, "the archive has no {name}"),
            Self::TooLarge(what) => write!(f, "{what} is larger than this version accepts"),
            Self::UnsafePath(name) => write!(f, "the archive has an unsafe entry name: {name}"),
            Self::BadManifest(reason) => write!(f, "invalid manifest: {reason}"),
            Self::UnsupportedVersion(version) => write!(
                f,
                "schema version {version} is not supported (this version reads {SCHEMA_VERSION})"
            ),
            Self::BadDocument(reason) => write!(f, "invalid document data: {reason}"),
            Self::Invalid(error) => write!(f, "invalid document: {error}"),
        }
    }
}

impl Error for DsgError {}

impl From<io::Error> for DsgError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn archive_error(error: ZipError) -> DsgError {
    match error {
        ZipError::Io(error) => DsgError::Io(error),
        other => DsgError::NotAnArchive(other.to_string()),
    }
}

/// Serialize a document as `.dsg` bytes. An invalid document is refused.
pub fn to_bytes(document: &Document) -> Result<Vec<u8>, DsgError> {
    document.validate().map_err(DsgError::Invalid)?;
    let manifest = Manifest {
        format: FORMAT.to_owned(),
        schema_version: SCHEMA_VERSION,
        generator: format!("designer {}", env!("CARGO_PKG_VERSION")),
    };
    let manifest =
        serde_json::to_vec_pretty(&manifest).map_err(|e| DsgError::BadManifest(e.to_string()))?;
    let body =
        serde_json::to_vec_pretty(document).map_err(|e| DsgError::BadDocument(e.to_string()))?;
    if body.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(DsgError::TooLarge(DOCUMENT_ENTRY));
    }
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [(MANIFEST_ENTRY, &manifest), (DOCUMENT_ENTRY, &body)] {
        writer.start_file(name, options).map_err(archive_error)?;
        writer.write_all(bytes)?;
    }
    Ok(writer.finish().map_err(archive_error)?.into_inner())
}

fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &'static str,
    limit: u64,
) -> Result<Vec<u8>, DsgError> {
    let entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(ZipError::FileNotFound) => return Err(DsgError::MissingEntry(name)),
        Err(error) => return Err(archive_error(error)),
    };
    // The declared size can lie, so the read itself is bounded too.
    if entry.size() > limit {
        return Err(DsgError::TooLarge(name));
    }
    let mut bytes = Vec::new();
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| DsgError::NotAnArchive(error.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(DsgError::TooLarge(name));
    }
    Ok(bytes)
}

/// Read and validate a document from `.dsg` data.
pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Document, DsgError> {
    let mut reader = reader;
    let start = reader.stream_position()?;
    let end = reader.seek(SeekFrom::End(0))?;
    if end < start {
        return Err(DsgError::NotAnArchive(
            "reader returned an invalid length".to_owned(),
        ));
    }
    if end - start > MAX_FILE_BYTES {
        return Err(DsgError::TooLarge("the file"));
    }
    reader.seek(SeekFrom::Start(start))?;
    let mut archive = ZipArchive::new(reader).map_err(archive_error)?;
    if archive.len() > MAX_ENTRIES {
        return Err(DsgError::TooLarge("the archive entry list"));
    }
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(archive_error)?;
        if entry.enclosed_name().is_none() {
            return Err(DsgError::UnsafePath(entry.name().to_owned()));
        }
    }
    let manifest = read_entry(&mut archive, MANIFEST_ENTRY, MAX_MANIFEST_BYTES)?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest).map_err(|e| DsgError::BadManifest(e.to_string()))?;
    if manifest.format != FORMAT {
        return Err(DsgError::BadManifest(format!(
            "format is {:?}, expected {FORMAT:?}",
            manifest.format
        )));
    }
    if manifest.schema_version == 0 {
        return Err(DsgError::BadManifest("schema_version is 0".to_owned()));
    }
    if manifest.schema_version > SCHEMA_VERSION {
        return Err(DsgError::UnsupportedVersion(manifest.schema_version));
    }
    let body = read_entry(&mut archive, DOCUMENT_ENTRY, MAX_DOCUMENT_BYTES)?;
    let document: Document =
        serde_json::from_slice(&body).map_err(|e| DsgError::BadDocument(e.to_string()))?;
    document.validate().map_err(DsgError::Invalid)?;
    Ok(document)
}

/// Read and validate a document from `.dsg` bytes.
pub fn from_bytes(bytes: &[u8]) -> Result<Document, DsgError> {
    from_reader(Cursor::new(bytes))
}

/// Open a `.dsg` file. The file is only read.
pub fn open(path: &Path) -> Result<Document, DsgError> {
    let file = fs::File::open(path)?;
    from_reader(io::BufReader::new(file))
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// Save a document to a `.dsg` file.
///
/// The data is written to a sibling temporary file, flushed to disk and then
/// moved over the destination, so an existing file is either fully replaced
/// or left as it was.
pub fn save(document: &Document, path: &Path) -> Result<(), DsgError> {
    let bytes = to_bytes(document)?;
    Ok(write_atomic(path, &bytes)?)
}

/// Write `bytes` to `path` through a flushed sibling temporary file that is
/// then renamed over the destination, so readers see either the old or the
/// new content, never a partial file.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = temporary_path(path);
    let written = (|| {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if written.is_err() {
        // Best effort: the destination is untouched either way.
        let _ = fs::remove_file(&temporary);
    }
    written
}
