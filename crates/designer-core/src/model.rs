//! Typed document tree.

use std::{collections::HashSet, error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{animation::Animation, prototype::Prototype};

/// Maximum number of segments accepted in one path.
pub const MAX_PATH_SEGMENTS: usize = 512;

/// Identifier of a node, unique within a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

/// Identifier of a page, unique within a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageId(pub u64);

/// An sRGB color with straight (non-premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    /// Red, 0 to 255.
    pub r: u8,
    /// Green, 0 to 255.
    pub g: u8,
    /// Blue, 0 to 255.
    pub b: u8,
    /// Opacity, 0 (transparent) to 255 (opaque).
    pub a: u8,
}

impl Color {
    /// An opaque color from its sRGB components.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Uppercase `#RRGGBB` notation, ignoring alpha.
    pub fn hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Opacity as a whole percentage.
    pub fn opacity_percent(&self) -> u8 {
        ((u16::from(self.a) * 100 + 127) / 255) as u8
    }
}

/// An axis-aligned rectangle in document units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width, never negative in a valid document.
    pub width: f64,
    /// Height, never negative in a valid document.
    pub height: f64,
}

impl Rect {
    /// A rectangle from its origin and size.
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The same rectangle moved by an offset.
    pub fn translated(&self, dx: f64, dy: f64) -> Self {
        Self::new(self.x + dx, self.y + dy, self.width, self.height)
    }

    pub(crate) fn is_valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
            && self.width >= 0.0
            && self.height >= 0.0
            && (self.x + self.width).is_finite()
            && (self.y + self.height).is_finite()
    }
}

/// A point in a path, normalized to its node's bounds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathPoint {
    /// Horizontal coordinate from 0 to 1.
    pub x: f64,
    /// Vertical coordinate from 0 to 1.
    pub y: f64,
}

impl PathPoint {
    fn is_valid(self) -> bool {
        (0.0..=1.0).contains(&self.x) && (0.0..=1.0).contains(&self.y)
    }

    fn interpolate(self, other: Self, progress: f64) -> Self {
        Self {
            x: self.x + (other.x - self.x) * progress,
            y: self.y + (other.y - self.y) * progress,
        }
    }
}

/// One line or cubic segment following the path's starting point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "segment", rename_all = "snake_case")]
pub enum PathSegment {
    /// Straight line to a point.
    LineTo {
        /// End point.
        end: PathPoint,
    },
    /// Cubic Bézier curve.
    CubicTo {
        /// First control point.
        control1: PathPoint,
        /// Second control point.
        control2: PathPoint,
        /// End point.
        end: PathPoint,
    },
}

impl PathSegment {
    fn is_valid(self) -> bool {
        match self {
            Self::LineTo { end } => end.is_valid(),
            Self::CubicTo {
                control1,
                control2,
                end,
            } => control1.is_valid() && control2.is_valid() && end.is_valid(),
        }
    }

    fn interpolate(self, other: Self, progress: f64) -> Option<Self> {
        match (self, other) {
            (Self::LineTo { end: from }, Self::LineTo { end: to }) => Some(Self::LineTo {
                end: from.interpolate(to, progress),
            }),
            (
                Self::CubicTo {
                    control1: from1,
                    control2: from2,
                    end: from_end,
                },
                Self::CubicTo {
                    control1: to1,
                    control2: to2,
                    end: to_end,
                },
            ) => Some(Self::CubicTo {
                control1: from1.interpolate(to1, progress),
                control2: from2.interpolate(to2, progress),
                end: from_end.interpolate(to_end, progress),
            }),
            _ => None,
        }
    }
}

/// A path in normalized node coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathData {
    /// Starting point.
    pub start: PathPoint,
    /// Segments in paint order.
    pub segments: Vec<PathSegment>,
    /// Close the final segment to the starting point.
    pub closed: bool,
}

impl PathData {
    /// Whether this path has valid normalized coordinates and bounded size.
    pub fn is_valid(&self) -> bool {
        self.start.is_valid()
            && !self.segments.is_empty()
            && self.segments.len() <= MAX_PATH_SEGMENTS
            && self.segments.iter().all(|segment| segment.is_valid())
    }

