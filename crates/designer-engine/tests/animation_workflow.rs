//! The editor animation workflow survives persistence and shares undo history.

use designer_core::{
    Document, Easing, NodeId, NodeKind, Operation, PageId, PathData, PathPoint, PathSegment,
    Property, Rect, SequenceId,
};
use designer_engine::{Session, dsg};

fn key(time_ms: u32, value: f64, easing: Easing) -> Operation {
    Operation::SetKeyframe {
        page: PageId(1),
        node: NodeId(1),
        property: Property::X,
        time_ms,
        value,
        easing,
    }
}

fn session() -> Session {
    let mut session = Session::new(Document::empty("Animation workflow"));
    session
        .apply(&[Operation::AddNode {
            page: PageId(1),
            parent: None,
            name: "Rectangle".into(),
            kind: NodeKind::Rectangle { corner_radius: 0.0 },
            bounds: Rect::new(10.0, 20.0, 100.0, 80.0),
            fill: None,
        }])
        .expect("shape");
    session
}

fn triangle(offset: f64) -> PathData {
    PathData {
        start: PathPoint {
            x: 0.5 + offset,
            y: 0.0,
        },
        segments: vec![
            PathSegment::LineTo {
                end: PathPoint { x: 1.0, y: 1.0 },
            },
            PathSegment::LineTo {
                end: PathPoint { x: 0.0, y: 1.0 },
            },
        ],
        closed: true,
    }
}

#[test]
fn author_preview_save_reopen_and_undo() {
    let mut session = session();
    session
        .apply(&[key(0, 10.0, Easing::Linear)])
        .expect("start");
    session
        .apply(&[key(1000, 210.0, Easing::Linear)])
        .expect("end");
    let before = session.document().clone();
    let revision = session.revision();
    for time in 0..=2000 {
        let evaluated = session.document().pages[0].at_time(time);
        let expected = 10.0 + 200.0 * f64::from(time.min(1000)) / 1000.0;
        assert!((evaluated.nodes[0].bounds.x - expected).abs() < 1e-9);
    }
    assert_eq!(session.document(), &before);
    assert_eq!(session.revision(), revision, "playback creates no history");
    let reopened =
        dsg::from_bytes(&dsg::to_bytes(session.document()).expect("save")).expect("open");
    assert_eq!(reopened, before);
    assert_eq!(reopened.pages[0].at_time(500).nodes[0].bounds.x, 110.0);
    assert!(session.undo());
    assert_eq!(
        session.document().pages[0].animation.tracks[0].keys.len(),
        1
    );
    assert!(session.redo());
    assert_eq!(session.document(), &reopened);
}

#[test]
fn replacing_easing_removing_keys_and_shortening_are_atomic() {
    let mut session = session();
    session
        .apply(&[
            key(0, 10.0, Easing::Linear),
            key(1000, 210.0, Easing::Linear),
        ])
        .expect("keys");
    session
        .apply(&[key(0, 30.0, Easing::Constant)])
        .expect("replace");
    assert_eq!(
        session.document().pages[0].at_time(500).nodes[0].bounds.x,
        30.0
    );
    let before = session.document().clone();
    let revision = session.revision();
    assert!(
        session
            .apply(&[Operation::SetDuration {
                page: PageId(1),
                duration_ms: 500
            }])
            .is_err()
    );
    assert_eq!(session.document(), &before);
    assert_eq!(session.revision(), revision);
    session
        .apply(&[Operation::RemoveKeyframe {
            page: PageId(1),
            node: NodeId(1),
            property: Property::X,
            time_ms: 1000,
        }])
        .expect("remove");
    session
        .apply(&[Operation::SetDuration {
            page: PageId(1),
            duration_ms: 500,
        }])
        .expect("shorten");
    assert_eq!(session.document().pages[0].animation.duration_ms, 500);
    assert!(session.undo() && session.undo());
    assert_eq!(session.document(), &before);
}

#[test]
fn easing_curves_survive_save_and_reopen() {
    let mut session = session();
    session
        .apply(&[
            key(0, 0.0, Easing::EaseIn),
            key(1000, 100.0, Easing::Linear),
        ])
        .expect("curved keys");

    let value = session.document().pages[0].at_time(500).nodes[0].bounds.x;
    assert!(value > 0.0 && value < 50.0);
    let reopened =
        dsg::from_bytes(&dsg::to_bytes(session.document()).expect("save")).expect("open");
    assert_eq!(reopened, *session.document());
    assert_eq!(reopened.pages[0].at_time(500).nodes[0].bounds.x, value);
}

