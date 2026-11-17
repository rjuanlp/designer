//! Pure inspector values and transaction construction.
use designer_core::{
    Color, FillOverride, Node, NodeId, NodeKind, NodeOverride, Operation, Page, PrototypeStateId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Name,
    X,
    Y,
    Width,
    Height,
    Radius,
    Opacity,
    Fill,
    Visible,
    Locked,
    Clips,
}

impl Field {
    pub(crate) const ALL: [Self; 11] = [
        Self::Name,
        Self::X,
        Self::Y,
        Self::Width,
        Self::Height,
        Self::Radius,
        Self::Opacity,
        Self::Fill,
        Self::Visible,
        Self::Locked,
        Self::Clips,
    ];
    pub(crate) fn available(self, node: &Node) -> bool {
        match self {
            Self::Radius => matches!(node.kind, NodeKind::Rectangle { .. }),
            Self::Clips => matches!(node.kind, NodeKind::Frame { .. }),
            _ => true,
        }
    }
    pub(crate) fn value(self, node: &Node) -> String {
        match self {
            Self::Name => node.name.clone(),
            Self::X => number(node.bounds.x),
            Self::Y => number(node.bounds.y),
            Self::Width => number(node.bounds.width),
            Self::Height => number(node.bounds.height),
            Self::Radius => {
                if let NodeKind::Rectangle { corner_radius } = node.kind {
                    number(corner_radius)
                } else {
                    String::new()
                }
            }
            Self::Opacity => number(node.opacity * 100.0),
            Self::Fill => node.fill.map_or_else(String::new, |c| {
                if c.a == 255 {
                    c.hex()
                } else {
                    format!("{}{:02X}", c.hex(), c.a)
                }
            }),
            Self::Visible => node.visible.to_string(),
            Self::Locked => node.locked.to_string(),
            Self::Clips => matches!(
                node.kind,
                NodeKind::Frame {
                    clips_content: true
                }
            )
            .to_string(),
        }
    }
}

impl Field {
    /// Whether the field is an on/off toggle instead of a text field.
    pub(crate) fn is_toggle(self) -> bool {
        matches!(self, Self::Visible | Self::Locked | Self::Clips)
    }

    /// Whether a prototype state can override the field.
    pub(crate) fn overridable(self) -> bool {
        matches!(
            self,
            Self::X | Self::Y | Self::Width | Self::Height | Self::Fill | Self::Opacity
        )
    }
}

/// Fill given to a node when "add fill" is used on one without fill.
pub(crate) const DEFAULT_FILL: &str = "#C4C4C4";

/// A number with at most two decimals and no useless fraction, as shown in a
/// field.
pub(crate) fn number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    // Avoid showing "-0".
    format!("{}", if rounded == 0.0 { 0.0 } else { rounded })
}

pub(crate) fn operation(
    page: &Page,
    id: NodeId,
    field: Field,
    input: &str,
) -> Result<Operation, ()> {
    let n = page.node(id).ok_or(())?;
    if !field.available(n) || (n.locked && field != Field::Locked) {
        return Err(());
    }
    let input = input.trim();
    let number = || -> Result<f64, ()> {
        let v: f64 = input.replace(',', ".").parse().map_err(|_| ())?;
        if v.is_finite() { Ok(v) } else { Err(()) }
    };
    let boolean = || input.parse::<bool>().map_err(|_| ());
    let page = page.id;
    let node = id;
    Ok(match field {
        Field::Name => {
            if input.is_empty() || input.chars().count() > 256 {
                return Err(());
            }
            Operation::SetName {
                page,
                node,
                name: input.into(),
            }
        }
        Field::X | Field::Y | Field::Width | Field::Height => {
            let v = number()?;
            if matches!(field, Field::Width | Field::Height) && v < 0.0 {
                return Err(());
            }
            let mut bounds = n.bounds;
            match field {
                Field::X => bounds.x = v,
                Field::Y => bounds.y = v,
                Field::Width => bounds.width = v,
                _ => bounds.height = v,
            }
            Operation::SetBounds { page, node, bounds }
        }
        Field::Radius => {
            let corner_radius = number()?;
            if corner_radius < 0.0 {
                return Err(());
            }
            Operation::SetCornerRadius {
                page,
                node,
                corner_radius,
            }
        }
        Field::Opacity => {
            let opacity = number()? / 100.0;
            if !(0.0..=1.0).contains(&opacity) {
                return Err(());
            }
            Operation::SetOpacity {
                page,
                node,
                opacity,
            }
        }
        Field::Fill => {
            let fill = if input.is_empty() {
                None
            } else {
                // The leading `#` is optional when typing.
                let hex = input.strip_prefix('#').unwrap_or(input);
                if !matches!(hex.len(), 6 | 8) || !hex.is_ascii() {
                    return Err(());
                }
                let v = u32::from_str_radix(hex, 16).map_err(|_| ())?;
                let v = if hex.len() == 6 { (v << 8) | 255 } else { v };
                Some(Color {
                    r: (v >> 24) as u8,
                    g: (v >> 16) as u8,
                    b: (v >> 8) as u8,
                    a: v as u8,
                })
            };
            Operation::SetFill { page, node, fill }
        }
        Field::Visible => Operation::SetVisible {
            page,
            node,
            visible: boolean()?,
        },
        Field::Locked => Operation::SetLocked {
            page,
            node,
            locked: boolean()?,
        },
        Field::Clips => Operation::SetClipsContent {
            page,
            node,
            clips_content: boolean()?,
        },
    })
}

