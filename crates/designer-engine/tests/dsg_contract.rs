//! Contract of the `.dsg` reader and writer: what round-trips and what is
//! refused.

use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use designer_core::{
    DEFAULT_PROTOTYPE_DURATION_MS, Document, Easing, FillOverride, INITIAL_STATE, NodeOverride,
    PrototypeState, PrototypeStateId, PrototypeTransition, PrototypeTrigger, Rect, sample_document,
};
use designer_engine::dsg::{self, DsgError};
use zip::{ZipWriter, write::SimpleFileOptions};

const GOOD_MANIFEST: &str = r#"{"format":"designer-dsg","schema_version":1}"#;

#[test]
fn version_three_frames_default_to_unclipped_and_v4_preserves_clipping() {
    let mut document = sample_document();
    let mut json = serde_json::to_value(&document).unwrap();
    fn remove_clipping(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                object.remove("clips_content");
                for child in object.values_mut() {
                    remove_clipping(child);
                }
            }
            serde_json::Value::Array(array) => {
                for child in array {
                    remove_clipping(child);
                }
            }
            _ => {}
        }
    }
    remove_clipping(&mut json);
    let bytes = archive(&[
        (
            "manifest.json",
            br#"{"format":"designer-dsg","schema_version":3}"#,
        ),
        ("document.json", &serde_json::to_vec(&json).unwrap()),
    ]);
    assert_eq!(dsg::from_bytes(&bytes).unwrap(), document);
    document.pages[0].nodes[0].kind = designer_core::NodeKind::Frame {
        clips_content: true,
    };
    assert_eq!(
        dsg::from_bytes(&dsg::to_bytes(&document).unwrap()).unwrap(),
        document
    );
}

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

struct OversizedReader;

impl Read for OversizedReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}

impl Seek for OversizedReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        Ok(match position {
            SeekFrom::End(_) => dsg::MAX_FILE_BYTES + 1,
            _ => 0,
        })
    }
}

#[test]
fn the_sample_document_round_trips_with_stable_identifiers() {
    let document = sample_document();
    let bytes = dsg::to_bytes(&document).expect("writes");
    assert_eq!(&bytes[..2], b"PK", "a .dsg file is a ZIP archive");
    assert_eq!(dsg::from_bytes(&bytes).expect("reads"), document);
}

#[test]
fn writer_marks_the_frame_clipping_schema_as_version_four() {
    let bytes = dsg::to_bytes(&sample_document()).expect("writes");
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("archive");
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .expect("manifest")
        .read_to_string(&mut manifest)
        .expect("manifest text");
    let manifest: serde_json::Value = serde_json::from_str(&manifest).expect("manifest json");
    assert_eq!(manifest["schema_version"], 4);
}

#[test]
fn non_empty_interactive_states_round_trip_without_losing_transition_data() {
    let mut document = sample_document();
    let page = &mut document.pages[0];
    let node = page.nodes[0].id;
    page.prototype.states.push(PrototypeState {
        id: PrototypeStateId(1),
        name: "Hover".to_owned(),
        overrides: vec![NodeOverride {
            node,
            bounds: Some(Rect::new(12.0, 15.0, 140.0, 42.0)),
            fill: Some(FillOverride::Clear),
            opacity: Some(0.75),
            visible: Some(false),
        }],
    });
    page.prototype.transitions.push(PrototypeTransition {
        node,
        from: INITIAL_STATE,
        trigger: PrototypeTrigger::Click,
        to: PrototypeStateId(1),
        duration_ms: DEFAULT_PROTOTYPE_DURATION_MS,
        easing: Easing::EaseOut,
    });

    let bytes = dsg::to_bytes(&document).expect("writes");
    assert_eq!(dsg::from_bytes(&bytes).expect("reads"), document);
}

#[test]
fn version_two_documents_without_prototype_fields_remain_readable() {
    let document = Document::empty("legacy v2");
    let mut legacy_json = serde_json::to_value(&document).expect("document json");
    let pages = legacy_json["pages"].as_array_mut().expect("pages");
    for page in pages {
        page.as_object_mut()
            .expect("page object")
            .remove("prototype");
    }
    let legacy_json = serde_json::to_vec(&legacy_json).expect("legacy json");
    let bytes = archive(&[
        (
            "manifest.json",
            br#"{"format":"designer-dsg","schema_version":2}"#,
        ),
        ("document.json", &legacy_json),
    ]);

    assert_eq!(
        dsg::from_bytes(&bytes).expect("reads version two"),
        document
    );
}

#[test]
fn version_one_documents_without_morph_fields_remain_readable() {
    let document = Document::empty("legacy");
    let mut legacy_json = serde_json::to_value(&document).expect("document json");
    let animation = legacy_json["pages"][0]["animation"]
        .as_object_mut()
        .expect("animation object");
    animation.remove("morph_tracks");
    let legacy_json = serde_json::to_vec(&legacy_json).expect("legacy json");
    let bytes = archive(&[
        ("manifest.json", GOOD_MANIFEST.as_bytes()),
        ("document.json", &legacy_json),
    ]);

    assert_eq!(
        dsg::from_bytes(&bytes).expect("reads version one"),
        document
    );
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
            br#"{"format":"designer-dsg","schema_version":5}"#,
        ),
        ("document.json", &good),
    ]);
    assert!(matches!(
        dsg::from_bytes(&future),
        Err(DsgError::UnsupportedVersion(5))
    ));
}

#[test]
fn readers_enforce_the_archive_size_limit_before_parsing() {
    assert!(matches!(
        dsg::from_reader(OversizedReader),
        Err(DsgError::TooLarge("the file"))
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