#[test]
fn nested_sequences_hold_their_end_state_and_round_trip_with_history() {
    let mut session = session();
    session
        .apply(&[
            Operation::AddSequence {
                page: PageId(1),
                parent: None,
                sequence: SequenceId(1),
                name: "Parent".into(),
                offset_ms: 200,
                duration_ms: 600,
            },
            Operation::AddSequence {
                page: PageId(1),
                parent: Some(SequenceId(1)),
                sequence: SequenceId(2),
                name: "Child".into(),
                offset_ms: 100,
                duration_ms: 200,
            },
        ])
        .expect("sequences");
    session
        .apply(&[
            Operation::SetSequenceKeyframe {
                page: PageId(1),
                sequence: SequenceId(1),
                node: NodeId(1),
                property: Property::X,
                time_ms: 0,
                value: 0.0,
                easing: Easing::Linear,
            },
            Operation::SetSequenceKeyframe {
                page: PageId(1),
                sequence: SequenceId(1),
                node: NodeId(1),
                property: Property::X,
                time_ms: 400,
                value: 100.0,
                easing: Easing::Linear,
            },
            Operation::SetSequenceKeyframe {
                page: PageId(1),
                sequence: SequenceId(2),
                node: NodeId(1),
                property: Property::Y,
                time_ms: 0,
                value: 20.0,
                easing: Easing::Linear,
            },
            Operation::SetSequenceKeyframe {
                page: PageId(1),
                sequence: SequenceId(2),
                node: NodeId(1),
                property: Property::Y,
                time_ms: 200,
                value: 60.0,
                easing: Easing::Linear,
            },
        ])
        .expect("sequence keys");

    let page = &session.document().pages[0];
    assert_eq!(page.at_time(400).nodes[0].bounds.x, 50.0);
    assert_eq!(page.at_time(400).nodes[0].bounds.y, 40.0);
    assert_eq!(page.at_time(900).nodes[0].bounds.x, 100.0);
    assert_eq!(page.at_time(900).nodes[0].bounds.y, 60.0);
    let saved = dsg::to_bytes(session.document()).expect("save");
    let reopened = dsg::from_bytes(&saved).expect("open");
    assert_eq!(reopened, *session.document());

    assert!(session.undo());
    assert!(
        session.document().pages[0]
            .animation
            .sequence(SequenceId(1))
            .expect("parent sequence")
            .tracks
            .is_empty()
    );
    assert!(session.redo());
    assert_eq!(session.document(), &reopened);
}

#[test]
fn nested_path_morphs_round_trip_and_share_undo_history() {
    let mut session = Session::new(Document::empty("Path morph"));
    session
        .apply(&[Operation::AddNode {
            page: PageId(1),
            parent: None,
            name: "Triangle".into(),
            kind: NodeKind::Path {
                path: triangle(0.0),
            },
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            fill: None,
        }])
        .expect("path");
    session
        .apply(&[Operation::AddSequence {
            page: PageId(1),
            parent: None,
            sequence: SequenceId(1),
            name: "Morph".into(),
            offset_ms: 0,
            duration_ms: 1000,
        }])
        .expect("sequence");
    session
        .apply(&[
            Operation::SetSequenceMorphKeyframe {
                page: PageId(1),
                sequence: SequenceId(1),
                node: NodeId(1),
                time_ms: 0,
                path: triangle(0.0),
                easing: Easing::Linear,
            },
            Operation::SetSequenceMorphKeyframe {
                page: PageId(1),
                sequence: SequenceId(1),
                node: NodeId(1),
                time_ms: 1000,
                path: triangle(0.2),
                easing: Easing::Linear,
            },
        ])
        .expect("morph keys");

    let page = &session.document().pages[0];
    let NodeKind::Path { path } = &page.at_time(500).nodes[0].kind else {
        panic!("target remains a path");
    };
    assert!((path.start.x - 0.6).abs() < 1e-12);
    let reopened =
        dsg::from_bytes(&dsg::to_bytes(session.document()).expect("save")).expect("open");
    assert_eq!(reopened, *session.document());

    assert!(session.undo());
    assert!(
        session.document().pages[0]
            .animation
            .sequence(SequenceId(1))
            .expect("sequence")
            .morph_tracks
            .is_empty()
    );
    assert!(session.redo());
    assert_eq!(session.document(), &reopened);
}
