//! Pure screen-reader tree construction. Control rectangles are intentionally omitted.
use crate::strings::{self, NarrowPanel, Screen, Tool};
use accesskit::{
    Action as A11yAction, Node as A11yNode, NodeId as A11yNodeId, Rect as A11yRect,
    Role as A11yRole, TreeId, TreeInfo, TreeUpdate,
};
use designer_core::{Node, NodeId, Page};
pub(crate) const A11Y_NAV_GROUP: u64 = 100;
pub(crate) const A11Y_CONTENT_GROUP: u64 = 101;
pub(crate) const A11Y_STATUS_NODE: u64 = 102;
pub(crate) const A11Y_LAYER_LIST: u64 = 103;
pub(crate) const A11Y_TOOLBAR_GROUP: u64 = 104;
pub(crate) const A11Y_NAV_BASE: u64 = 1_000;
pub(crate) const A11Y_TOOL_BASE: u64 = 2_000;
pub(crate) const A11Y_LAYER_BASE: u64 = 1 << 32;
/// Stable note identity.
pub(crate) const A11Y_NOTE: u64 = 105;

/// Stable time identity.
pub(crate) const A11Y_TIME: u64 = 120;

/// Stable canvas identity.
pub(crate) const A11Y_CANVAS: u64 = 121;

/// Stable panels identity.
pub(crate) const A11Y_PANELS: u64 = 140;

/// Stable states identity.
pub(crate) const A11Y_STATES: u64 = 150;

/// Stable import summary identity.
pub(crate) const A11Y_IMPORT_SUMMARY: u64 = 160;

/// Stable undo identity.
pub(crate) const A11Y_UNDO: u64 = 3001;

/// Stable redo identity.
pub(crate) const A11Y_REDO: u64 = 3002;

/// Stable delete identity.
pub(crate) const A11Y_DELETE: u64 = 3003;

/// Stable new identity.
pub(crate) const A11Y_NEW: u64 = 3010;

/// Stable open identity.
pub(crate) const A11Y_OPEN: u64 = 3011;

/// Stable save as identity.
pub(crate) const A11Y_SAVE_AS: u64 = 3012;

/// Stable sample identity.
pub(crate) const A11Y_SAMPLE: u64 = 3013;

/// Stable timeline identity.
pub(crate) const A11Y_TIMELINE: u64 = 3020;

/// Stable play identity.
pub(crate) const A11Y_PLAY: u64 = 3021;

/// Stable hover identity.
pub(crate) const A11Y_HOVER: u64 = 3030;

/// Stable click identity.
pub(crate) const A11Y_CLICK: u64 = 3031;

/// Stable remove state identity.
pub(crate) const A11Y_REMOVE_STATE: u64 = 3032;

/// Stable import pick identity.
pub(crate) const A11Y_IMPORT_PICK: u64 = 3040;

/// Stable import apply identity.
pub(crate) const A11Y_IMPORT_APPLY: u64 = 3041;

/// Stable theme identity.
pub(crate) const A11Y_THEME: u64 = 3050;

/// Stable panel base identity.
pub(crate) const A11Y_PANEL_BASE: u64 = 3100;

/// Stable panel last identity.
pub(crate) const A11Y_PANEL_LAST: u64 = 3102;

/// Stable property base identity.
pub(crate) const A11Y_PROPERTY_BASE: u64 = 4000;

/// Stable property extra identity.
pub(crate) const A11Y_PROPERTY_EXTRA: u64 = 4020;

/// Prototype identity range.
pub(crate) const A11Y_STATE_BASE: u64 = 1u64 << 63;

/// The interface state the screen-reader tree is derived from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct A11ySnapshot<'a> {
    pub(crate) screen: Screen,
    pub(crate) tool: Tool,
    pub(crate) selection: Option<NodeId>,
    pub(crate) page: Option<&'a Page>,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) status: Option<&'a str>,
    pub(crate) title: String,
    pub(crate) bounds: A11yRect,
    pub(crate) focus: A11yNodeId,
    pub(crate) dark: bool,
    pub(crate) timeline: bool,
    pub(crate) playing: bool,
    pub(crate) time_ms: u32,
    pub(crate) can_play: bool,
    pub(crate) narrow: bool,
    pub(crate) panel: NarrowPanel,
    pub(crate) prototype_state: designer_core::PrototypeStateId,
    pub(crate) can_import: bool,
    pub(crate) import_summary: String,
}

