//! Built-in sample document used by the application shell.
//!
//! It is original content, written for this project. Names are in Spanish
//! because the initial interface language is Spanish.

use crate::{
    Animation, Color, Document, Node, NodeId, NodeKind, Page, PageId, PathData, PathPoint,
    PathSegment, Rect,
};

const CANVAS: Color = Color::rgb(30, 31, 36);
const PAPER: Color = Color::rgb(242, 244, 248);
const INK: Color = Color::rgb(24, 25, 29);
const ACCENT: Color = Color::rgb(105, 87, 235);
const WARM: Color = Color::rgb(240, 138, 93);
const MINT: Color = Color::rgb(92, 200, 165);
const SOFT: Color = Color::rgb(222, 226, 235);
const GLOW: Color = Color {
    r: 105,
    g: 87,
    b: 235,
    a: 64,
};

struct Ids(u64);

impl Ids {
    fn next(&mut self) -> NodeId {
        self.0 += 1;
        NodeId(self.0)
    }
}

fn node(ids: &mut Ids, name: &str, kind: NodeKind, bounds: Rect, fill: Option<Color>) -> Node {
    Node {
        id: ids.next(),
        name: name.to_owned(),
        kind,
        bounds,
        fill,
        opacity: 1.0,
        visible: true,
        locked: false,
        children: Vec::new(),
    }
}

fn rounded(radius: f64) -> NodeKind {
    NodeKind::Rectangle {
        corner_radius: radius,
    }
}

fn text(content: &str) -> NodeKind {
    NodeKind::Text {
        content: content.to_owned(),
    }
}

/// A small two-page document that exercises every node kind, nesting, a
/// hidden layer, a locked layer, a translucent fill and a very long name.
pub fn sample_document() -> Document {
    let mut ids = Ids(0);

    let mut card = node(
        &mut ids,
        "Tarjeta",
        rounded(20.0),
        Rect::new(48.0, 150.0, 264.0, 300.0),
        Some(PAPER),
    );
    card.children = vec![
        node(
            &mut ids,
            "Imagen",
            rounded(12.0),
            Rect::new(16.0, 16.0, 232.0, 140.0),
            Some(SOFT),
        ),
        node(
            &mut ids,
            "Avatar",
            NodeKind::Ellipse,
            Rect::new(24.0, 124.0, 64.0, 64.0),
            Some(WARM),
        ),
        node(
            &mut ids,
            "Nombre",
            text("Lucía Ferrer"),
            Rect::new(24.0, 200.0, 216.0, 24.0),
            Some(INK),
        ),
        node(
            &mut ids,
            "Descripción de la tarjeta con un nombre de capa deliberadamente muy largo",
            text("Diseñadora de producto"),
            Rect::new(24.0, 230.0, 216.0, 20.0),
            Some(INK),
        ),
    ];

    let mut actions = node(
        &mut ids,
        "Acciones",
        NodeKind::Group,
        Rect::new(48.0, 474.0, 264.0, 48.0),
        None,
    );
    actions.children = vec![
        node(
            &mut ids,
            "Botón principal",
            rounded(24.0),
            Rect::new(0.0, 0.0, 168.0, 48.0),
            Some(ACCENT),
        ),
        node(
            &mut ids,
            "Botón secundario",
            NodeKind::Ellipse,
            Rect::new(216.0, 0.0, 48.0, 48.0),
            Some(MINT),
        ),
    ];

    let mut guide = node(
        &mut ids,
        "Guía oculta",
        rounded(0.0),
        Rect::new(0.0, 96.0, 360.0, 2.0),
        Some(WARM),
    );
    guide.visible = false;

    let mut header = node(
        &mut ids,
        "Cabecera",
        rounded(0.0),
        Rect::new(0.0, 0.0, 360.0, 96.0),
        Some(ACCENT),
    );
    header.locked = true;

    let mut screen = node(
        &mut ids,
        "Pantalla de perfil",
        NodeKind::Frame,
        Rect::new(120.0, 80.0, 360.0, 560.0),
        Some(Color::rgb(250, 250, 252)),
    );
    screen.children = vec![
        header,
        node(
            &mut ids,
            "Resplandor",
            NodeKind::Ellipse,
            Rect::new(180.0, -40.0, 240.0, 240.0),
            Some(GLOW),
        ),
        guide,
        card,
        actions,
    ];

    let mut palette = node(
        &mut ids,
        "Paleta",
        NodeKind::Frame,
        Rect::new(560.0, 80.0, 280.0, 200.0),
        Some(PAPER),
    );
    palette.children = [ACCENT, WARM, MINT, INK]
        .into_iter()
        .zip(["Acento", "Cálido", "Menta", "Tinta"])
        .enumerate()
        .map(|(index, (color, name))| {
            node(
                &mut ids,
                name,
                rounded(8.0),
                Rect::new(24.0 + index as f64 * 60.0, 24.0, 48.0, 48.0),
                Some(color),
            )
        })
        .collect();
    palette.children.push(node(
        &mut ids,
        "Trazado",
        NodeKind::Path {
            path: PathData {
                start: PathPoint { x: 0.5, y: 0.0 },
                segments: vec![
                    PathSegment::LineTo {
                        end: PathPoint { x: 1.0, y: 1.0 },
                    },
                    PathSegment::LineTo {
                        end: PathPoint { x: 0.0, y: 1.0 },
                    },
                    PathSegment::LineTo {
                        end: PathPoint { x: 0.5, y: 0.0 },
                    },
                ],
                closed: true,
            },
        },
        Rect::new(24.0, 100.0, 48.0, 48.0),
        Some(MINT),
    ));

    Document {
        name: "Documento de ejemplo".to_owned(),
        pages: vec![
            Page {
                id: PageId(1),
                name: "Perfil".to_owned(),
                background: CANVAS,
                nodes: vec![screen, palette],
                animation: Animation::default(),
            },
            Page {
                id: PageId(2),
                name: "Página vacía".to_owned(),
                background: CANVAS,
                nodes: Vec::new(),
                animation: Animation::default(),
            },
        ],
        next_node_id: ids.0 + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_valid_and_covers_the_shell_states() {
        let document = sample_document();
        assert_eq!(document.validate(), Ok(()));

        let page = &document.pages[0];
        let mut kinds = [false; 6];
        let (mut hidden, mut locked, mut long_name, mut translucent) = (false, false, false, false);
        fn walk(node: &Node, visit: &mut dyn FnMut(&Node)) {
            visit(node);
            node.children.iter().for_each(|child| walk(child, visit));
        }
        for top in &page.nodes {
            walk(top, &mut |node| {
                let index = match node.kind {
                    NodeKind::Frame => 0,
                    NodeKind::Group => 1,
                    NodeKind::Rectangle { .. } => 2,
                    NodeKind::Ellipse => 3,
                    NodeKind::Text { .. } => 4,
                    NodeKind::Path { .. } => 5,
                };
                kinds[index] = true;
                hidden |= !node.visible;
                locked |= node.locked;
                long_name |= node.name.chars().count() >= 60;
                translucent |= node.fill.is_some_and(|fill| fill.a < 255);
            });
        }
        assert!(kinds.iter().all(|seen| *seen), "every node kind appears");
        assert!(hidden && locked && long_name && translucent);
        assert!(document.pages.iter().any(|page| page.nodes.is_empty()));
    }
}