    /// Whether another path has the same segment types and closure.
    pub fn has_same_topology(&self, other: &Self) -> bool {
        self.closed == other.closed
            && self.segments.len() == other.segments.len()
            && self.segments.iter().zip(&other.segments).all(|(from, to)| {
                matches!(
                    (from, to),
                    (PathSegment::LineTo { .. }, PathSegment::LineTo { .. })
                        | (PathSegment::CubicTo { .. }, PathSegment::CubicTo { .. })
                )
            })
    }

    /// Interpolate between matching paths.
    pub fn interpolate(&self, other: &Self, progress: f64) -> Option<Self> {
        if !progress.is_finite()
            || !self.is_valid()
            || !other.is_valid()
            || !self.has_same_topology(other)
        {
            return None;
        }
        let progress = progress.clamp(0.0, 1.0);
        Some(Self {
            start: self.start.interpolate(other.start, progress),
            segments: self
                .segments
                .iter()
                .copied()
                .zip(other.segments.iter().copied())
                .map(|(from, to)| from.interpolate(to, progress))
                .collect::<Option<Vec<_>>>()?,
            closed: self.closed,
        })
    }
}

/// What a node is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    /// A container that clips nothing yet and may have a fill.
    Frame,
    /// A container without its own appearance.
    Group,
    /// A rectangle with one radius for all corners.
    Rectangle {
        /// Corner radius in document units.
        corner_radius: f64,
    },
    /// An ellipse inscribed in the node bounds.
    Ellipse,
    /// A filled vector path in normalized node coordinates.
    Path {
        /// Path geometry.
        path: PathData,
    },
    /// A text layer. Text is stored but not rendered yet.
    Text {
        /// The text content.
        content: String,
    },
}

/// A node in the layer tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Identity within the document.
    pub id: NodeId,
    /// Layer name shown to the user.
    pub name: String,
    /// What the node is.
    pub kind: NodeKind,
    /// Bounds relative to the parent's origin.
    pub bounds: Rect,
    /// Fill, if the node paints one.
    pub fill: Option<Color>,
    /// Opacity of the node and its descendants, 0 to 1.
    pub opacity: f64,
    /// Hidden nodes and their descendants are not drawn.
    pub visible: bool,
    /// Locked nodes will not be editable from the canvas.
    pub locked: bool,
    /// Children, painted in order (later children on top).
    pub children: Vec<Node>,
}

/// A page with its own canvas background and top-level nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    /// Identity within the document.
    pub id: PageId,
    /// Page name shown to the user.
    pub name: String,
    /// Canvas color behind all nodes.
    pub background: Color,
    /// Top-level nodes, painted in order.
    pub nodes: Vec<Node>,
    /// Keyframe animation of this page.
    pub animation: Animation,
    /// Interactive states and triggers for this page.
    #[serde(default)]
    pub prototype: Prototype,
}

/// A design document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Document name shown to the user.
    pub name: String,
    /// Pages in display order.
    pub pages: Vec<Page>,
    /// Identifier the next created node will receive. Always greater than
    /// every node identifier in use.
    pub next_node_id: u64,
}

/// A structural problem found by [`Document::validate`].
#[derive(Clone, Debug, PartialEq)]
pub enum ModelError {
    /// The document has no pages.
    NoPages,
    /// Two pages share an identifier.
    DuplicatePage(PageId),
    /// Two nodes share an identifier.
    DuplicateNode(NodeId),
    /// A node has invalid geometry or path data, or an opacity outside 0 to 1.
    InvalidGeometry(NodeId),
    /// `next_node_id` is not greater than an identifier in use.
    StaleNextNodeId,
    /// The animation of a page is inconsistent.
    InvalidAnimation(PageId, String),
    /// The prototype of a page is inconsistent.
    InvalidPrototype(PageId, String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPages => write!(f, "the document has no pages"),
            Self::DuplicatePage(id) => write!(f, "page id {} is used more than once", id.0),
            Self::DuplicateNode(id) => write!(f, "node id {} is used more than once", id.0),
            Self::InvalidGeometry(id) => write!(f, "node {} has invalid geometry", id.0),
            Self::StaleNextNodeId => write!(f, "next_node_id is already in use"),
            Self::InvalidAnimation(id, reason) => {
                write!(f, "page {} has an invalid animation: {reason}", id.0)
            }
            Self::InvalidPrototype(id, reason) => {
                write!(f, "page {} has an invalid prototype: {reason}", id.0)
            }
        }
    }
}

