//! Timeline controls share the same transactions and history as canvas edits.

use designer_core::{Easing, MAX_DURATION_MS, Property};

use super::*;

fn row() -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(metrics::SPACE_XS))
}

impl Shell {
    fn seek_animation(&mut self, time: u32, cx: &mut Context<Self>) {
        let Some(page) = self.session.document().page(self.page) else {
            return;
        };
        self.animation.seek(time, page.animation.duration_ms);
        self.drag = None;
        self.preview = None;
        self.redraw(cx);
    }

    fn animation_command(&mut self, operation: Operation, cx: &mut Context<Self>) {
        self.animation.pause();
        self.status = self
            .session
            .apply(&[operation])
            .err()
            .map(|error| strings::change_failed(&error).into());
        self.settle(cx);
    }

    fn write_key(&mut self, delta: f64, change_easing: bool, cx: &mut Context<Self>) {
        let Some(page) = self.session.document().page(self.page) else {
            return;
        };
        let Some(id) = self.selection else {
            return;
        };
        let evaluated = page.at_time(self.animation.time_ms);
        let Some(node) = evaluated.node(id) else {
            return;
        };
        if node.locked {
            return;
        }
        let property = self.animation.property;
        let existing = page.animation.track(id, property).and_then(|track| {
            track
                .keys
                .iter()
                .find(|key| key.time_ms == self.animation.time_ms)
        });
        let mut easing = existing.map_or(self.animation.easing, |key| key.easing);
        if change_easing {
            easing = match easing {
                Easing::Linear => Easing::Constant,
                Easing::Constant => Easing::Linear,
            };
        }
        self.animation.easing = easing;
        let value = crate::timeline::value(node, property) + delta;
        let value = match property {
            Property::Opacity => value.clamp(0.0, 1.0),
            Property::Width | Property::Height => value.max(0.0),
            _ => value,
        };
        self.animation.preview = true;
        self.animation_command(
            Operation::SetKeyframe {
                page: self.page,
                node: id,
                property,
                time_ms: self.animation.time_ms,
                value,
                easing,
            },
            cx,
        );
    }

    pub(super) fn timeline_editor(
        &self,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let Some(page) = self.session.document().page(self.page) else {
            return div().id("timeline-editor");
        };
        let duration = page.animation.duration_ms;
        let time = self.animation.time_ms;
        let evaluated = page.at_time(time);
        let node = self.selection.and_then(|id| evaluated.node(id));
        let editable = node.is_some_and(|node| !node.locked);
        let track = self
            .selection
            .and_then(|id| page.animation.track(id, self.animation.property));
        let key = track.and_then(|track| track.keys.iter().find(|key| key.time_ms == time));
        let easing = key.map_or(self.animation.easing, |key| key.easing);
        let step = if self.animation.property == Property::Opacity {
            0.1
        } else {
            10.0
        };
        let transport = row()
            .child(command(
                "animation-play",
                if self.animation.is_playing() {
                    strings::PAUSE
                } else {
                    strings::PLAY
                },
                !page.animation.tracks.is_empty(),
                palette,
                cx.listener(move |this, _, _, cx| {
                    this.drag = None;
                    this.preview = None;
                    this.animation
                        .toggle_play(std::time::Instant::now(), duration);
                    this.redraw(cx);
                }),
            ))
            .child(
                button(
                    "animation-base",
                    strings::BASE_VALUES,
                    palette,
                    !self.animation.preview,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.animation.pause();
                    this.animation.preview = false;
                    this.redraw(cx);
                })),
            )
            .child(
                button("time-back", strings::TIME_BACK, palette, false).on_click(cx.listener(
                    move |this, _, _, cx| this.seek_animation(time.saturating_sub(100), cx),
                )),
            )
            .child(
                button("time-forward", strings::TIME_FORWARD, palette, false).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.seek_animation(time.saturating_add(100), cx)
                    }),
                ),
            )
            .child(muted(strings::animation_time(time, duration), palette));
        let ruler = row().children((0..=4_u32).map(|index| {
            let at = duration * index / 4;
            button(
                ("time-mark", index),
                strings::milliseconds(at),
                palette,
                self.animation.preview && time == at,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.seek_animation(at, cx)))
        }));
        let duration_row = row()
            .child(muted(strings::DURATION, palette))
            .child(
                button("duration-less", strings::DURATION_LESS, palette, false).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.animation_command(
                            Operation::SetDuration {
                                page: this.page,
                                duration_ms: duration.saturating_sub(500).max(100),
                            },
                            cx,
                        )
                    }),
                ),
            )
            .child(
                button("duration-more", strings::DURATION_MORE, palette, false).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.animation_command(
                            Operation::SetDuration {
                                page: this.page,
                                duration_ms: duration.saturating_add(500).min(MAX_DURATION_MS),
                            },
                            cx,
                        )
                    }),
                ),
            );
        let properties = row().children(Property::ALL.into_iter().enumerate().map(
            |(index, property)| {
                button(
                    ("animation-property", index),
                    strings::property_label(property),
                    palette,
                    self.animation.property == property,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.animation.property = property;
                    this.animation.pause();
                    cx.notify();
                }))
            },
        ));
        let value_label = node.map_or_else(
            || strings::ANIMATION_SELECT.to_owned(),
            |node| {
                strings::animation_value(
                    &node.name,
                    crate::timeline::value(node, self.animation.property),
                )
            },
        );
        let editor = row()
            .child(command(
                "key-write",
                if key.is_some() {
                    strings::REPLACE_KEY
                } else {
                    strings::ADD_KEY
                },
                editable,
                palette,
                cx.listener(|this, _, _, cx| this.write_key(0.0, false, cx)),
            ))
            .child(command(
                "key-less",
                strings::VALUE_LESS,
                editable,
                palette,
                cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                    this.write_key(
                        -step / if event.modifiers().shift { 10.0 } else { 1.0 },
                        false,
                        cx,
                    )
                }),
            ))
            .child(command(
                "key-more",
                strings::VALUE_MORE,
                editable,
                palette,
                cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                    this.write_key(
                        step / if event.modifiers().shift { 10.0 } else { 1.0 },
                        false,
                        cx,
                    )
                }),
            ))
            .child(command(
                "key-easing",
                match easing {
                    Easing::Linear => strings::LINEAR,
                    Easing::Constant => strings::CONSTANT,
                },
                editable,
                palette,
                cx.listener(|this, _, _, cx| this.write_key(0.0, true, cx)),
            ))
            .child(command(
                "key-remove",
                strings::REMOVE_KEY,
                editable && key.is_some(),
                palette,
                cx.listener(|this, _, _, cx| {
                    if let Some(node) = this.selection {
                        this.animation_command(
                            Operation::RemoveKeyframe {
                                page: this.page,
                                node,
                                property: this.animation.property,
                                time_ms: this.animation.time_ms,
                            },
                            cx,
                        );
                    }
                }),
            ));
        let keys = row().children(track.into_iter().flat_map(|track| track.keys.iter()).map(
            |key| {
                let at = key.time_ms;
                button(
                    ("key-at", at),
                    strings::key_label(at, key.value),
                    palette,
                    time == at,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.seek_animation(at, cx)))
            },
        ));
        div()
            .id("timeline-editor")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(metrics::SPACE_S))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_XS))
            .child(transport)
            .child(ruler)
            .child(duration_row)
            .child(properties)
            .child(muted(value_label, palette).truncate())
            .child(editor)
            .child(keys)
            .child(muted(
                if page.animation.tracks.is_empty() {
                    strings::NO_ANIMATIONS
                } else {
                    strings::ANIMATION_HINT
                },
                palette,
            ))
    }
}
