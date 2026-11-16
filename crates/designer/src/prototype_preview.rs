//! Deterministic state and trigger handling for the interactive preview.

use std::time::{Duration, Instant};

use designer_core::{
    Easing, NodeId, Page, PrototypeStateId, PrototypeTransition, PrototypeTrigger,
    interpolate_prototype_pages,
};

#[derive(Clone, Debug)]
struct ActiveTransition {
    from: Page,
    to: PrototypeStateId,
    started: Instant,
    duration: Duration,
    easing: Easing,
}

/// Runtime-only interaction state for one preview session.
#[derive(Clone, Debug)]
pub struct PrototypePreview {
    state: PrototypeStateId,
    hovered_node: Option<NodeId>,
    pressed_node: Option<NodeId>,
    transition: Option<ActiveTransition>,
}

impl PrototypePreview {
    /// Begin at the page's configured initial state.
    pub fn new(page: &Page) -> Self {
        Self {
            state: page.prototype.initial,
            hovered_node: None,
            pressed_node: None,
            transition: None,
        }
    }

    /// Current logical state; it changes as soon as a transition begins.
    pub fn state(&self) -> PrototypeStateId {
        self.state
    }

    /// Whether a transition still needs animation frames.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.transition.as_ref().is_some_and(|transition| {
            now.saturating_duration_since(transition.started) < transition.duration
        })
    }

    /// Whether a transition is available for the next render.
    pub fn has_transition(&self) -> bool {
        self.transition.is_some()
    }

    /// Clear a completed transition after its final frame is scheduled.
    pub fn finish_transition(&mut self, now: Instant) {
        if self.transition.as_ref().is_some_and(|transition| {
            now.saturating_duration_since(transition.started) >= transition.duration
        }) {
            self.transition = None;
        }
    }

    /// Renderable page at the supplied time.
    pub fn page_at(&self, base: &Page, now: Instant) -> Option<Page> {
        let Some(transition) = &self.transition else {
            return base.in_prototype_state(self.state);
        };
        let target = base.in_prototype_state(transition.to)?;
        let elapsed = now.saturating_duration_since(transition.started);
        let progress = elapsed.as_secs_f64() / transition.duration.as_secs_f64();
        interpolate_prototype_pages(&transition.from, &target, progress, transition.easing)
    }

    /// Select a state directly in the editor controls.
    pub fn select_state(&mut self, page: &Page, state: PrototypeStateId) -> bool {
        if page.prototype.state(state).is_none() {
            return false;
        }
        self.state = state;
        self.transition = None;
        true
    }

    /// Update hover state, firing leave before enter if the target changes.
    pub fn pointer_moved(&mut self, page: &Page, node: Option<NodeId>, now: Instant) -> bool {
        if self.hovered_node == node {
            return false;
        }
        let previous = std::mem::replace(&mut self.hovered_node, node);
        let mut changed = false;
        if let Some(previous) = previous {
            changed |= self.trigger(page, previous, PrototypeTrigger::HoverLeave, now);
        }
        if let Some(node) = node {
            changed |= self.trigger(page, node, PrototypeTrigger::Hover, now);
        }
        changed
    }

    /// Start tracking a pointer press until the matching release.
    pub fn pointer_down(&mut self, node: Option<NodeId>) {
        self.pressed_node = node;
    }

    /// Fire a click only when press and release target the same node.
    pub fn pointer_up(&mut self, page: &Page, node: Option<NodeId>, now: Instant) -> bool {
        let pressed = std::mem::take(&mut self.pressed_node);
        pressed
            .filter(|pressed| Some(*pressed) == node)
            .is_some_and(|pressed| self.trigger(page, pressed, PrototypeTrigger::Click, now))
    }

    fn trigger(
        &mut self,
        page: &Page,
        node: NodeId,
        trigger: PrototypeTrigger,
        now: Instant,
    ) -> bool {
        let Some(transition) = page
            .prototype
            .transition(node, self.state, trigger)
            .cloned()
        else {
            return false;
        };
        self.start_transition(page, transition, now)
    }

    fn start_transition(
        &mut self,
        page: &Page,
        transition: PrototypeTransition,
        now: Instant,
    ) -> bool {
        let Some(from) = self.page_at(page, now) else {
            return false;
        };
        self.state = transition.to;
        self.transition = Some(ActiveTransition {
            from,
            to: transition.to,
            started: now,
            duration: Duration::from_millis(u64::from(transition.duration_ms)),
            easing: transition.easing,
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use designer_core::{
        Animation, Color, FillOverride, INITIAL_STATE, Node, NodeId, NodeKind, NodeOverride,
        PageId, Prototype, PrototypeState, PrototypeStateId, PrototypeTransition, PrototypeTrigger,
        Rect,
    };

    use super::*;

    fn page() -> Page {
        let mut prototype = Prototype::default();
        prototype.states.push(PrototypeState {
            id: PrototypeStateId(1),
            name: "Hover".into(),
            overrides: vec![NodeOverride {
                node: NodeId(1),
                bounds: Some(Rect::new(0.0, 0.0, 20.0, 10.0)),
                fill: Some(FillOverride::Set(Color::rgb(120, 10, 10))),
                opacity: None,
                visible: None,
            }],
        });
        prototype.states.push(PrototypeState {
            id: PrototypeStateId(2),
            name: "Pressed".into(),
            overrides: vec![NodeOverride {
                node: NodeId(1),
                bounds: Some(Rect::new(0.0, 0.0, 30.0, 10.0)),
                fill: None,
                opacity: None,
                visible: None,
            }],
        });
        prototype.transitions = vec![
            PrototypeTransition {
                node: NodeId(1),
                from: INITIAL_STATE,
                trigger: PrototypeTrigger::Hover,
                to: PrototypeStateId(1),
                duration_ms: 200,
                easing: Easing::Linear,
            },
            PrototypeTransition {
                node: NodeId(1),
                from: PrototypeStateId(1),
                trigger: PrototypeTrigger::HoverLeave,
                to: INITIAL_STATE,
                duration_ms: 200,
                easing: Easing::Linear,
            },
            PrototypeTransition {
                node: NodeId(1),
                from: INITIAL_STATE,
                trigger: PrototypeTrigger::Click,
                to: PrototypeStateId(2),
                duration_ms: 100,
                easing: Easing::Linear,
            },
        ];
        Page {
            id: PageId(1),
            name: "test".into(),
            background: Color::rgb(0, 0, 0),
            nodes: vec![Node {
                id: NodeId(1),
                name: "target".into(),
                kind: NodeKind::Rectangle { corner_radius: 0.0 },
                bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
                fill: Some(Color::rgb(10, 10, 10)),
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
    fn hover_enters_and_leaves_the_target_state() {
        let page = page();
        let start = Instant::now();
        let mut preview = PrototypePreview::new(&page);
        assert!(preview.pointer_moved(&page, Some(NodeId(1)), start));
        assert_eq!(preview.state(), PrototypeStateId(1));
        assert!(preview.is_animating(start));
        assert_eq!(
            preview
                .page_at(&page, start + Duration::from_millis(100))
                .expect("transition")
                .nodes[0]
                .bounds
                .width,
            15.0
        );
        assert!(preview.pointer_moved(&page, None, start + Duration::from_millis(100)));
        assert_eq!(preview.state(), INITIAL_STATE);
        assert_eq!(
            preview
                .page_at(&page, start + Duration::from_millis(200))
                .expect("interrupted transition")
                .nodes[0]
                .bounds
                .width,
            12.5
        );
    }

    #[test]
    fn click_requires_matching_press_and_release_targets() {
        let page = page();
        let now = Instant::now();
        let mut preview = PrototypePreview::new(&page);
        preview.pointer_down(Some(NodeId(1)));
        assert!(!preview.pointer_up(&page, None, now));
        assert_eq!(preview.state(), INITIAL_STATE);
        preview.pointer_down(Some(NodeId(1)));
        assert!(preview.pointer_up(&page, Some(NodeId(1)), now));
        assert_eq!(preview.state(), PrototypeStateId(2));
    }
}
