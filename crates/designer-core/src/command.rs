//! Typed operations and atomic transactions.
//!
//! Every change to a document, from the interface or from an agent, is a list
//! of [`Operation`] values applied by [`Document::transact`]. A transaction is
//! applied to a copy and validated as a whole, so it either succeeds in full
//! or leaves the document untouched.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    Color, Document, Easing, Keyframe, ModelError, Node, NodeId, NodeKind, PageId, Property, Rect,
};

/// Most operations one transaction may contain.
pub const MAX_OPERATIONS: usize = 1000;
/// Longest node name, in characters.
pub const MAX_NAME_CHARS: usize = 256;
/// Longest text content, in characters.
pub const MAX_TEXT_CHARS: usize = 100_000;

/// One change to a document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    /// Create a node as the last (topmost) child of `parent`, or of the page.
    AddNode {
        /// Target page.
        page: PageId,
        /// Parent node, or `None` for a top-level node.
        #[serde(default)]
        parent: Option<NodeId>,
        /// Layer name.
        name: String,
        /// What the node is.
        kind: NodeKind,
        /// Bounds relative to the parent's origin.
        bounds: Rect,
        /// Fill, if any.
        #[serde(default)]
        fill: Option<Color>,
    },
    /// Delete a node, its descendants and their animation tracks.
    DeleteNode {
        /// Target page.
        page: PageId,
        /// Node to delete.
        node: NodeId,
    },
    /// Replace the base bounds of a node.
    SetBounds {
        /// Target page.
        page: PageId,
        /// Target node.
        node: NodeId,
        /// New bounds relative to the parent's origin.
        bounds: Rect,
    },
    /// Rename a node.
    SetName {
        /// Target page.
        page: PageId,
        /// Target node.
        node: NodeId,
        /// New layer name.
        name: String,
    },
    /// Replace or remove the fill of a node.
    SetFill {
        /// Target page.
        page: PageId,
        /// Target node.
        node: NodeId,
        /// New fill, or `None` to remove it.
        fill: Option<Color>,
    },
    /// Replace the base opacity of a node.
    SetOpacity {
        /// Target page.
        page: PageId,
        /// Target node.
        node: NodeId,
        /// New opacity, 0 to 1.
        opacity: f64,
    },
    /// Add a keyframe, replacing one at the same time on the same track.
    SetKeyframe {
        /// Target page.
        page: PageId,
        /// Animated node.
        node: NodeId,
        /// Animated property.
        property: Property,
        /// Time from the start of the animation.
        time_ms: u32,
        /// Value at that time.
        value: f64,
        /// Interpolation towards the next keyframe.
        #[serde(default)]
        easing: Easing,
    },
    /// Remove a keyframe. Fails if there is none at that time.
    RemoveKeyframe {
        /// Target page.
        page: PageId,
        /// Animated node.
        node: NodeId,
        /// Animated property.
        property: Property,
        /// Time of the keyframe.
        time_ms: u32,
    },
    /// Change the length of the page animation.
    SetDuration {
        /// Target page.
        page: PageId,
        /// New duration. No keyframe may lie after it.
        duration_ms: u32,
    },
}

impl Operation {
    /// The `op` tag of every operation, as serialized.
    pub const NAMES: [&'static str; 9] = [
        "add_node",
        "delete_node",
        "set_bounds",
        "set_name",
        "set_fill",
        "set_opacity",
        "set_keyframe",
        "remove_keyframe",
        "set_duration",
    ];
}

