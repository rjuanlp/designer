//! Display-list contract tests. They need no GPU.
use designer_core::{
    Animation, Color, Node, NodeId, NodeKind, Page, PageId, Rect, sample_document,
};
use designer_render::document::{
    Shape, ViewTransform, absolute_bounds, content_bounds, display_list, fit_view, page_scene,
};

fn node(id: u64, kind: NodeKind, bounds: Rect, children: Vec<Node>) -> Node {
    Node {
        id: NodeId(id),
        name: String::new(),
        kind,
        bounds,
        fill: Some(Color::rgb(1, 2, 3)),
        opacity: 1.0,
        visible: true,
        locked: false,
        children,
    }
}

fn page(nodes: Vec<Node>) -> Page {
    Page {
        id: PageId(1),
        name: String::new(),
        background: Color::rgb(0, 0, 0),
        nodes,
        animation: Animation::default(),
    }
}

#[test]
fn children_are_offset_by_every_ancestor_and_painted_after_them() {
    let leaf = node(3, NodeKind::Ellipse, Rect::new(1.0, 2.0, 4.0, 4.0), vec![]);
    let group = node(
        2,
        NodeKind::Group,
        Rect::new(10.0, 20.0, 50.0, 50.0),
        vec![leaf],
    );
    let frame = node(
        1,
        NodeKind::Frame,
        Rect::new(100.0, 200.0, 300.0, 300.0),
        vec![group],
    );
    let page = page(vec![frame]);

    let items = display_list(&page);
    // The group paints nothing itself but still offsets its child.
    assert_eq!(
        items.iter().map(|item| item.node.0).collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(items[1].bounds, Rect::new(111.0, 222.0, 4.0, 4.0));
    assert_eq!(items[1].shape, Shape::Ellipse);
    assert_eq!(
        absolute_bounds(&page, NodeId(2)),
        Some(Rect::new(110.0, 220.0, 50.0, 50.0))
    );
    assert_eq!(absolute_bounds(&page, NodeId(9)), None);
}

#[test]
fn hidden_nodes_hide_their_subtree_and_unfilled_or_text_nodes_paint_nothing() {
    let child = node(2, NodeKind::Ellipse, Rect::new(0.0, 0.0, 1.0, 1.0), vec![]);
    let mut hidden = node(
        1,
        NodeKind::Frame,
        Rect::new(0.0, 0.0, 9.0, 9.0),
        vec![child],
    );
    hidden.visible = false;
    let mut unfilled = node(3, NodeKind::Ellipse, Rect::new(0.0, 0.0, 1.0, 1.0), vec![]);
    unfilled.fill = None;
    let text = node(
        4,
        NodeKind::Text {
            content: "hola".into(),
        },
        Rect::new(0.0, 0.0, 1.0, 1.0),
        vec![],
    );
    let page = page(vec![hidden, unfilled, text]);

    assert!(display_list(&page).is_empty());
    // A hidden node can still be located, for example to outline a selection.
    assert!(absolute_bounds(&page, NodeId(2)).is_some());
    let _ = page_scene(&page, ViewTransform::IDENTITY, Some(NodeId(2)));
}

#[test]
fn fit_view_centers_content_without_enlarging_it() {
    let content = Rect::new(100.0, 50.0, 400.0, 200.0);

    let shrunk = fit_view(content, 220.0, 400.0, 10.0);
    assert!((shrunk.scale - 0.5).abs() < 1e-12);
    // Content left edge lands on the margin; vertically it is centered.
    assert!((content.x * shrunk.scale + shrunk.offset_x - 10.0).abs() < 1e-9);
    let top = content.y * shrunk.scale + shrunk.offset_y;
    assert!((top - (400.0 - 100.0) / 2.0).abs() < 1e-9);

    let roomy = fit_view(content, 2000.0, 2000.0, 10.0);
    assert_eq!(roomy.scale, 1.0);

    for (width, height) in [(0.0, 100.0), (15.0, 100.0), (f64::NAN, 100.0)] {
        assert_eq!(
            fit_view(content, width, height, 10.0),
            ViewTransform::IDENTITY
        );
    }
    let empty = Rect::new(0.0, 0.0, 0.0, 10.0);
    assert_eq!(fit_view(empty, 100.0, 100.0, 0.0), ViewTransform::IDENTITY);
}

#[test]
fn content_bounds_cover_all_top_level_nodes() {
    let a = node(
        1,
        NodeKind::Frame,
        Rect::new(10.0, 20.0, 30.0, 40.0),
        vec![],
    );
    let b = node(
        2,
        NodeKind::Frame,
        Rect::new(-5.0, 50.0, 10.0, 100.0),
        vec![],
    );
    assert_eq!(
        content_bounds(&page(vec![a, b])),
        Some(Rect::new(-5.0, 20.0, 45.0, 130.0))
    );
    assert_eq!(content_bounds(&page(vec![])), None);
}

#[test]
fn sample_document_builds_a_scene_on_every_page() {
    let document = sample_document();
    for page in &document.pages {
        let items = display_list(page);
        assert_eq!(items.is_empty(), page.nodes.is_empty());
        let view = content_bounds(page)
            .map(|bounds| fit_view(bounds, 800.0, 600.0, 24.0))
            .unwrap_or(ViewTransform::IDENTITY);
        assert!(view.scale.is_finite() && view.scale > 0.0);
        let _ = page_scene(page, view, items.first().map(|item| item.node));
    }
}
