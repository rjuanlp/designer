//! Editor operation atomicity and history contracts.
use designer_core::{Document, NodeId, NodeKind, Operation, PageId, Rect};
use designer_engine::{
    Session,
    bridge::{SessionRequest, execute_session_request},
};

fn session() -> Session {
    let mut session = Session::new(Document::empty("test"));
    for kind in [
        NodeKind::Frame {
            clips_content: false,
        },
        NodeKind::Rectangle { corner_radius: 0.0 },
        NodeKind::Ellipse,
    ] {
        session
            .apply(&[Operation::AddNode {
                page: PageId(1),
                parent: None,
                name: "test".into(),
                kind,
                bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
                fill: None,
            }])
            .unwrap();
    }
    session
}

#[test]
fn editor_operations_apply_undo_redo_and_are_advertised() {
    for operation in [
        Operation::SetCornerRadius {
            page: PageId(1),
            node: NodeId(2),
            corner_radius: 12.0,
        },
        Operation::SetVisible {
            page: PageId(1),
            node: NodeId(2),
            visible: false,
        },
        Operation::SetLocked {
            page: PageId(1),
            node: NodeId(2),
            locked: true,
        },
        Operation::ReorderNode {
            page: PageId(1),
            node: NodeId(1),
            index: usize::MAX,
        },
        Operation::SetClipsContent {
            page: PageId(1),
            node: NodeId(1),
            clips_content: true,
        },
    ] {
        let mut session = session();
        let before = session.document().clone();
        let capabilities =
            execute_session_request(&mut session, SessionRequest::DescribeCapabilities).unwrap();
        let serialized = serde_json::to_value(&operation).unwrap();
        assert!(
            capabilities["operations"]
                .as_array()
                .unwrap()
                .contains(&serialized["op"])
        );
        session.apply(std::slice::from_ref(&operation)).unwrap();
        let after = session.document().clone();
        assert_ne!(before, after);
        assert!(session.undo());
        assert_eq!(session.document(), &before);
        assert!(session.redo());
        assert_eq!(session.document(), &after);
        for field in ["page", "node"] {
            let mut bad = serialized.clone();
            bad[field] = 999.into();
            let bad: Operation = serde_json::from_value(bad).unwrap();
            let revision = session.revision();
            assert!(session.apply(&[operation.clone(), bad]).is_err());
            assert_eq!(session.document(), &after);
            assert_eq!(session.revision(), revision);
        }
    }
}

#[test]
fn invalid_radius_values_and_wrong_node_kinds_are_rejected() {
    let mut session = session();
    let before = session.document().clone();
    for radius in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(
            session
                .apply(&[Operation::SetCornerRadius {
                    page: PageId(1),
                    node: NodeId(2),
                    corner_radius: radius
                }])
                .is_err()
        );
    }
    assert!(
        session
            .apply(&[Operation::SetCornerRadius {
                page: PageId(1),
                node: NodeId(1),
                corner_radius: 1.0
            }])
            .is_err()
    );
    assert!(
        session
            .apply(&[Operation::SetClipsContent {
                page: PageId(1),
                node: NodeId(2),
                clips_content: true
            }])
            .is_err()
    );
    assert_eq!(session.document(), &before);
}

#[test]
fn reordering_is_clamped_and_never_reparents() {
    let mut session = session();
    session
        .apply(&[Operation::AddNode {
            page: PageId(1),
            parent: Some(NodeId(1)),
            name: "child".into(),
            kind: NodeKind::Ellipse,
            bounds: Rect::new(0.0, 0.0, 1.0, 1.0),
            fill: None,
        }])
        .unwrap();
    session
        .apply(&[Operation::ReorderNode {
            page: PageId(1),
            node: NodeId(4),
            index: usize::MAX,
        }])
        .unwrap();
    assert_eq!(
        session.document().pages[0].nodes[0].children[0].id,
        NodeId(4)
    );
    session
        .apply(&[Operation::ReorderNode {
            page: PageId(1),
            node: NodeId(1),
            index: usize::MAX,
        }])
        .unwrap();
    assert_eq!(session.document().pages[0].nodes[2].id, NodeId(1));
}
