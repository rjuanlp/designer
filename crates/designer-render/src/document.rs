//! Turns a document page into a flat display list and a Vello scene.
//!
//! The display list is pure data, so paint order, visibility and transforms
//! can be tested without a GPU.

use designer_core::{Color, Node, NodeId, NodeKind, Page, PathData, PathSegment, Rect};
use vello::{
    Scene,
    kurbo::{Affine, BezPath, Ellipse, RoundedRect, Stroke},
    peniko::{self, Fill},
};

/// Visual constants of the canvas overlay, in target pixels.
pub mod overlay_tokens {
    /// Selection outline color, in sRGB.
    pub const SELECTION: [u8; 3] = [64, 156, 255];
    /// Selection outline width.
    pub const SELECTION_WIDTH: f64 = 2.0;
}

/// Geometry of one painted item.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle with one corner radius, in document units.
    Rectangle {
        /// Corner radius.
        corner_radius: f64,
    },
    /// An ellipse inscribed in the item bounds.
    Ellipse,
    /// A path in normalized item coordinates.
    Path(PathData),
}

/// One filled shape in page coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawItem {
    /// The node that produced the item.
    pub node: NodeId,
    /// Bounds in page coordinates.
    pub bounds: Rect,
    /// Geometry.
    pub shape: Shape,
    /// Fill color.
    pub fill: Color,
}

/// Maps page coordinates to target pixels: `pixel = point * scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    /// Target pixels per document unit. Always finite and positive.
    pub scale: f64,
    /// Horizontal offset in target pixels.
    pub offset_x: f64,
    /// Vertical offset in target pixels.
    pub offset_y: f64,
}

impl ViewTransform {
    /// The identity view.
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        offset_x: 0.0,
        offset_y: 0.0,
    };

    /// The page point shown at a target pixel.
    pub fn to_page(&self, pixel_x: f64, pixel_y: f64) -> (f64, f64) {
        (
            (pixel_x - self.offset_x) / self.scale,
            (pixel_y - self.offset_y) / self.scale,
        )
    }

    fn affine(&self) -> Affine {
        Affine::translate((self.offset_x, self.offset_y)) * Affine::scale(self.scale)
    }
}

fn shape_of(node: &Node) -> Option<Shape> {
    match &node.kind {
        NodeKind::Frame => Some(Shape::Rectangle { corner_radius: 0.0 }),
        NodeKind::Rectangle { corner_radius } => Some(Shape::Rectangle {
            corner_radius: *corner_radius,
        }),
        NodeKind::Ellipse => Some(Shape::Ellipse),
        NodeKind::Path { path } => Some(Shape::Path(path.clone())),
        // Groups paint nothing themselves. Text is not rendered yet.
        NodeKind::Group | NodeKind::Text { .. } => None,
    }
}

fn fill_with_opacity(mut fill: Color, opacity: f64) -> Option<Color> {
    fill.a = (f64::from(fill.a) * opacity).round() as u8;
    (fill.a != 0).then_some(fill)
}

fn collect(node: &Node, origin: (f64, f64), inherited_opacity: f64, items: &mut Vec<DrawItem>) {
    if !node.visible {
        return;
    }
    let opacity = inherited_opacity * node.opacity;
    if opacity == 0.0 {
        return;
    }
    let bounds = node.bounds.translated(origin.0, origin.1);
    if let (Some(shape), Some(fill)) = (
        shape_of(node),
        node.fill.and_then(|fill| fill_with_opacity(fill, opacity)),
    ) {
        items.push(DrawItem {
            node: node.id,
            bounds,
            shape,
            fill,
        });
    }
    for child in &node.children {
        collect(child, (bounds.x, bounds.y), opacity, items);
    }
}

/// Visible, filled shapes of a page in paint order (later items on top).
///
/// Hidden nodes hide their whole subtree. Child bounds are relative to the
/// parent's origin.
pub fn display_list(page: &Page) -> Vec<DrawItem> {
    let mut items = Vec::new();
    for node in &page.nodes {
        collect(node, (0.0, 0.0), 1.0, &mut items);
    }
    items
}

fn locate(node: &Node, origin: (f64, f64), id: NodeId) -> Option<Rect> {
    let bounds = node.bounds.translated(origin.0, origin.1);
    if node.id == id {
        return Some(bounds);
    }
    node.children
        .iter()
        .find_map(|child| locate(child, (bounds.x, bounds.y), id))
}

