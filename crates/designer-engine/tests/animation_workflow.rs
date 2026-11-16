//! The editor animation workflow survives persistence and shares undo history.

use designer_core::{Document, Easing, NodeId, NodeKind, Operation, PageId, Property, Rect};
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
