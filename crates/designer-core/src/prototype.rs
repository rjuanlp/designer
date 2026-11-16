//! Interactive prototype states and transitions.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    Color, Node, NodeId, Page, Rect,
    animation::{Easing, MAX_DURATION_MS},
};

/// Identifier of a prototype state, unique within a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrototypeStateId(pub u64);

/// The built-in starting state for a page.
pub const INITIAL_STATE: PrototypeStateId = PrototypeStateId(0);
/// Maximum states on one page.
pub const MAX_PROTOTYPE_STATES: usize = 256;
/// Maximum transitions on one page.
pub const MAX_PROTOTYPE_TRANSITIONS: usize = 4096;
/// Maximum node overrides in one state.
pub const MAX_NODE_OVERRIDES: usize = 10_000;
/// Default duration for transitions created by the interface.
pub const DEFAULT_PROTOTYPE_DURATION_MS: u32 = 200;

/// An event that can cause a prototype transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrototypeTrigger {
    /// The pointer enters the target node.
    Hover,
    /// The pointer leaves the target node.
    HoverLeave,
    /// The target node is clicked.
    Click,
}

/// How a state changes a node's fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", content = "color", rename_all = "snake_case")]
pub enum FillOverride {
    /// Replace the base fill.
    Set(Color),
    /// Remove the base fill.
    Clear,
}

/// State-specific changes to one node. Unspecified properties use the node's
/// base value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeOverride {
    /// Node receiving the overrides.
    pub node: NodeId,
    /// Replacement bounds relative to the parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
    /// Replacement fill or an instruction to remove it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<FillOverride>,
    /// Replacement opacity, from 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// Replacement visibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

/// A named prototype state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrototypeState {
    /// Identity within the page.
    pub id: PrototypeStateId,
    /// User-facing state name.
    pub name: String,
    /// Node changes for this state.
    #[serde(default)]
    pub overrides: Vec<NodeOverride>,
}

/// A transition fired by a node event in a particular state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrototypeTransition {
    /// Node that receives the event.
    pub node: NodeId,
    /// State in which the event is active.
    pub from: PrototypeStateId,
    /// Event that starts this transition.
    pub trigger: PrototypeTrigger,
    /// State entered when the event occurs.
    pub to: PrototypeStateId,
    /// Transition duration in milliseconds.
    pub duration_ms: u32,
    /// Interpolation curve.
    #[serde(default)]
    pub easing: Easing,
}

/// Interactive states and event transitions for one page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prototype {
    /// State shown before any interaction.
    pub initial: PrototypeStateId,
    /// States, including the built-in initial state.
    pub states: Vec<PrototypeState>,
    /// Event transitions.
    #[serde(default)]
    pub transitions: Vec<PrototypeTransition>,
}

impl Default for Prototype {
    fn default() -> Self {
        Self {
            initial: INITIAL_STATE,
            states: vec![PrototypeState {
                id: INITIAL_STATE,
                name: "Initial".to_owned(),
                overrides: Vec::new(),
            }],
            transitions: Vec::new(),
        }
    }
}

impl Prototype {
    /// Find a state by identity.
    pub fn state(&self, id: PrototypeStateId) -> Option<&PrototypeState> {
        self.states.iter().find(|state| state.id == id)
    }

    /// Find a transition matching a node, state and trigger.
    pub fn transition(
        &self,
        node: NodeId,
        from: PrototypeStateId,
        trigger: PrototypeTrigger,
    ) -> Option<&PrototypeTransition> {
        self.transitions.iter().find(|transition| {
            transition.node == node && transition.from == from && transition.trigger == trigger
        })
    }