pub(crate) fn build_accessibility_tree(snapshot: A11ySnapshot<'_>) -> TreeUpdate {
    let A11ySnapshot {
        screen,
        tool,
        selection,
        page,
        can_undo,
        can_redo,
        status,
        title,
        bounds,
        focus,
        dark,
        timeline,
        playing,
        time_ms,
        can_play,
        narrow,
        panel,
        prototype_state,
        can_import,
        import_summary,
    } = snapshot;
    let mut nodes = Vec::new();
    let root_children = vec![A11yNodeId(A11Y_NAV_GROUP), A11yNodeId(A11Y_CONTENT_GROUP)];

    let mut navigation = A11yNode::new(A11yRole::TabList);
    let mut navigation_children = Vec::new();
    for (index, item) in Screen::ALL.into_iter().enumerate() {
        let id = A11Y_NAV_BASE + index as u64;
        let mut node = A11yNode::new(A11yRole::Tab);
        node.set_label(item.label());
        // Unselected tabs report `false` so they still expose a selection pattern.
        node.set_selected(screen == item);
        node.add_action(A11yAction::Click);
        node.add_action(A11yAction::Focus);
        navigation_children.push(A11yNodeId(id));
        nodes.push((A11yNodeId(id), node));
    }
    navigation.set_label(strings::SECTIONS);
    navigation.set_children(navigation_children);
    nodes.push((A11yNodeId(A11Y_NAV_GROUP), navigation));

    let mut content = A11yNode::new(A11yRole::Group);
    content.set_label(screen.label());
    let mut content_children = Vec::new();
    if screen == Screen::Design {
        let toolbar_id = A11yNodeId(A11Y_TOOLBAR_GROUP);
        let mut toolbar = A11yNode::new(A11yRole::Toolbar);
        let mut toolbar_children = Vec::new();
        for (index, item) in Tool::ALL.into_iter().enumerate() {
            let id = A11Y_TOOL_BASE + index as u64;
            let mut node = A11yNode::new(A11yRole::Button);
            node.set_label(item.label());
            if tool == item {
                node.set_toggled(accesskit::Toggled::True);
            } else {
                node.set_toggled(accesskit::Toggled::False);
            }
            node.add_action(A11yAction::Click);
            node.add_action(A11yAction::Focus);
            toolbar_children.push(A11yNodeId(id));
            nodes.push((A11yNodeId(id), node));
        }

        for (offset, label, enabled) in [
            (A11Y_UNDO, strings::UNDO, can_undo),
            (A11Y_REDO, strings::REDO, can_redo),
            (A11Y_DELETE, strings::DELETE_SELECTION, selection.is_some()),
        ] {
            let id = offset;
            let mut node = A11yNode::new(A11yRole::Button);
            node.set_label(label);
            if enabled {
                node.add_action(A11yAction::Click);
                node.add_action(A11yAction::Focus);
            } else {
                node.set_disabled();
            }
            toolbar_children.push(A11yNodeId(id));
            nodes.push((A11yNodeId(id), node));
        }
        toolbar.set_label(strings::TOOLS_ACTIONS);
        toolbar.set_children(toolbar_children);
        content_children.push(toolbar_id);
        nodes.push((toolbar_id, toolbar));

        let mut layer_children = Vec::new();
        if let Some(page) = page {
            let mut rows = Vec::new();
            flatten(&page.nodes, 0, &mut rows);
            for (depth, layer) in rows {
                let Some(id) = A11Y_LAYER_BASE
                    .checked_add(layer.id.0)
                    .filter(|id| *id < A11Y_STATE_BASE)
                else {
                    continue;
                };
                let mut node = A11yNode::new(A11yRole::ListItem);
                node.set_label(layer.name.clone());
                node.set_level(depth + 1);
                node.set_selected(selection == Some(layer.id));
                node.add_action(A11yAction::Click);
                node.add_action(A11yAction::Focus);
                layer_children.push(A11yNodeId(id));
                nodes.push((A11yNodeId(id), node));
            }
        }
        let mut layers = A11yNode::new(A11yRole::List);
        layers.set_label(strings::LAYERS);
        layers.set_children(layer_children);
        content_children.push(A11yNodeId(A11Y_LAYER_LIST));
        nodes.push((A11yNodeId(A11Y_LAYER_LIST), layers));
    }
    let mut controls = Vec::new();
    match screen {
        Screen::Documents => controls.extend([
            (A11Y_NEW, strings::NEW_DOCUMENT, true, None),
            (A11Y_OPEN, strings::OPEN_DOCUMENT, true, None),
            (A11Y_SAVE_AS, strings::SAVE_AS, true, None),
            (A11Y_SAMPLE, strings::OPEN_SAMPLE, true, None),
        ]),
        Screen::Design => {
            controls.push((A11Y_TIMELINE, strings::TIMELINE, true, Some(timeline)));
            if timeline {
                controls.push((
                    A11Y_PLAY,
                    if playing {
                        strings::PAUSE
                    } else {
                        strings::PLAY
                    },
                    can_play,
                    Some(playing),
                ));
                let mut time = A11yNode::new(A11yRole::Label);
                time.set_value(if playing {
                    strings::PLAYING.to_owned()
                } else {
                    strings::milliseconds(time_ms)
                });
                nodes.push((A11yNodeId(A11Y_TIME), time));
                content_children.push(A11yNodeId(A11Y_TIME));
            }
            let selected = selection.and_then(|id| page.and_then(|p| p.node(id)));
            let mut canvas = A11yNode::new(A11yRole::Group);
            canvas.set_label(format!(
                "{}: {}",
                NarrowPanel::Canvas.label(),
                selected.map_or_else(|| strings::NOTHING_SELECTED.to_owned(), |n| n.name.clone())
            ));
            nodes.push((A11yNodeId(A11Y_CANVAS), canvas));
            content_children.push(A11yNodeId(A11Y_CANVAS));
            if let Some(n) = selected {
                for (index, (label, value)) in [
                    (strings::NAME, n.name.clone()),
                    (strings::KIND, strings::kind_label(&n.kind).to_owned()),
                    (strings::POSITION_X, crate::shell::format_number(n.bounds.x)),
                    (strings::POSITION_Y, crate::shell::format_number(n.bounds.y)),
                    (strings::WIDTH, crate::shell::format_number(n.bounds.width)),
                    (
                        strings::HEIGHT,
                        crate::shell::format_number(n.bounds.height),
                    ),
                    (
                        strings::OPACITY,
                        format!("{} %", crate::shell::format_number(n.opacity * 100.0)),
                    ),
                    (
                        strings::FILL,
                        n.fill
                            .map_or_else(|| strings::NO_FILL.to_owned(), |c| c.hex()),
                    ),
                    (
                        strings::FILL_OPACITY,
                        n.fill
                            .map_or_else(String::new, |c| format!("{} %", c.opacity_percent())),
                    ),
                    (
                        strings::VISIBLE,
                        if n.visible { strings::YES } else { strings::NO }.to_owned(),
                    ),
                    (
                        strings::LOCKED,
                        if n.locked { strings::YES } else { strings::NO }.to_owned(),
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let id = A11yNodeId(A11Y_PROPERTY_BASE + index as u64);
                    let mut node = A11yNode::new(A11yRole::Label);
                    node.set_label(label);
                    node.set_value(format!("{label}: {value}"));
                    nodes.push((id, node));
                    content_children.push(id);
                }
            }
            if let Some(n) = selected {
                let extra = match &n.kind {
                    designer_core::NodeKind::Rectangle { corner_radius } => {
                        Some((strings::RADIUS, crate::shell::format_number(*corner_radius)))
                    }
                    designer_core::NodeKind::Text { content, .. } => {
                        Some((strings::CONTENT, content.clone()))
                    }
                    _ => None,
                };
                if let Some((label, value)) = extra {
                    let mut node = A11yNode::new(A11yRole::Label);
                    node.set_label(label);
                    node.set_value(format!("{label}: {value}"));
                    nodes.push((A11yNodeId(A11Y_PROPERTY_EXTRA), node));
                    content_children.push(A11yNodeId(A11Y_PROPERTY_EXTRA));
                }
            }
            if narrow {
                let mut tabs = A11yNode::new(A11yRole::TabList);
                tabs.set_label(strings::SECTIONS);
                let mut children = Vec::new();
                for (index, item) in NarrowPanel::ALL.into_iter().enumerate() {
                    let id = A11yNodeId(A11Y_PANEL_BASE + index as u64);
                    let mut node = A11yNode::new(A11yRole::Tab);
                    node.set_label(item.label());
                    node.set_selected(item == panel);
                    node.add_action(A11yAction::Click);
                    node.add_action(A11yAction::Focus);
                    nodes.push((id, node));
                    children.push(id);
                }
                tabs.set_children(children);
                nodes.push((A11yNodeId(A11Y_PANELS), tabs));
                content_children.push(A11yNodeId(A11Y_PANELS));
            }
        }
        Screen::Preview => {
            let can_create = selection.is_some()
                && page.is_some_and(|p| {
                    p.prototype.states.len() < designer_core::MAX_PROTOTYPE_STATES
                });
            controls.extend([
                (A11Y_HOVER, strings::CREATE_HOVER_STATE, can_create, None),
                (A11Y_CLICK, strings::CREATE_CLICK_STATE, can_create, None),
                (
                    A11Y_REMOVE_STATE,
                    strings::REMOVE_PROTOTYPE_STATE,
                    prototype_state != designer_core::INITIAL_STATE,
                    None,
                ),
            ]);
            let mut list = A11yNode::new(A11yRole::List);
            list.set_label(strings::PROTOTYPE_STATES);
            let mut children = Vec::new();
            if let Some(page) = page {
                for state in &page.prototype.states {
                    // Reserve the upper half for prototype identities.
                    if let Some(raw) = A11Y_STATE_BASE.checked_add(state.id.0) {
                        let id = A11yNodeId(raw);
                        let mut node = A11yNode::new(A11yRole::ListItem);
                        node.set_label(state.name.clone());
                        node.set_selected(state.id == prototype_state);
                        node.add_action(A11yAction::Click);
                        node.add_action(A11yAction::Focus);
                        nodes.push((id, node));
                        children.push(id);
                    }
                }
            }
            list.set_children(children);
            nodes.push((A11yNodeId(A11Y_STATES), list));
            content_children.push(A11yNodeId(A11Y_STATES));
        }
        Screen::Import => {
            controls.extend([
                (A11Y_IMPORT_PICK, strings::IMPORT_PICK_FILE, true, None),
                (A11Y_IMPORT_APPLY, strings::IMPORT_APPLY, can_import, None),
            ]);
            let mut node = A11yNode::new(A11yRole::Label);
            node.set_value(import_summary);
            nodes.push((A11yNodeId(A11Y_IMPORT_SUMMARY), node));
            content_children.push(A11yNodeId(A11Y_IMPORT_SUMMARY));
        }
        Screen::Preferences => controls.push((A11Y_THEME, strings::THEME_DARK, true, Some(dark))),
        Screen::Components => {}
    }
    for (raw, label, enabled, toggled) in controls {
        let id = A11yNodeId(raw);
        let mut node = A11yNode::new(A11yRole::Button);
        node.set_label(label);
        if enabled {
            node.add_action(A11yAction::Click);
            node.add_action(A11yAction::Focus);
        } else {
            node.set_disabled();
        }
        if let Some(value) = toggled {
            node.set_toggled(if value {
                accesskit::Toggled::True
            } else {
                accesskit::Toggled::False
            });
        }
        nodes.push((id, node));
        content_children.push(id);
    }
    if screen == Screen::Components {
        let mut note = A11yNode::new(A11yRole::Label);
        note.set_value(screen.pending_description());
        content_children.push(A11yNodeId(A11Y_NOTE));
        nodes.push((A11yNodeId(A11Y_NOTE), note));
    }
    if let Some(status) = status {
        let mut status_node = A11yNode::new(A11yRole::Status);
        status_node.set_label(status);
        status_node.set_live(accesskit::Live::Polite);
        content_children.push(A11yNodeId(A11Y_STATUS_NODE));
        nodes.push((A11yNodeId(A11Y_STATUS_NODE), status_node));
    }
    content.set_children(content_children);
    nodes.push((A11yNodeId(A11Y_CONTENT_GROUP), content));

    let mut root = A11yNode::new(A11yRole::Window);
    root.set_label(title);
    root.set_children(root_children);
    root.set_bounds(bounds);
    nodes.push((A11yNodeId(0), root));
    let focus = if nodes.iter().any(|(id, _)| *id == focus) {
        focus
    } else {
        A11yNodeId(0)
    };
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(A11yNodeId(0))),
        tree_id: TreeId::ROOT,
        focus,
    }
}