pub(crate) fn effective_node(
    page: &Page,
    id: NodeId,
    state: Option<PrototypeStateId>,
) -> Option<Node> {
    let mut node = page.node(id)?.clone();
    if let Some(o) = state
        .and_then(|s| page.prototype.state(s))
        .and_then(|s| s.overrides.iter().find(|o| o.node == id))
    {
        if let Some(bounds) = o.bounds {
            node.bounds = bounds;
        }
        if let Some(fill) = o.fill {
            node.fill = match fill {
                FillOverride::Clear => None,
                FillOverride::Set(c) => Some(c),
            };
        }
        if let Some(opacity) = o.opacity {
            node.opacity = opacity;
        }
    }
    Some(node)
}

pub(crate) fn edit_operation(
    page: &Page,
    id: NodeId,
    state: Option<PrototypeStateId>,
    field: Field,
    value: &str,
    reset: bool,
) -> Result<Operation, ()> {
    let effective = effective_node(page, id, state).ok_or(())?;
    let mut op = operation(page, id, field, value)?;
    if let Operation::SetBounds { bounds, .. } = &mut op {
        let parsed = *bounds;
        *bounds = effective.bounds;
        match field {
            Field::X => bounds.x = parsed.x,
            Field::Y => bounds.y = parsed.y,
            Field::Width => bounds.width = parsed.width,
            Field::Height => bounds.height = parsed.height,
            _ => {}
        }
    }
    state.map_or(Ok(op.clone()), |s| state_operation(page, s, op, reset))
}

/// Convert supported edits to one state transaction; reset restores inheritance.
pub(crate) fn state_operation(
    page: &Page,
    state: PrototypeStateId,
    edit: Operation,
    reset: bool,
) -> Result<Operation, ()> {
    let mut prototype = page.prototype.clone();
    let target = prototype
        .states
        .iter_mut()
        .find(|s| s.id == state)
        .ok_or(())?;
    let id = match edit {
        Operation::SetBounds { node, .. }
        | Operation::SetFill { node, .. }
        | Operation::SetOpacity { node, .. } => node,
        _ => return Ok(edit),
    };
    let index = target
        .overrides
        .iter()
        .position(|o| o.node == id)
        .unwrap_or_else(|| {
            target.overrides.push(NodeOverride {
                node: id,
                bounds: None,
                fill: None,
                opacity: None,
                visible: None,
            });
            target.overrides.len() - 1
        });
    let o = &mut target.overrides[index];
    match edit {
        Operation::SetBounds { bounds, .. } => o.bounds = (!reset).then_some(bounds),
        Operation::SetFill { fill, .. } => {
            o.fill = (!reset).then_some(fill.map_or(FillOverride::Clear, FillOverride::Set))
        }
        Operation::SetOpacity { opacity, .. } => o.opacity = (!reset).then_some(opacity),
        _ => unreachable!(),
    }
    target.overrides.retain(|o| {
        o.bounds.is_some() || o.fill.is_some() || o.opacity.is_some() || o.visible.is_some()
    });
    Ok(Operation::SetPrototype {
        page: page.id,
        prototype,
    })
}

/// Raising moves towards the end of paint order (the start of the layer list).
pub(crate) fn reorder(nodes: &[Node], id: NodeId, raise: bool) -> Option<usize> {
    if let Some(i) = nodes.iter().position(|n| n.id == id) {
        return if raise {
            (i + 1 < nodes.len()).then_some(i + 1)
        } else {
            i.checked_sub(1)
        };
    }
    nodes.iter().find_map(|n| reorder(&n.children, id, raise))
}