    /// Validate state references and node overrides.
    pub fn validate(&self, node_exists: impl Fn(NodeId) -> bool) -> Result<(), String> {
        if self.states.is_empty() || self.state(self.initial).is_none() {
            return Err("the initial prototype state does not exist".to_owned());
        }
        if self.states.len() > MAX_PROTOTYPE_STATES
            || self.transitions.len() > MAX_PROTOTYPE_TRANSITIONS
        {
            return Err("the prototype has too many states or transitions".to_owned());
        }
        let mut state_ids = HashSet::new();
        let mut transitions = HashSet::new();
        for state in &self.states {
            if state.name.trim().is_empty() || state.name.chars().count() > 256 {
                return Err(format!(
                    "prototype state {} has an invalid name",
                    state.id.0
                ));
            }
            if !state_ids.insert(state.id) {
                return Err(format!("prototype state id {} is repeated", state.id.0));
            }
            if state.overrides.len() > MAX_NODE_OVERRIDES {
                return Err(format!(
                    "prototype state {} has too many overrides",
                    state.id.0
                ));
            }
            let mut overridden_nodes = HashSet::new();
            for change in &state.overrides {
                if !node_exists(change.node) {
                    return Err(format!(
                        "prototype state {} references missing node {}",
                        state.id.0, change.node.0
                    ));
                }
                if !overridden_nodes.insert(change.node) {
                    return Err(format!(
                        "prototype state {} overrides node {} more than once",
                        state.id.0, change.node.0
                    ));
                }
                if change.bounds.is_some_and(|bounds| !bounds.is_valid())
                    || change.opacity.is_some_and(|opacity| {
                        !opacity.is_finite() || !(0.0..=1.0).contains(&opacity)
                    })
                {
                    return Err(format!(
                        "prototype state {} has invalid geometry for node {}",
                        state.id.0, change.node.0
                    ));
                }
            }
        }
        let mut seen_state_names = HashSet::new();
        if self
            .states
            .iter()
            .any(|state| !seen_state_names.insert(state.name.trim().to_lowercase()))
        {
            return Err("prototype state names must be unique".to_owned());
        }
        for transition in &self.transitions {
            if !node_exists(transition.node) {
                return Err(format!(
                    "prototype transition references missing node {}",
                    transition.node.0
                ));
            }
            if self.state(transition.from).is_none() || self.state(transition.to).is_none() {
                return Err("prototype transition references a missing state".to_owned());
            }
            if transition.duration_ms == 0 || transition.duration_ms > MAX_DURATION_MS {
                return Err(format!(
                    "prototype transition duration must be 1 to {MAX_DURATION_MS} ms"
                ));
            }
            let key = (transition.node, transition.from, transition.trigger);
            if !transitions.insert(key) {
                return Err("prototype has duplicate node/state/trigger transitions".to_owned());
            }
        }
        Ok(())
    }
}

impl Page {
    /// A copy of this page with one state's overrides applied.
    pub fn in_prototype_state(&self, state: PrototypeStateId) -> Option<Self> {
        let state = self.prototype.state(state)?;
        let mut page = self.clone();
        for change in &state.overrides {
            let node = page.node_mut(change.node)?;
            apply_override(node, change);
        }
        Some(page)
    }

    /// A page interpolated between two states, with an eased progress from 0 to
    /// 1. Returns `None` for invalid progress or missing states/nodes.
    pub fn between_prototype_states(
        &self,
        from: PrototypeStateId,
        to: PrototypeStateId,
        progress: f64,
        easing: Easing,
    ) -> Option<Self> {
        if !progress.is_finite() {
            return None;
        }
        let from = self.in_prototype_state(from)?;
        let to = self.in_prototype_state(to)?;
        interpolate_prototype_pages(&from, &to, progress, easing)
    }
}

/// Interpolate between two snapshots of the same page. This supports
/// interrupting a running prototype transition from its current visual state.
pub fn interpolate_prototype_pages(
    from: &Page,
    to: &Page,
    progress: f64,
    easing: Easing,
) -> Option<Page> {
    if !progress.is_finite() || from.id != to.id {
        return None;
    }
    let progress = easing.progress(progress);
    let mut page = from.clone();
    if page.nodes.len() != to.nodes.len() {
        return None;
    }
    for (target, source) in page.nodes.iter_mut().zip(&to.nodes) {
        if !interpolate_node(target, source, progress) {
            return None;
        }
    }
    Some(page)
}

impl Prototype {
    /// Remove references to deleted nodes.
    pub(crate) fn remove_nodes(&mut self, removed: &[NodeId]) {
        for state in &mut self.states {
            state
                .overrides
                .retain(|change| !removed.contains(&change.node));
        }
        self.transitions
            .retain(|transition| !removed.contains(&transition.node));
    }
}

fn interpolate_node(target: &mut Node, source: &Node, progress: f64) -> bool {
    if target.id != source.id || target.children.len() != source.children.len() {
        return false;
    }
    target.bounds = interpolate_rect(target.bounds, source.bounds, progress);
    target.opacity = interpolate(target.opacity, source.opacity, progress);
    if target.fill != source.fill {
        target.fill = interpolate_fill(target.fill, source.fill, progress);
    }
    if progress >= 1.0 {
        target.visible = source.visible;
    }
    for (target_child, source_child) in target.children.iter_mut().zip(&source.children) {
        if !interpolate_node(target_child, source_child, progress) {
            return false;
        }
    }
    true
}

fn apply_override(node: &mut Node, change: &NodeOverride) {
    if let Some(bounds) = change.bounds {
        node.bounds = bounds;
    }
    match change.fill {
        Some(FillOverride::Set(fill)) => node.fill = Some(fill),
        Some(FillOverride::Clear) => node.fill = None,
        None => {}
    }
    if let Some(opacity) = change.opacity {
        node.opacity = opacity;
    }
    if let Some(visible) = change.visible {
        node.visible = visible;
    }
}

fn interpolate(from: f64, to: f64, progress: f64) -> f64 {
    if progress <= 0.0 {
        from
    } else if progress >= 1.0 {
        to
    } else {
        from * (1.0 - progress) + to * progress
    }
}

