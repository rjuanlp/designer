//! Inspector presentation and session integration.
use super::*;
use crate::inspector::{self, Field};
use crate::text_field::view::FieldView;

impl Field {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Name => strings::NAME,
            Self::X => strings::POSITION_X,
            Self::Y => strings::POSITION_Y,
            Self::Width => strings::WIDTH,
            Self::Height => strings::HEIGHT,
            Self::Radius => strings::RADIUS,
            Self::Opacity => strings::OPACITY,
            Self::Fill => strings::FILL,
            Self::Visible => strings::VISIBLE,
            Self::Locked => strings::LOCKED,
            Self::Clips => strings::CLIPS_CONTENT,
        }
    }
}

impl Shell {
    pub(super) fn inspector_accessibility(
        &mut self,
        request: &ActionRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let target = request.target_node.0;
        if (A11Y_EDIT_BASE..A11Y_EDIT_BASE + 11).contains(&target) {
            let field = Field::ALL[(target - A11Y_EDIT_BASE) as usize];
            if request.action == A11yAction::Focus {
                if let Some((_, view)) = self.fields.iter().find(|(f, _)| *f == field) {
                    window.focus(&view.read(cx).focus);
                }
            } else if let Some(id) = self.selection {
                if let Some(accesskit::ActionData::Value(value)) = &request.data {
                    self.commit_field(id, field, value, false, cx);
                } else if request.action == A11yAction::Click
                    && let Some(n) = self.effective_node(id)
                {
                    self.commit_field(
                        id,
                        field,
                        if field.value(&n) == "true" {
                            "false"
                        } else {
                            "true"
                        },
                        false,
                        cx,
                    );
                }
            }
            return true;
        }
        if (A11Y_RESET_BASE..A11Y_RESET_BASE + 11).contains(&target)
            && request.action == A11yAction::Click
        {
            let field = Field::ALL[(target - A11Y_RESET_BASE) as usize];
            if let Some(n) = self.selection.and_then(|id| self.effective_node(id)) {
                self.commit_field(n.id, field, &field.value(&n), true, cx);
            }
            return true;
        }
        if target == A11Y_NO_FILL && request.action == A11yAction::Click {
            if let Some(n) = self.selection.and_then(|id| self.effective_node(id)) {
                self.commit_field(
                    n.id,
                    Field::Fill,
                    if n.fill.is_some() {
                        ""
                    } else {
                        inspector::DEFAULT_FILL
                    },
                    false,
                    cx,
                );
            }
            return true;
        }
        if (A11Y_LAYER_CONTROL_BASE..A11Y_STATE_BASE).contains(&target)
            && request.action == A11yAction::Click
        {
            let offset = target - A11Y_LAYER_CONTROL_BASE;
            let id = NodeId(offset / 4);
            if let Some(n) = self.current_page().and_then(|p| p.node(id)).cloned() {
                match offset % 4 {
                    0 => {
                        self.commit_field(id, Field::Visible, &(!n.visible).to_string(), false, cx)
                    }
                    1 => self.commit_field(id, Field::Locked, &(!n.locked).to_string(), false, cx),
                    index => {
                        if !n.locked
                            && let Some(index) = self
                                .current_page()
                                .and_then(|p| inspector::reorder(&p.nodes, id, index == 2))
                        {
                            let _ = self.session.apply(&[Operation::ReorderNode {
                                page: self.page,
                                node: id,
                                index,
                            }]);
                            self.redraw(cx);
                        }
                    }
                }
            }
            return true;
        }
        false
    }
    fn edit_state(&self) -> Option<PrototypeStateId> {
        (self.screen == Screen::Preview).then_some(self.prototype_preview.state())
    }
    fn effective_node(&self, id: NodeId) -> Option<Node> {
        inspector::effective_node(
            self.session.document().page(self.page)?,
            id,
            self.edit_state(),
        )
    }
    pub(super) fn commit_field(
        &mut self,
        id: NodeId,
        field: Field,
        value: &str,
        reset: bool,
        cx: &mut Context<Self>,
    ) {
        self.commit_target((self.page, id, self.edit_state()), field, value, reset, cx);
    }
    fn commit_target(
        &mut self,
        target: (PageId, NodeId, Option<PrototypeStateId>),
        field: Field,
        value: &str,
        reset: bool,
        cx: &mut Context<Self>,
    ) {
        let (page_id, id, state) = target;
        let result = (|| {
            let page = self.session.document().page(page_id).ok_or(())?;
            let op = inspector::edit_operation(page, id, state, field, value, reset)?;
            self.session.apply(&[op]).map_err(|_| ())?;
            if self.page == page_id
                && self.edit_state() == state
                && let Some(state) = state
            {
                let page = self.session.document().page(page_id).ok_or(())?;
                self.prototype_preview = PrototypePreview::new(page);
                self.prototype_preview.select_state(page, state);
            }
            Ok::<_, ()>(())
        })();
        self.status = result.err().map(|()| strings::INVALID_PROPERTY.into());
        self.redraw(cx);
    }
    pub(super) fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.selection.map(|id| (self.page, id, self.edit_state()));
        if target != self.field_target {
            if let Some(previous) = self.field_target {
                let pending: Vec<_> = self
                    .fields
                    .iter()
                    .filter_map(|(field, view)| {
                        view.update(cx, |v, _| v.take_pending())
                            .map(|value| (*field, value))
                    })
                    .collect();
                for (field, value) in pending {
                    self.commit_target(previous, field, &value, false, cx);
                }
            }
            self.fields.clear();
            self.field_target = target;
        }
        let Some(id) = self.selection else {
            return;
        };
        let Some(node) = self.effective_node(id) else {
            return;
        };
        let palette = self.palette();
        for field in Field::ALL.into_iter().filter(|f| f.available(&node)) {
            if let Some((_, view)) = self.fields.iter().find(|(f, _)| *f == field) {
                view.update(cx, |v, _| v.sync(field.value(&node), palette));
            } else {
                let shell = cx.weak_entity();
                let view = cx.new(|cx| {
                    FieldView::new(
                        field.value(&node),
                        palette,
                        window,
                        cx,
                        move |value, _, cx| {
                            shell
                                .update(cx, |this, cx| {
                                    if let Some(target) = target {
                                        this.commit_target(target, field, &value, false, cx);
                                    }
                                    this.effective_node(id)
                                        .map_or(value.clone(), |n| field.value(&n))
                                })
                                .unwrap_or(value)
                        },
                    )
                });
                self.fields.push((field, view));
            }
        }
    }
    pub(super) fn inspector(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let panel = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(palette.panel))
            .child(section_title(strings::PROPERTIES, palette));
        let Some(node) = self.selection.and_then(|id| self.effective_node(id)) else {
            return panel.child(
                muted(strings::NOTHING_SELECTED, palette)
                    .px(px(metrics::SPACE_M))
                    .py(px(metrics::SPACE_S)),
            );
        };
        let id = node.id;
        let overrides = self.edit_state().and_then(|state| {
            self.current_page()?
                .prototype
                .state(state)?
                .overrides
                .iter()
                .find(|o| o.node == id)
                .cloned()
        });
        let overridden = |field: Field| {
            overrides.as_ref().is_some_and(|o| match field {
                Field::X | Field::Y | Field::Width | Field::Height => o.bounds.is_some(),
                Field::Fill => o.fill.is_some(),
                Field::Opacity => o.opacity.is_some(),
                _ => false,
            })
        };
        let row = |label: &'static str, value: AnyElement| {
            div()
                .flex()
                .items_center()
                .gap(px(metrics::SPACE_S))
                .px(px(metrics::SPACE_M))
                .py(px(metrics::SPACE_XS))
                .child(
                    div()
                        .flex_none()
                        .w(px(metrics::LABEL_WIDTH))
                        .text_color(rgb(palette.text_muted))
                        .child(label),
                )
                .child(div().flex_1().min_w_0().child(value))
                .into_any_element()
        };
        let mut rows = vec![row(
            strings::KIND,
            div()
                .child(strings::kind_label(&node.kind))
                .into_any_element(),
        )];
        for (field, view) in self.fields.iter().filter(|(f, _)| f.available(&node)) {
            let field = *field;
            let enabled = !node.locked || field == Field::Locked;
            let value = if field.is_toggle() {
                let yes = field.value(&node) == "true";
                command(
                    ("property", field as u64),
                    if yes { Icon::ToggleOn } else { Icon::ToggleOff },
                    if yes { strings::YES } else { strings::NO },
                    enabled,
                    palette,
                    cx.listener(move |this, _, _, cx| {
                        this.commit_field(id, field, if yes { "false" } else { "true" }, false, cx)
                    }),
                )
            } else if !enabled {
                muted(field.value(&node), palette).into_any_element()
            } else if field == Field::Fill {
                div()
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_S))
                    .when_some(node.fill, |this, color| {
                        this.child(
                            div()
                                .flex_none()
                                .size(px(metrics::SWATCH))
                                .rounded(px(metrics::SPACE_XS))
                                .border_1()
                                .border_color(rgb(palette.border))
                                .bg(rgb((u32::from(color.r) << 16)
                                    | (u32::from(color.g) << 8)
                                    | u32::from(color.b))),
                        )
                    })
                    .child(div().flex_1().min_w_0().child(view.clone()))
                    .child({
                        let clear = node.fill.is_some();
                        command(
                            "no-fill",
                            if clear { Icon::FillNone } else { Icon::FillAdd },
                            if clear {
                                strings::NO_FILL
                            } else {
                                strings::ADD_FILL
                            },
                            true,
                            palette,
                            cx.listener(move |this, _, _, cx| {
                                this.commit_field(
                                    id,
                                    Field::Fill,
                                    if clear { "" } else { inspector::DEFAULT_FILL },
                                    false,
                                    cx,
                                )
                            }),
                        )
                    })
                    .into_any_element()
            } else {
                view.clone().into_any_element()
            };
            let value = if field.overridable() && overridden(field) {
                let shown = field.value(&node);
                div()
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_XS))
                    .child(div().flex_1().min_w_0().child(value))
                    .child(command(
                        ("reset-property", field as u64),
                        Icon::Reset,
                        strings::RESET_OVERRIDE,
                        enabled,
                        palette,
                        cx.listener(move |this, _, _, cx| {
                            this.commit_field(id, field, &shown, true, cx)
                        }),
                    ))
                    .into_any_element()
            } else {
                value
            };
            rows.push(row(field.label(), value));
        }
        if let NodeKind::Text { content, .. } = &node.kind {
            rows.push(row(
                strings::CONTENT,
                div().child(content.clone()).into_any_element(),
            ));
        }
        panel.child(
            div()
                .id("inspector-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(rows),
        )
    }

    /// A compact visibility or lock toggle at the end of a layer row. It is
    /// not a tab stop: the same toggles are in the inspector.
    pub(super) fn layer_toggle(
        &self,
        node: &Node,
        field: Field,
        selected: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = node.id;
        let (on, icon, label, name) = match field {
            Field::Visible => (
                node.visible,
                if node.visible {
                    Icon::Eye
                } else {
                    Icon::EyeOff
                },
                if node.visible {
                    strings::HIDE_LAYER
                } else {
                    strings::SHOW_LAYER
                },
                "layer-visible",
            ),
            _ => (
                node.locked,
                if node.locked {
                    Icon::Lock
                } else {
                    Icon::Unlock
                },
                if node.locked {
                    strings::UNLOCK_LAYER
                } else {
                    strings::LOCK_LAYER
                },
                "layer-locked",
            ),
        };
        let color = if selected {
            palette.selected_text
        } else {
            palette.text
        };
        // The resting state of both toggles (visible, unlocked) is faint.
        let resting = on == (field == Field::Visible);
        let enabled = !node.locked || field == Field::Locked;
        let hover = palette.hover;
        div()
            .id((name, id.0))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(metrics::SWATCH + metrics::SPACE_XS))
            .rounded(px(metrics::SPACE_XS))
            .when(resting, |this| this.opacity(0.4))
            .tooltip(tooltip(label, palette))
            .when(enabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(rgb(hover)).opacity(1.0))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.commit_field(id, field, &(!on).to_string(), false, cx);
                    }))
            })
            .child(icon_image(icon, color))
            .into_any_element()
    }

    /// Raise and lower commands for the selected layer, shown in the layers
    /// header.
    pub(super) fn layer_order_controls(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let selected = self
            .shown_selection()
            .and_then(|id| self.current_page()?.node(id).map(|n| (id, n.locked)));
        let mut controls = div().flex().flex_none().pr(px(metrics::SPACE_XS));
        for (raise, icon, label) in [
            (true, Icon::ArrowUp, strings::RAISE_LAYER),
            (false, Icon::ArrowDown, strings::LOWER_LAYER),
        ] {
            let target = selected.filter(|(_, locked)| !locked).and_then(|(id, _)| {
                let index = inspector::reorder(&self.current_page()?.nodes, id, raise)?;
                Some((id, index))
            });
            controls = controls.child(command(
                label,
                icon,
                label,
                target.is_some(),
                palette,
                cx.listener(move |this, _, _, cx| {
                    if let Some((node, index)) = target {
                        let _ = this.session.apply(&[Operation::ReorderNode {
                            page: this.page,
                            node,
                            index,
                        }]);
                        this.redraw(cx);
                    }
                }),
            ));
        }
        controls
    }
}