impl Error for ModelError {}

impl Node {
    /// Number of nodes in this subtree, including this node.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Node::count).sum::<usize>()
    }

    fn find(&self, id: NodeId) -> Option<&Node> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(id))
    }

    fn find_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }

    fn validate(
        &self,
        seen: &mut HashSet<NodeId>,
        parent_origin: (f64, f64),
    ) -> Result<(), ModelError> {
        if !seen.insert(self.id) {
            return Err(ModelError::DuplicateNode(self.id));
        }
        let radius_valid = match self.kind {
            NodeKind::Rectangle { corner_radius } => {
                corner_radius.is_finite() && corner_radius >= 0.0
            }
            _ => true,
        };
        let path_valid = match &self.kind {
            NodeKind::Path { path } => path.is_valid(),
            _ => true,
        };
        let opacity_valid = self.opacity.is_finite() && (0.0..=1.0).contains(&self.opacity);
        let absolute_bounds = self.bounds.translated(parent_origin.0, parent_origin.1);
        if !self.bounds.is_valid()
            || !absolute_bounds.is_valid()
            || !radius_valid
            || !path_valid
            || !opacity_valid
        {
            return Err(ModelError::InvalidGeometry(self.id));
        }
        self.children
            .iter()
            .try_for_each(|child| child.validate(seen, (absolute_bounds.x, absolute_bounds.y)))
    }
}

impl Page {
    /// Find a node anywhere in the page.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find_map(|node| node.find(id))
    }

    /// Find a node anywhere in the page, for modification.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find_map(|node| node.find_mut(id))
    }

    /// Number of nodes in the page.
    pub fn node_count(&self) -> usize {
        self.nodes.iter().map(Node::count).sum()
    }
}