/// Bounds of a node in page coordinates, whether or not it is visible.
pub fn absolute_bounds(page: &Page, id: NodeId) -> Option<Rect> {
    page.nodes
        .iter()
        .find_map(|node| locate(node, (0.0, 0.0), id))
}

fn contains(shape: &Shape, bounds: Rect, x: f64, y: f64) -> bool {
    let inside = x >= bounds.x
        && y >= bounds.y
        && x <= bounds.x + bounds.width
        && y <= bounds.y + bounds.height;
    match shape {
        // Rounded corners are ignored: the miss area is a few units at most.
        Shape::Rectangle { .. } => inside,
        Shape::Ellipse => {
            let (radius_x, radius_y) = (bounds.width / 2.0, bounds.height / 2.0);
            if !inside || radius_x <= 0.0 || radius_y <= 0.0 {
                return false;
            }
            let dx = (x - bounds.x - radius_x) / radius_x;
            let dy = (y - bounds.y - radius_y) / radius_y;
            dx * dx + dy * dy <= 1.0
        }
        Shape::Path(path) => path_contains(path, bounds, x, y),
    }
}

fn path_contains(path: &PathData, bounds: Rect, x: f64, y: f64) -> bool {
    let mut points = vec![(
        bounds.x + path.start.x * bounds.width,
        bounds.y + path.start.y * bounds.height,
    )];
    let mut current = points[0];
    for segment in &path.segments {
        match *segment {
            PathSegment::LineTo { end } => {
                current = (
                    bounds.x + end.x * bounds.width,
                    bounds.y + end.y * bounds.height,
                );
                points.push(current);
            }
            PathSegment::CubicTo {
                control1,
                control2,
                end,
            } => {
                let control1 = (
                    bounds.x + control1.x * bounds.width,
                    bounds.y + control1.y * bounds.height,
                );
                let control2 = (
                    bounds.x + control2.x * bounds.width,
                    bounds.y + control2.y * bounds.height,
                );
                let end = (
                    bounds.x + end.x * bounds.width,
                    bounds.y + end.y * bounds.height,
                );
                let start = current;
                for step in 1..=16 {
                    let t = f64::from(step) / 16.0;
                    let inverse = 1.0 - t;
                    points.push((
                        inverse.powi(3) * start.0
                            + 3.0 * inverse.powi(2) * t * control1.0
                            + 3.0 * inverse * t.powi(2) * control2.0
                            + t.powi(3) * end.0,
                        inverse.powi(3) * start.1
                            + 3.0 * inverse.powi(2) * t * control1.1
                            + 3.0 * inverse * t.powi(2) * control2.1
                            + t.powi(3) * end.1,
                    ));
                }
                current = end;
            }
        }
    }
    let mut winding = 0_i32;
    for index in 0..points.len() {
        let (x1, y1) = points[index];
        let (x2, y2) = points[(index + 1) % points.len()];
        let is_left = (x2 - x1) * (y - y1) - (x - x1) * (y2 - y1);
        if y1 <= y {
            if y2 > y && is_left > 0.0 {
                winding += 1;
            }
        } else if y2 <= y && is_left < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

fn hit(node: &Node, origin: (f64, f64), inherited_opacity: f64, x: f64, y: f64) -> Option<NodeId> {
    if !node.visible || node.locked {
        return None;
    }
    let opacity = inherited_opacity * node.opacity;
    if opacity == 0.0 {
        return None;
    }
    let bounds = node.bounds.translated(origin.0, origin.1);
    // Children are painted over their parent, and later children on top.
    node.children
        .iter()
        .rev()
        .find_map(|child| hit(child, (bounds.x, bounds.y), opacity, x, y))
        .or_else(|| {
            let shape = shape_of(node)?;
            (node
                .fill
                .and_then(|fill| fill_with_opacity(fill, opacity))
                .is_some()
                && contains(&shape, bounds, x, y))
            .then_some(node.id)
        })
}

/// The topmost painted node under a page point.
///
/// Only what is drawn can be hit: hidden nodes, nodes without a fill, groups
/// and text are skipped. A locked node protects its whole subtree.
pub fn hit_test(page: &Page, x: f64, y: f64) -> Option<NodeId> {
    page.nodes
        .iter()
        .rev()
        .find_map(|node| hit(node, (0.0, 0.0), 1.0, x, y))
}

/// Smallest rectangle containing every top-level node, if the page has any.
pub fn content_bounds(page: &Page) -> Option<Rect> {
    let mut nodes = page.nodes.iter().map(|node| node.bounds);
    let first = nodes.next()?;
    let (mut left, mut top) = (first.x, first.y);
    let (mut right, mut bottom) = (first.x + first.width, first.y + first.height);
    for bounds in nodes {
        left = left.min(bounds.x);
        top = top.min(bounds.y);
        right = right.max(bounds.x + bounds.width);
        bottom = bottom.max(bounds.y + bounds.height);
    }
    Some(Rect::new(left, top, right - left, bottom - top))
}

/// A view that centers `content` in a target, leaving `margin` pixels free on
/// every side and never enlarging beyond 100%.
///
/// Returns the identity view when the content or the target has no area.
pub fn fit_view(
    content: Rect,
    target_width: f64,
    target_height: f64,
    margin: f64,
) -> ViewTransform {
    let available_width = target_width - 2.0 * margin;
    let available_height = target_height - 2.0 * margin;
    let usable = [
        content.width,
        content.height,
        available_width,
        available_height,
    ]
    .iter()
    .all(|value| value.is_finite() && *value > 0.0);
    if !usable {
        return ViewTransform::IDENTITY;
    }
    let scale = (available_width / content.width)
        .min(available_height / content.height)
        .min(1.0);
    ViewTransform {
        scale,
        offset_x: (target_width - content.width * scale) / 2.0 - content.x * scale,
        offset_y: (target_height - content.height * scale) / 2.0 - content.y * scale,
    }
}

fn color(color: Color) -> peniko::Color {
    peniko::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

/// Build the Vello scene of a page. The page background is not included; pass
/// it as the render base color. `selection` adds an outline around one node.
pub fn page_scene(page: &Page, view: ViewTransform, selection: Option<NodeId>) -> Scene {
    let mut scene = Scene::new();
    let transform = view.affine();
    for item in display_list(page) {
        let Rect {
            x,
            y,
            width,
            height,
        } = item.bounds;
        match item.shape {
            Shape::Rectangle { corner_radius } => scene.fill(
                Fill::NonZero,
                transform,
                color(item.fill),
                None,
                &RoundedRect::new(x, y, x + width, y + height, corner_radius),
            ),
            Shape::Ellipse => scene.fill(
                Fill::NonZero,
                transform,
                color(item.fill),
                None,
                &Ellipse::new(
                    (x + width / 2.0, y + height / 2.0),
                    (width / 2.0, height / 2.0),
                    0.0,
                ),
            ),
            Shape::Path(path) => scene.fill(
                Fill::NonZero,
                transform,
                color(item.fill),
                None,
                &vello_path(&path, item.bounds),
            ),
        }
    }

    fn vello_path(path: &PathData, bounds: Rect) -> BezPath {
        let point = |point: designer_core::PathPoint| {
            (
                bounds.x + point.x * bounds.width,
                bounds.y + point.y * bounds.height,
            )
        };
        let mut result = BezPath::new();
        result.move_to(point(path.start));
        for segment in &path.segments {
            match *segment {
                PathSegment::LineTo { end } => result.line_to(point(end)),
                PathSegment::CubicTo {
                    control1,
                    control2,
                    end,
                } => result.curve_to(point(control1), point(control2), point(end)),
            }
        }
        if path.closed {
            result.close_path();
        }
        result
    }
    if let Some(bounds) = selection.and_then(|id| absolute_bounds(page, id)) {
        // Drawn in target pixels so the outline keeps its width at any zoom.
        let [r, g, b] = overlay_tokens::SELECTION;
        let left = bounds.x * view.scale + view.offset_x;
        let top = bounds.y * view.scale + view.offset_y;
        scene.stroke(
            &Stroke::new(overlay_tokens::SELECTION_WIDTH),
            Affine::IDENTITY,
            peniko::Color::from_rgb8(r, g, b),
            None,
            &vello::kurbo::Rect::new(
                left,
                top,
                left + bounds.width * view.scale,
                top + bounds.height * view.scale,
            ),
        );
    }
    scene
}
