//! Contract of the `.dsg` reader and writer: what round-trips and what is
//! refused.

use std::io::{Cursor, Write};

use designer_core::{Document, sample_document};
use designer_engine::dsg::{self, DsgError};
use zip::{ZipWriter, write::SimpleFileOptions};

const GOOD_MANIFEST: &str = r#"{"format":"designer-dsg","schema_version":1}"#;

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .expect("entry");
        writer.write_all(bytes).expect("bytes");
    }
    writer.finish().expect("archive").into_inner()
}

fn document_json(document: &Document) -> Vec<u8> {
    serde_json::to_vec(document).expect("json")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("dsg-contract-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temporary directory");
    directory
}

#[test]
fn the_sample_document_round_trips_with_stable_identifiers() {
    let document = sample_document();
    let bytes = dsg::to_bytes(&document).expect("writes");
    assert_eq!(&bytes[..2], b"PK", "a .dsg file is a ZIP archive");
    assert_eq!(dsg::from_bytes(&bytes).expect("reads"), document);
}

#[test]
fn files_are_saved_and_replaced_without_leftovers() {
    let directory = scratch("save");
    let path = directory.join("drawing.dsg");
    dsg::save(&Document::empty("first"), &path).expect("saves");
    dsg::save(&sample_document(), &path).expect("replaces");
    assert_eq!(dsg::open(&path).expect("opens"), sample_document());
    let names: Vec<_> = std::fs::read_dir(&directory)
        .expect("lists")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(names, ["drawing.dsg"], "no temporary file is left behind");
    std::fs::remove_dir_all(&directory).expect("cleanup");
}

#[test]
fn a_failed_save_keeps_the_previous_file() {
    let directory = scratch("failed");
    let path = directory.join("kept.dsg");
    dsg::save(&sample_document(), &path).expect("saves");

    let mut invalid = Document::empty("bad");
    invalid.pages.clear();
    assert!(matches!(
        dsg::save(&invalid, &path),
        Err(DsgError::Invalid(_))
    ));
    assert!(matches!(
        dsg::save(&sample_document(), &directory.join("missing").join("x.dsg")),
        Err(DsgError::Io(_))
    ));
    assert_eq!(dsg::open(&path).expect("still opens"), sample_document());
    assert_eq!(std::fs::read_dir(&directory).expect("lists").count(), 1);
    std::fs::remove_dir_all(&directory).expect("cleanup");
}

#[test]
fn damaged_and_foreign_files_are_refused() {
    let good = document_json(&Document::empty("t"));
    let whole = dsg::to_bytes(&sample_document()).expect("writes");

    assert!(matches!(
        dsg::from_bytes(b"not a zip"),
        Err(DsgError::NotAnArchive(_))
    ));
    assert!(dsg::from_bytes(&whole[..whole.len() / 2]).is_err());
    assert!(matches!(
        dsg::from_bytes(&archive(&[("document.json", &good)])),
        Err(DsgError::MissingEntry("manifest.json"))
    ));
    assert!(matches!(
        dsg::from_bytes(&archive(&[("manifest.json", GOOD_MANIFEST.as_bytes())])),
        Err(DsgError::MissingEntry("document.json"))
    ));
    for manifest in [
        "{",
        r#"{"format":"other","schema_version":1}"#,
        r#"{"format":"designer-dsg"}"#,
        r#"{"format":"designer-dsg","schema_version":0}"#,
        r#"{"format":"designer-dsg","schema_version":-1}"#,
    ] {
        let bytes = archive(&[
            ("manifest.json", manifest.as_bytes()),
            ("document.json", &good),
        ]);
        assert!(
            matches!(dsg::from_bytes(&bytes), Err(DsgError::BadManifest(_))),
            "{manifest} was accepted"
        );
    }
    let future = archive(&[
        (
            "manifest.json",
            br#"{"format":"designer-dsg","schema_version":2}"#,
        ),
        ("document.json", &good),
    ]);
    assert!(matches!(
        dsg::from_bytes(&future),
        Err(DsgError::UnsupportedVersion(2))
    ));
}

#[test]
fn documents_that_break_the_model_are_refused() {
    let with_document = |json: &[u8]| {
        dsg::from_bytes(&archive(&[
            ("manifest.json", GOOD_MANIFEST.as_bytes()),
            ("document.json", json),
        ]))
    };
    assert!(matches!(
        with_document(b"[1, 2"),
        Err(DsgError::BadDocument(_))
    ));
    assert!(matches!(
        with_document(br#"{"name":"t"}"#),
        Err(DsgError::BadDocument(_))
    ));

    // Well-formed JSON whose values break an invariant.
    let mut no_pages = Document::empty("t");
    no_pages.pages.clear();
    assert!(matches!(
        with_document(&document_json(&no_pages)),
        Err(DsgError::Invalid(_))
    ));
    let mut stale = sample_document();
    stale.next_node_id = 1;
    assert!(matches!(
        with_document(&document_json(&stale)),
        Err(DsgError::Invalid(_))
    ));
    let negative = String::from_utf8(document_json(&sample_document()))
        .expect("utf-8")
        .replacen("\"width\":", "\"width\":-", 1);
    assert!(matches!(
        with_document(negative.as_bytes()),
        Err(DsgError::Invalid(_))
    ));
}

#[test]
fn unsafe_entry_names_and_oversized_manifests_are_refused() {
    let good = document_json(&Document::empty("t"));
    let escaping = archive(&[
        ("manifest.json", GOOD_MANIFEST.as_bytes()),
        ("document.json", &good),
        ("../outside.txt", b"x"),
    ]);
    assert!(matches!(
        dsg::from_bytes(&escaping),
        Err(DsgError::UnsafePath(_))
    ));

    let padding = " ".repeat(dsg::MAX_MANIFEST_BYTES as usize);
    let big = format!("{GOOD_MANIFEST}{padding}");
    let oversized = archive(&[("manifest.json", big.as_bytes()), ("document.json", &good)]);
    assert!(matches!(
        dsg::from_bytes(&oversized),
        Err(DsgError::TooLarge("manifest.json"))
    ));
}