impl Document {
    /// A document with one empty page.
    pub fn empty(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            pages: vec![Page {
                id: PageId(1),
                name: "1".to_owned(),
                background: Color::rgb(30, 31, 36),
                nodes: Vec::new(),
                animation: Animation::default(),
                prototype: Prototype::default(),
            }],
            next_node_id: 1,
        }
    }

    /// Find a page by identifier.
    pub fn page(&self, id: PageId) -> Option<&Page> {
        self.pages.iter().find(|page| page.id == id)
    }

    /// Find a page by identifier, for modification.
    pub fn page_mut(&mut self, id: PageId) -> Option<&mut Page> {
        self.pages.iter_mut().find(|page| page.id == id)
    }

    /// Check identifiers and geometry across the whole document.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.pages.is_empty() {
            return Err(ModelError::NoPages);
        }
        let mut pages = HashSet::new();
        let mut nodes = HashSet::new();
        for page in &self.pages {
            if !pages.insert(page.id) {
                return Err(ModelError::DuplicatePage(page.id));
            }
            let before = nodes.clone();
            page.nodes
                .iter()
                .try_for_each(|node| node.validate(&mut nodes, (0.0, 0.0)))?;
            let on_page = |id: NodeId| nodes.contains(&id) && !before.contains(&id);
            let path_of = |id: NodeId| match &page.node(id)?.kind {
                NodeKind::Path { path } => Some(path.clone()),
                _ => None,
            };
            page.animation
                .validate(on_page, path_of)
                .map_err(|reason| ModelError::InvalidAnimation(page.id, reason))?;
            page.prototype
                .validate(on_page)
                .map_err(|reason| ModelError::InvalidPrototype(page.id, reason))?;
            for state in &page.prototype.states {
                let state_page = page
                    .in_prototype_state(state.id)
                    .expect("validated state exists");
                let mut state_nodes = HashSet::new();
                state_page
                    .nodes
                    .iter()
                    .try_for_each(|node| node.validate(&mut state_nodes, (0.0, 0.0)))?;
            }
        }
        if nodes.iter().any(|id| id.0 >= self.next_node_id) {
            return Err(ModelError::StaleNextNodeId);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u64, bounds: Rect, children: Vec<Node>) -> Node {
        Node {
            id: NodeId(id),
            name: format!("n{id}"),
            kind: NodeKind::Group,
            bounds,
            fill: None,
            opacity: 1.0,
            visible: true,
            locked: false,
            children,
        }
    }

    fn document(nodes: Vec<Node>) -> Document {
        Document {
            name: "t".into(),
            pages: vec![Page {
                id: PageId(1),
                name: "p".into(),
                background: Color::rgb(0, 0, 0),
                nodes,
                animation: Animation::default(),
                prototype: Prototype::default(),
            }],
            next_node_id: 100,
        }
    }

    const UNIT: Rect = Rect::new(0.0, 0.0, 1.0, 1.0);

    #[test]
    fn rejects_structural_problems() {
        let empty = Document {
            name: "t".into(),
            pages: vec![],
            next_node_id: 1,
        };
        assert_eq!(empty.validate(), Err(ModelError::NoPages));

        let nested_duplicate = document(vec![node(1, UNIT, vec![node(1, UNIT, vec![])])]);
        assert_eq!(
            nested_duplicate.validate(),
            Err(ModelError::DuplicateNode(NodeId(1)))
        );

        for bad in [
            Rect::new(f64::NAN, 0.0, 1.0, 1.0),
            Rect::new(0.0, f64::INFINITY, 1.0, 1.0),
            Rect::new(0.0, 0.0, -1.0, 1.0),
            Rect::new(f64::MAX, 0.0, f64::MAX, 1.0),
        ] {
            assert_eq!(
                document(vec![node(7, bad, vec![])]).validate(),
                Err(ModelError::InvalidGeometry(NodeId(7)))
            );
        }
        let overflowed_child = node(
            1,
            Rect::new(1.0e308, 0.0, 0.0, 0.0),
            vec![node(2, Rect::new(1.0e308, 0.0, 0.0, 0.0), vec![])],
        );
        assert_eq!(
            document(vec![overflowed_child]).validate(),
            Err(ModelError::InvalidGeometry(NodeId(2)))
        );

        let mut stale = document(vec![node(7, UNIT, vec![])]);
        stale.next_node_id = 7;
        assert_eq!(stale.validate(), Err(ModelError::StaleNextNodeId));

        let mut faded = node(8, UNIT, vec![]);
        faded.opacity = 1.5;
        assert_eq!(
            document(vec![faded]).validate(),
            Err(ModelError::InvalidGeometry(NodeId(8)))
        );
        assert_eq!(Document::empty("n").validate(), Ok(()));

        let mut two_pages = document(vec![]);
        two_pages.pages.push(two_pages.pages[0].clone());
        assert_eq!(
            two_pages.validate(),
            Err(ModelError::DuplicatePage(PageId(1)))
        );
    }

    #[test]
    fn rejects_negative_corner_radius() {
        let mut bad = node(3, UNIT, vec![]);
        bad.kind = NodeKind::Rectangle {
            corner_radius: -1.0,
        };
        assert_eq!(
            document(vec![bad]).validate(),
            Err(ModelError::InvalidGeometry(NodeId(3)))
        );
    }

    #[test]
    fn finds_nested_nodes_and_counts_them() {
        let doc = document(vec![
            node(1, UNIT, vec![node(2, UNIT, vec![])]),
            node(3, UNIT, vec![]),
        ]);
        let page = &doc.pages[0];
        assert_eq!(page.node_count(), 3);
        assert_eq!(page.node(NodeId(2)).map(|n| n.name.as_str()), Some("n2"));
        assert!(page.node(NodeId(9)).is_none());
        assert!(doc.page(PageId(1)).is_some() && doc.page(PageId(2)).is_none());
    }

    #[test]
    fn color_notation() {
        let color = Color {
            r: 105,
            g: 87,
            b: 235,
            a: 128,
        };
        assert_eq!(color.hex(), "#6957EB");
        assert_eq!(color.opacity_percent(), 50);
        assert_eq!(Color::rgb(0, 0, 0).opacity_percent(), 100);
    }
}