fn interpolate_rect(from: Rect, to: Rect, progress: f64) -> Rect {
    Rect::new(
        interpolate(from.x, to.x, progress),
        interpolate(from.y, to.y, progress),
        interpolate(from.width, to.width, progress),
        interpolate(from.height, to.height, progress),
    )
}

fn interpolate_fill(from: Option<Color>, to: Option<Color>, progress: f64) -> Option<Color> {
    fn channels(color: Option<Color>) -> [f64; 4] {
        color.map_or([0.0; 4], |color| {
            [
                f64::from(color.r),
                f64::from(color.g),
                f64::from(color.b),
                f64::from(color.a),
            ]
        })
    }
    fn channel(from: f64, to: f64, progress: f64) -> u8 {
        interpolate(from, to, progress).round() as u8
    }
    if progress <= 0.0 {
        return from;
    }
    if progress >= 1.0 {
        return to;
    }
    if from.is_none() && to.is_none() {
        return None;
    }
    let from = channels(from);
    let to = channels(to);
    Some(Color {
        r: channel(from[0], to[0], progress),
        g: channel(from[1], to[1], progress),
        b: channel(from[2], to[2], progress),
        a: channel(from[3], to[3], progress),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Animation, NodeKind, PageId};

    fn page() -> Page {
        let mut prototype = Prototype::default();
        prototype.states.push(PrototypeState {
            id: PrototypeStateId(1),
            name: "Hover".to_owned(),
            overrides: vec![NodeOverride {
                node: NodeId(1),
                bounds: Some(Rect::new(20.0, 10.0, 100.0, 80.0)),
                fill: Some(FillOverride::Set(Color::rgb(200, 100, 50))),
                opacity: Some(0.5),
                visible: None,
            }],
        });
        prototype.transitions.push(PrototypeTransition {
            node: NodeId(1),
            from: INITIAL_STATE,
            trigger: PrototypeTrigger::Hover,
            to: PrototypeStateId(1),
            duration_ms: 200,
            easing: Easing::EaseOut,
        });
        Page {
            id: PageId(1),
            name: "page".to_owned(),
            background: Color::rgb(0, 0, 0),
            nodes: vec![Node {
                id: NodeId(1),
                name: "button".to_owned(),
                kind: NodeKind::Rectangle { corner_radius: 0.0 },
                bounds: Rect::new(0.0, 0.0, 100.0, 80.0),
                fill: Some(Color::rgb(0, 0, 0)),
                opacity: 1.0,
                visible: true,
                locked: false,
                children: Vec::new(),
            }],
            animation: Animation::default(),
            prototype,
        }
    }

    #[test]
    fn applies_node_overrides_without_mutating_the_base_page() {
        let page = page();
        let hovered = page
            .in_prototype_state(PrototypeStateId(1))
            .expect("state exists");
        assert_eq!(hovered.nodes[0].bounds.x, 20.0);
        assert_eq!(hovered.nodes[0].fill, Some(Color::rgb(200, 100, 50)));
        assert_eq!(hovered.nodes[0].opacity, 0.5);
        assert_eq!(page.nodes[0].bounds.x, 0.0);
    }

    #[test]
    fn transitions_interpolate_properties_and_support_reversal() {
        let page = page();
        let halfway = page
            .between_prototype_states(INITIAL_STATE, PrototypeStateId(1), 0.5, Easing::Linear)
            .expect("states exist");
        assert_eq!(halfway.nodes[0].bounds.x, 10.0);
        assert_eq!(halfway.nodes[0].opacity, 0.75);
        assert_eq!(
            page.between_prototype_states(PrototypeStateId(1), INITIAL_STATE, 0.5, Easing::Linear)
                .expect("states exist")
                .nodes[0]
                .bounds
                .x,
            10.0
        );
    }

    #[test]
    fn interpolation_rejects_pages_with_different_tree_shapes() {
        let from = page();
        let mut to = from.clone();
        to.nodes.push(Node {
            id: NodeId(2),
            name: "extra".to_owned(),
            kind: NodeKind::Group,
            bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            fill: None,
            opacity: 1.0,
            visible: true,
            locked: false,
            children: Vec::new(),
        });
        assert!(interpolate_prototype_pages(&from, &to, 0.5, Easing::Linear).is_none());
    }

    #[test]
    fn prototype_validation_rejects_conflicting_or_missing_targets() {
        let page = page();
        page.prototype
            .validate(|node| page.node(node).is_some())
            .expect("valid prototype");
        let mut invalid = page.prototype.clone();
        invalid.transitions.push(invalid.transitions[0].clone());
        assert!(invalid.validate(|node| page.node(node).is_some()).is_err());
        let mut invalid = page.prototype.clone();
        invalid.transitions[0].node = NodeId(99);
        assert!(invalid.validate(|node| page.node(node).is_some()).is_err());
    }
}