fn flatten<'a>(nodes: &'a [Node], depth: usize, rows: &mut Vec<(usize, &'a Node)>) {
    // Topmost layer first, as painted last.
    for node in nodes.iter().rev() {
        rows.push((depth, node));
        flatten(&node.children, depth + 1, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectable_groups_always_expose_selection_state() {
        let mut doc = designer_core::sample_document();
        doc.pages[0]
            .prototype
            .states
            .push(designer_core::PrototypeState {
                id: designer_core::PrototypeStateId(999),
                name: "Other".into(),
                overrides: vec![],
            });
        let page = &doc.pages[0];
        for screen in Screen::ALL {
            let mut state = snapshot(screen);
            state.page = Some(page);
            state.selection = Some(page.nodes[0].id);
            state.prototype_state = designer_core::PrototypeStateId(999);
            let tree = build_accessibility_tree(state);
            let mut groups = vec![(A11Y_NAV_GROUP, A11yRole::Tab)];
            if screen == Screen::Design {
                groups.extend([
                    (A11Y_LAYER_LIST, A11yRole::ListItem),
                    (A11Y_PANELS, A11yRole::Tab),
                ]);
            } else if screen == Screen::Preview {
                groups.push((A11Y_STATES, A11yRole::ListItem));
            }
            for (group, role) in groups {
                let parent = &tree
                    .nodes
                    .iter()
                    .find(|(id, _)| id.0 == group)
                    .expect("group")
                    .1;
                let mut selected = 0;
                for child in parent.children() {
                    let node = &tree
                        .nodes
                        .iter()
                        .find(|(id, _)| id == child)
                        .expect("child")
                        .1;
                    assert_eq!(node.role(), role);
                    match node.is_selected() {
                        Some(true) => selected += 1,
                        Some(false) => {}
                        None => panic!("selection pattern missing for {child:?}"),
                    }
                }
                assert_eq!(selected, 1, "group {group}");
            }
        }
    }

    #[test]
    fn playback_time_does_not_change_tree() {
        let mut a = snapshot(Screen::Design);
        a.playing = true;
        let mut b = a.clone();
        b.time_ms += 123;
        assert_eq!(build_accessibility_tree(a), build_accessibility_tree(b));
    }

    #[test]
    fn controls_states_and_ranges_are_consistent() {
        let expected = [
            (
                Screen::Documents,
                vec![A11Y_NEW, A11Y_OPEN, A11Y_SAVE_AS, A11Y_SAMPLE],
            ),
            (
                Screen::Design,
                vec![A11Y_UNDO, A11Y_REDO, A11Y_DELETE, A11Y_TIMELINE, A11Y_PLAY],
            ),
            (
                Screen::Preview,
                vec![A11Y_HOVER, A11Y_CLICK, A11Y_REMOVE_STATE],
            ),
            (Screen::Import, vec![A11Y_IMPORT_PICK, A11Y_IMPORT_APPLY]),
            (Screen::Preferences, vec![A11Y_THEME]),
            (Screen::Components, vec![]),
        ];
        for (screen, ids) in expected {
            let tree = build_accessibility_tree(snapshot(screen));
            for id in ids {
                let node = &tree
                    .nodes
                    .iter()
                    .find(|(n, _)| n.0 == id)
                    .expect("control")
                    .1;
                assert_eq!(node.role(), A11yRole::Button);
                if !node.is_disabled() {
                    assert!(node.supports_action(A11yAction::Click));
                }
            }
        }
        for (screen, id) in [
            (Screen::Design, A11Y_TOOL_BASE),
            (Screen::Design, A11Y_TIMELINE),
            (Screen::Preferences, A11Y_THEME),
        ] {
            let tree = build_accessibility_tree(snapshot(screen));
            assert_eq!(
                tree.nodes
                    .iter()
                    .find(|(n, _)| n.0 == id)
                    .unwrap()
                    .1
                    .toggled(),
                Some(accesskit::Toggled::True)
            );
        }
        let mut state = snapshot(Screen::Design);
        state.narrow = false;
        assert!(
            !build_accessibility_tree(state)
                .nodes
                .iter()
                .any(|(id, _)| id.0 == A11Y_PANELS)
        );
        let doc = designer_core::sample_document();
        let mut state = snapshot(Screen::Design);
        state.page = Some(&doc.pages[0]);
        state.selection = Some(doc.pages[0].nodes[0].id);
        let tree = build_accessibility_tree(state);
        assert!(
            tree.nodes
                .iter()
                .find(|(id, _)| id.0 == A11Y_PROPERTY_BASE + 4)
                .unwrap()
                .1
                .value()
                .unwrap()
                .starts_with(strings::WIDTH)
        );
        let mut state = snapshot(Screen::Preview);
        state.page = Some(&doc.pages[0]);
        let tree = build_accessibility_tree(state);
        assert_eq!(
            tree.nodes
                .iter()
                .find(|(id, _)| id.0 == A11Y_STATE_BASE)
                .unwrap()
                .1
                .is_selected(),
            Some(true)
        );
    }

    #[test]
    fn labels_and_disabled_controls_expose_correct_properties() {
        for (screen, ids) in [
            (
                Screen::Design,
                vec![A11Y_UNDO, A11Y_REDO, A11Y_DELETE, A11Y_PLAY],
            ),
            (
                Screen::Preview,
                vec![A11Y_HOVER, A11Y_CLICK, A11Y_REMOVE_STATE],
            ),
            (Screen::Import, vec![A11Y_IMPORT_APPLY]),
        ] {
            let tree = build_accessibility_tree(snapshot(screen));
            for id in ids {
                let node = &tree
                    .nodes
                    .iter()
                    .find(|(n, _)| n.0 == id)
                    .expect("disabled control")
                    .1;
                assert!(node.is_disabled());
                assert!(!node.supports_action(A11yAction::Click));
                assert!(!node.supports_action(A11yAction::Focus));
            }
        }
        for screen in Screen::ALL {
            let tree = build_accessibility_tree(snapshot(screen));
            for (_, node) in tree.nodes {
                if node.role() == A11yRole::Label {
                    assert!(node.value().is_some_and(|value| !value.is_empty()));
                }
                if node.is_disabled() {
                    assert!(!node.supports_action(A11yAction::Click));
                    assert!(!node.supports_action(A11yAction::Focus));
                }
            }
        }
    }

    #[test]
    fn fixed_ids_and_ranges_are_disjoint() {
        let fixed = [
            A11Y_NAV_GROUP,
            A11Y_CONTENT_GROUP,
            A11Y_STATUS_NODE,
            A11Y_LAYER_LIST,
            A11Y_TOOLBAR_GROUP,
            A11Y_NOTE,
            A11Y_TIME,
            A11Y_CANVAS,
            A11Y_PANELS,
            A11Y_STATES,
            A11Y_IMPORT_SUMMARY,
            A11Y_UNDO,
            A11Y_REDO,
            A11Y_DELETE,
            A11Y_NEW,
            A11Y_OPEN,
            A11Y_SAVE_AS,
            A11Y_SAMPLE,
            A11Y_TIMELINE,
            A11Y_PLAY,
            A11Y_HOVER,
            A11Y_CLICK,
            A11Y_REMOVE_STATE,
            A11Y_IMPORT_PICK,
            A11Y_IMPORT_APPLY,
            A11Y_THEME,
            A11Y_PROPERTY_EXTRA,
        ];
        let mut ids = std::collections::HashSet::new();
        for id in fixed {
            assert!(ids.insert(id));
        }
        for range in [
            A11Y_NAV_BASE..A11Y_NAV_BASE + Screen::ALL.len() as u64,
            A11Y_TOOL_BASE..A11Y_TOOL_BASE + Tool::ALL.len() as u64,
            A11Y_PANEL_BASE..A11Y_PANEL_LAST + 1,
            A11Y_PROPERTY_BASE..A11Y_PROPERTY_BASE + 11,
        ] {
            for id in range {
                assert!(ids.insert(id));
            }
        }
        assert!(ids.iter().all(|id| *id < A11Y_LAYER_BASE));
        const { assert!(A11Y_LAYER_BASE < A11Y_STATE_BASE) };
    }

    fn snapshot(screen: Screen) -> A11ySnapshot<'static> {
        A11ySnapshot {
            screen,
            tool: Tool::Move,
            selection: None,
            page: None,
            can_undo: false,
            can_redo: false,
            status: Some("ready"),
            title: "test".into(),
            bounds: A11yRect::new(0.0, 0.0, 800.0, 600.0),
            focus: A11yNodeId(u64::MAX),
            dark: true,
            timeline: true,
            playing: false,
            time_ms: 100,
            can_play: false,
            narrow: true,
            panel: NarrowPanel::Canvas,
            prototype_state: designer_core::INITIAL_STATE,
            can_import: false,
            import_summary: "none".into(),
        }
    }
    #[test]
    fn all_screens_have_unique_closed_trees_and_selected_navigation() {
        for (index, screen) in Screen::ALL.into_iter().enumerate() {
            let state = snapshot(screen);
            assert_eq!(state, state.clone());
            let tree = build_accessibility_tree(state.clone());
            assert_eq!(tree, build_accessibility_tree(state));
            let ids: std::collections::HashSet<_> = tree.nodes.iter().map(|(id, _)| *id).collect();
            assert_eq!(ids.len(), tree.nodes.len());
            for (_, node) in &tree.nodes {
                for child in node.children() {
                    assert!(ids.contains(child));
                }
                if node.is_disabled() {
                    assert!(!node.supports_action(A11yAction::Click));
                }
            }
            let root = &tree.nodes.iter().find(|(id, _)| id.0 == 0).expect("root").1;
            assert_eq!(
                root.children(),
                &[A11yNodeId(A11Y_NAV_GROUP), A11yNodeId(A11Y_CONTENT_GROUP)]
            );
            let tab = &tree
                .nodes
                .iter()
                .find(|(id, _)| id.0 == A11Y_NAV_BASE + index as u64)
                .expect("tab")
                .1;
            assert_eq!(tab.is_selected(), Some(true));
            let status = &tree
                .nodes
                .iter()
                .find(|(id, _)| id.0 == A11Y_STATUS_NODE)
                .expect("status")
                .1;
            assert_eq!(status.live(), Some(accesskit::Live::Polite));
            assert_eq!(tree.focus, A11yNodeId(0));
        }
    }
    #[test]
    fn layers_expose_selection_level_and_focus() {
        let doc = designer_core::sample_document();
        let page = &doc.pages[0];
        let selected = page.nodes[0].id;
        let mut state = snapshot(Screen::Design);
        state.page = Some(page);
        state.selection = Some(selected);
        state.focus = A11yNodeId(A11Y_LAYER_BASE + selected.0);
        let tree = build_accessibility_tree(state);
        let layer = &tree
            .nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .expect("layer")
            .1;
        assert_eq!(layer.is_selected(), Some(true));
        assert_eq!(layer.level(), Some(1));
    }
}