/// Why a transaction was rejected. The document is unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandError {
    /// The transaction has no operations or more than [`MAX_OPERATIONS`].
    BadOperationCount(usize),
    /// An operation names a page that does not exist.
    UnknownPage(PageId),
    /// An operation names a node that is not on the page.
    UnknownNode(NodeId),
    /// There is no keyframe to remove at that time.
    UnknownKeyframe(NodeId, Property, u32),
    /// A name or text is too long.
    TextTooLong,
    /// No more node identifiers are available.
    OutOfIdentifiers,
    /// The resulting document would be invalid.
    Invalid(ModelError),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadOperationCount(count) => write!(
                f,
                "a transaction needs 1 to {MAX_OPERATIONS} operations, got {count}"
            ),
            Self::UnknownPage(id) => write!(f, "page {} does not exist", id.0),
            Self::UnknownNode(id) => write!(f, "node {} is not on the page", id.0),
            Self::UnknownKeyframe(node, property, time) => write!(
                f,
                "node {} has no {property:?} keyframe at {time} ms",
                node.0
            ),
            Self::TextTooLong => write!(f, "a name or text is too long"),
            Self::OutOfIdentifiers => write!(f, "no more node identifiers are available"),
            Self::Invalid(error) => write!(f, "{error}"),
        }
    }
}

impl Error for CommandError {}

fn collect_ids(node: &Node, ids: &mut Vec<NodeId>) {
    ids.push(node.id);
    node.children
        .iter()
        .for_each(|child| collect_ids(child, ids));
}

/// Remove a node from a list or from any descendant list.
fn remove(nodes: &mut Vec<Node>, id: NodeId) -> Option<Node> {
    if let Some(index) = nodes.iter().position(|node| node.id == id) {
        return Some(nodes.remove(index));
    }
    nodes
        .iter_mut()
        .find_map(|node| remove(&mut node.children, id))
}

fn check_text(name: &str, kind: Option<&NodeKind>) -> Result<(), CommandError> {
    let content_chars = match kind {
        Some(NodeKind::Text { content }) => content.chars().count(),
        _ => 0,
    };
    if name.chars().count() > MAX_NAME_CHARS || content_chars > MAX_TEXT_CHARS {
        return Err(CommandError::TextTooLong);
    }
    Ok(())
}

impl Document {
    fn apply(
        &mut self,
        operation: &Operation,
        affected: &mut Vec<NodeId>,
    ) -> Result<(), CommandError> {
        use CommandError::{UnknownNode, UnknownPage};
        match operation {
            Operation::AddNode {
                page,
                parent,
                name,
                kind,
                bounds,
                fill,
            } => {
                check_text(name, Some(kind))?;
                let id = NodeId(self.next_node_id);
                self.next_node_id = id.0.checked_add(1).ok_or(CommandError::OutOfIdentifiers)?;
                let page = self.page_mut(*page).ok_or(UnknownPage(*page))?;
                let siblings = match parent {
                    Some(parent) => {
                        &mut page.node_mut(*parent).ok_or(UnknownNode(*parent))?.children
                    }
                    None => &mut page.nodes,
                };
                siblings.push(Node {
                    id,
                    name: name.clone(),
                    kind: kind.clone(),
                    bounds: *bounds,
                    fill: *fill,
                    opacity: 1.0,
                    visible: true,
                    locked: false,
                    children: Vec::new(),
                });
                affected.push(id);
            }
            Operation::DeleteNode { page, node } => {
                let page = self.page_mut(*page).ok_or(UnknownPage(*page))?;
                let removed = remove(&mut page.nodes, *node).ok_or(UnknownNode(*node))?;
                let mut ids = Vec::new();
                collect_ids(&removed, &mut ids);
                page.animation.remove_nodes(&ids);
                affected.extend(ids);
            }
            Operation::SetBounds { page, node, bounds } => {
                self.node_mut(*page, *node)?.bounds = *bounds;
                affected.push(*node);
            }
            Operation::SetName { page, node, name } => {
                check_text(name, None)?;
                self.node_mut(*page, *node)?.name = name.clone();
                affected.push(*node);
            }
            Operation::SetFill { page, node, fill } => {
                self.node_mut(*page, *node)?.fill = *fill;
                affected.push(*node);
            }
            Operation::SetOpacity {
                page,
                node,
                opacity,
            } => {
                self.node_mut(*page, *node)?.opacity = *opacity;
                affected.push(*node);
            }
            Operation::SetKeyframe {
                page,
                node,
                property,
                time_ms,
                value,
                easing,
            } => {
                self.node_mut(*page, *node)?;
                let page = self.page_mut(*page).ok_or(UnknownPage(*page))?;
                page.animation.set_key(
                    *node,
                    *property,
                    Keyframe {
                        time_ms: *time_ms,
                        value: *value,
                        easing: *easing,
                    },
                );
                affected.push(*node);
            }
            Operation::RemoveKeyframe {
                page,
                node,
                property,
                time_ms,
            } => {
                let page = self.page_mut(*page).ok_or(UnknownPage(*page))?;
                if !page.animation.remove_key(*node, *property, *time_ms) {
                    return Err(CommandError::UnknownKeyframe(*node, *property, *time_ms));
                }
                affected.push(*node);
            }
            Operation::SetDuration { page, duration_ms } => {
                let page = self.page_mut(*page).ok_or(UnknownPage(*page))?;
                page.animation.duration_ms = *duration_ms;
            }
        }
        Ok(())
    }