/// Locked ancestors protect their descendants as well.
pub(crate) fn editable(nodes: &[Node], id: NodeId) -> bool {
    nodes
        .iter()
        .any(|n| !n.locked && (n.id == id || editable(&n.children, id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use designer_core::sample_document;
    use designer_engine::Session;
    #[test]
    fn locks_protect_nodes_and_descendants_but_allow_unlock() {
        let mut doc = sample_document();
        let p = &mut doc.pages[0];
        let id = p.nodes[0].id;
        p.nodes[0].locked = true;
        assert!(!editable(&p.nodes, id));
        for field in Field::ALL {
            if field != Field::Locked {
                assert!(operation(p, id, field, "1").is_err());
            }
        }
        assert!(operation(p, id, Field::Locked, "false").is_ok());
        for child in &p.nodes[0].children {
            assert!(!editable(&p.nodes, child.id));
        }
    }
    #[test]
    fn fill_and_opacity_overrides_and_resets_are_atomic() {
        for (field, value) in [
            (Field::Fill, "#12345678"),
            (Field::Fill, ""),
            (Field::Opacity, "23,5"),
        ] {
            let doc = sample_document();
            let p = &doc.pages[0];
            let id = p.nodes[0].id;
            let edit = operation(p, id, field, value).unwrap();
            let op = state_operation(p, p.prototype.initial, edit.clone(), false).unwrap();
            let mut session = Session::new(doc.clone());
            session.apply(&[op]).unwrap();
            assert_eq!(session.document().pages[0].nodes, p.nodes);
            let reset = state_operation(
                &session.document().pages[0],
                p.prototype.initial,
                edit,
                true,
            )
            .unwrap();
            session.apply(&[reset]).unwrap();
            assert_eq!(session.document().pages[0].prototype, p.prototype);
            assert!(session.undo());
            assert!(session.undo());
            assert_eq!(session.document(), &doc);
            assert!(!session.undo());
        }
    }
    #[test]
    fn every_commit_is_one_undoable_transaction() {
        for field in Field::ALL {
            let mut doc = sample_document();
            let n = &mut doc.pages[0].nodes[0];
            n.locked = false;
            n.kind = if field == Field::Clips {
                NodeKind::Frame {
                    clips_content: true,
                }
            } else {
                NodeKind::Rectangle { corner_radius: 0.0 }
            };
            let id = n.id;
            let value = field.value(n);
            let op = operation(&doc.pages[0], id, field, &value).unwrap();
            let mut session = Session::new(doc.clone());
            session.apply(&[op]).unwrap();
            assert!(session.undo());
            assert_eq!(session.document(), &doc);
            assert!(!session.undo());
        }
    }
    #[test]
    fn numbers_drop_a_useless_fraction_and_keep_two_decimals() {
        assert_eq!(number(120.0), "120");
        assert_eq!(number(-0.5), "-0.5");
        assert_eq!(number(1.0 / 3.0), "0.33");
        assert_eq!(number(2.999), "3");
        assert_eq!(number(-0.001), "0");
    }
    #[test]
    fn opaque_fills_are_shown_without_alpha_and_parse_back() {
        let mut doc = sample_document();
        let node = &mut doc.pages[0].nodes[0];
        node.locked = false;
        node.fill = Some(Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(Field::Fill.value(node), "#123456");
        node.fill = Some(Color {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0x80,
        });
        assert_eq!(Field::Fill.value(node), "#12345680");
        let id = node.id;
        let shown = Field::Fill.value(node);
        let fill = doc.pages[0].nodes[0].fill;
        assert_eq!(
            operation(&doc.pages[0], id, Field::Fill, &shown),
            Ok(Operation::SetFill {
                page: doc.pages[0].id,
                node: id,
                fill,
            })
        );
        assert!(operation(&doc.pages[0], id, Field::Fill, DEFAULT_FILL).is_ok());
    }
    #[test]
    fn rejects_invalid_numbers_and_colors() {
        let doc = sample_document();
        let p = &doc.pages[0];
        let id = p.nodes[0].id;
        for f in [Field::X, Field::Width, Field::Opacity] {
            for v in ["NaN", "inf", "", "1,2.3"] {
                assert!(operation(p, id, f, v).is_err());
            }
        }
        assert!(operation(p, id, Field::Width, "-1").is_err());
        assert!(operation(p, id, Field::Opacity, "101").is_err());
        assert!(operation(p, id, Field::X, " -1,25 ").is_ok());
        for v in ["#123456", "#12345678", "123456", "3366ff80", ""] {
            assert!(operation(p, id, Field::Fill, v).is_ok());
        }
        for v in ["#xyzxyz", "12345", "#123", "##123456"] {
            assert!(operation(p, id, Field::Fill, v).is_err());
        }
    }
    #[test]
    fn overrides_preserve_base_and_reset_inheritance() {
        let doc = sample_document();
        let p = &doc.pages[0];
        let id = p.nodes[0].id;
        let edit = operation(p, id, Field::X, "23").unwrap();
        let op = state_operation(p, p.prototype.initial, edit.clone(), false).unwrap();
        let (changed, _) = doc.transact(&[op]).unwrap();
        assert_eq!(changed.pages[0].nodes, p.nodes);
        let reset = state_operation(&changed.pages[0], p.prototype.initial, edit, true).unwrap();
        let (reset, _) = changed.transact(&[reset]).unwrap();
        assert_eq!(reset.pages[0].prototype, p.prototype);
    }
    #[test]
    fn reversed_order_maps_to_paint_indexes() {
        let doc = sample_document();
        let nodes = &doc.pages[0].nodes;
        assert_eq!(reorder(nodes, nodes[0].id, false), None);
        assert_eq!(reorder(nodes, nodes[0].id, true), Some(1));
        assert_eq!(reorder(nodes, nodes.last().unwrap().id, true), None);
    }
}