    fn node_mut(&mut self, page: PageId, node: NodeId) -> Result<&mut Node, CommandError> {
        self.page_mut(page)
            .ok_or(CommandError::UnknownPage(page))?
            .node_mut(node)
            .ok_or(CommandError::UnknownNode(node))
    }

    /// Apply operations in order as one atomic transaction.
    ///
    /// Returns the resulting document and the identifiers of the nodes that
    /// were created, changed or deleted, without duplicates. `self` is never
    /// modified; on error nothing is produced.
    pub fn transact(
        &self,
        operations: &[Operation],
    ) -> Result<(Document, Vec<NodeId>), CommandError> {
        if operations.is_empty() || operations.len() > MAX_OPERATIONS {
            return Err(CommandError::BadOperationCount(operations.len()));
        }
        let mut next = self.clone();
        let mut affected = Vec::new();
        for operation in operations {
            next.apply(operation, &mut affected)?;
        }
        next.validate().map_err(CommandError::Invalid)?;
        let mut seen = std::collections::HashSet::new();
        affected.retain(|id| seen.insert(*id));
        Ok((next, affected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Animation;

    const PAGE: PageId = PageId(1);

    fn add(parent: Option<NodeId>, x: f64) -> Operation {
        Operation::AddNode {
            page: PAGE,
            parent,
            name: "n".into(),
            kind: NodeKind::Ellipse,
            bounds: Rect::new(x, 0.0, 10.0, 10.0),
            fill: Some(Color::rgb(1, 2, 3)),
        }
    }

    fn key(node: u64, time_ms: u32, value: f64) -> Operation {
        Operation::SetKeyframe {
            page: PAGE,
            node: NodeId(node),
            property: Property::X,
            time_ms,
            value,
            easing: Easing::Linear,
        }
    }

    #[test]
    fn adds_nested_nodes_with_fresh_identifiers() {
        let empty = Document::empty("t");
        let (document, affected) = empty
            .transact(&[add(None, 0.0), add(Some(NodeId(1)), 5.0)])
            .expect("valid");
        assert_eq!(affected, [NodeId(1), NodeId(2)]);
        assert_eq!(document.next_node_id, 3);
        let page = &document.pages[0];
        assert_eq!(page.nodes.len(), 1);
        assert_eq!(page.nodes[0].children[0].bounds.x, 5.0);
        // The source document is untouched.
        assert_eq!(empty, Document::empty("t"));
    }

    #[test]
    fn a_failing_operation_rejects_the_whole_transaction() {
        let empty = Document::empty("t");
        let cases: Vec<(Vec<Operation>, CommandError)> = vec![
            (vec![], CommandError::BadOperationCount(0)),
            (
                vec![add(None, 0.0), add(Some(NodeId(9)), 0.0)],
                CommandError::UnknownNode(NodeId(9)),
            ),
            (
                vec![
                    add(None, 0.0),
                    Operation::DeleteNode {
                        page: PageId(7),
                        node: NodeId(1),
                    },
                ],
                CommandError::UnknownPage(PageId(7)),
            ),
            (
                vec![add(None, f64::NAN)],
                CommandError::Invalid(ModelError::InvalidGeometry(NodeId(1))),
            ),
            (
                vec![
                    add(None, 0.0),
                    Operation::SetOpacity {
                        page: PAGE,
                        node: NodeId(1),
                        opacity: 2.0,
                    },
                ],
                CommandError::Invalid(ModelError::InvalidGeometry(NodeId(1))),
            ),
            (
                vec![
                    add(None, 0.0),
                    Operation::RemoveKeyframe {
                        page: PAGE,
                        node: NodeId(1),
                        property: Property::X,
                        time_ms: 0,
                    },
                ],
                CommandError::UnknownKeyframe(NodeId(1), Property::X, 0),
            ),
            (
                vec![Operation::SetName {
                    page: PAGE,
                    node: NodeId(1),
                    name: "x".repeat(MAX_NAME_CHARS + 1),
                }],
                CommandError::TextTooLong,
            ),
            (vec![key(1, 0, 1.0)], CommandError::UnknownNode(NodeId(1))),
        ];
        for (operations, expected) in cases {
            assert_eq!(empty.transact(&operations), Err(expected));
        }
        let too_many = vec![add(None, 0.0); MAX_OPERATIONS + 1];
        assert!(empty.transact(&too_many).is_err());
    }

    #[test]
    fn keyframes_must_fit_the_duration_and_follow_their_node() {
        let (document, _) = Document::empty("t")
            .transact(&[add(None, 0.0), add(Some(NodeId(1)), 0.0), key(2, 500, 9.0)])
            .expect("valid");
        assert!(matches!(
            document.transact(&[key(2, 2001, 1.0)]),
            Err(CommandError::Invalid(ModelError::InvalidAnimation(..)))
        ));
        assert!(matches!(
            document.transact(&[Operation::SetDuration {
                page: PAGE,
                duration_ms: 499
            }]),
            Err(CommandError::Invalid(ModelError::InvalidAnimation(..)))
        ));
        let (longer, _) = document
            .transact(&[
                Operation::SetDuration {
                    page: PAGE,
                    duration_ms: 3000,
                },
                key(2, 3000, 1.0),
            ])
            .expect("valid");
        assert_eq!(longer.pages[0].animation.tracks[0].keys.len(), 2);

        // Deleting the parent removes the child and its track.
        let (deleted, affected) = longer
            .transact(&[Operation::DeleteNode {
                page: PAGE,
                node: NodeId(1),
            }])
            .expect("valid");
        assert_eq!(affected, [NodeId(1), NodeId(2)]);
        assert!(deleted.pages[0].nodes.is_empty());
        assert_eq!(
            deleted.pages[0].animation.tracks,
            Animation::default().tracks
        );
        // Identifiers are never reused.
        assert_eq!(deleted.next_node_id, 3);
    }

    #[test]
    fn operations_round_trip_through_json_with_stable_names() {
        let operations = vec![
            add(None, 1.0),
            Operation::DeleteNode {
                page: PAGE,
                node: NodeId(1),
            },
            Operation::SetBounds {
                page: PAGE,
                node: NodeId(1),
                bounds: Rect::new(1.0, 2.0, 3.0, 4.0),
            },
            Operation::SetName {
                page: PAGE,
                node: NodeId(1),
                name: "a".into(),
            },
            Operation::SetFill {
                page: PAGE,
                node: NodeId(1),
                fill: None,
            },
            Operation::SetOpacity {
                page: PAGE,
                node: NodeId(1),
                opacity: 0.5,
            },
            key(1, 0, 1.0),
            Operation::RemoveKeyframe {
                page: PAGE,
                node: NodeId(1),
                property: Property::Opacity,
                time_ms: 0,
            },
            Operation::SetDuration {
                page: PAGE,
                duration_ms: 10,
            },
        ];
        let mut names = Vec::new();
        for operation in &operations {
            let json = serde_json::to_value(operation).expect("serializes");
            names.push(json["op"].as_str().expect("tag").to_owned());
            let back: Operation = serde_json::from_value(json).expect("deserializes");
            assert_eq!(&back, operation);
        }
        assert_eq!(names, Operation::NAMES);
        assert!(serde_json::from_str::<Operation>(r#"{"op":"run_script"}"#).is_err());
    }
}
