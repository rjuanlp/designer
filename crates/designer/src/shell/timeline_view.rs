//! Timeline controls share the same transactions and history as canvas edits.

use designer_core::{Easing, MAX_DURATION_MS, Property, Sequence, SequenceId};

use super::*;

fn row() -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(metrics::SPACE_XS))
}

fn curve_preview(easing: Easing, palette: &Palette) -> Div {
    let points = (0..=20_u32).map(|index| {
        let progress = f64::from(index) / 20.0;
        let x = index as f32 * 4.5;
        let y = (1.0 - easing.progress(progress)) as f32 * 36.0;
        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .size(px(3.0))
            .rounded(px(1.5))
            .bg(rgb(palette.selected))
    });
    div()
        .relative()
        .flex_none()
        .w(px(96.0))
        .h(px(42.0))
        .rounded(px(metrics::RADIUS))
        .bg(rgb(palette.app))
        .border_1()
        .border_color(rgb(palette.border))
        .children(points)
}

fn append_sequence_rows(
    sequences: &[Sequence],
    depth: usize,
    rows: &mut Vec<(SequenceId, String, usize)>,
) {
    for sequence in sequences {
        rows.push((sequence.id, sequence.name.clone(), depth));
        append_sequence_rows(&sequence.children, depth + 1, rows);
    }
}

fn has_tracks(sequences: &[Sequence]) -> bool {
    sequences
        .iter()
        .any(|sequence| !sequence.tracks.is_empty() || has_tracks(&sequence.children))
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

    fn animation_command(&mut self, operation: Operation, cx: &mut Context<Self>) -> bool {
        self.animation.pause();
        let result = self.session.apply(&[operation]);
        let succeeded = result.is_ok();
        self.status = result
            .err()
            .map(|error| strings::change_failed(&error).into());
        self.settle(cx);
        succeeded
    }

    fn add_sequence(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.session.document().page(self.page) else {
            return;
        };
        let parent = self.animation.active_sequence;
        let (parent_offset, parent_duration) = parent
            .and_then(|id| {
                page.animation
                    .sequence_timing(id)
                    .map(|(offset, duration, _)| (offset, duration))
            })
            .unwrap_or((0, page.animation.duration_ms));
        let offset_ms = self
            .animation
            .time_ms
            .saturating_sub(parent_offset)
            .min(parent_duration);
        let duration_ms = parent_duration.saturating_sub(offset_ms).min(1000);
        let Some(sequence) = page.animation.next_sequence_id() else {
            return;
        };
        if duration_ms == 0 {
            return;
        }
        let operation = Operation::AddSequence {
            page: self.page,
            parent,
            sequence,
            name: format!("Secuencia {}", sequence.0),
            offset_ms,
            duration_ms,
        };
        if self.animation_command(operation, cx) {
            self.animation.active_sequence = Some(sequence);
            self.redraw(cx);
        }
    }

    fn remove_sequence(&mut self, cx: &mut Context<Self>) {
        let Some(sequence) = self.animation.active_sequence else {
            return;
        };
        if self.animation_command(
            Operation::RemoveSequence {
                page: self.page,
                sequence,
            },
            cx,
        ) {
            self.animation.active_sequence = None;
            self.redraw(cx);
        }
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
        let time_ms = match self.animation.active_sequence {
            Some(sequence) => {
                let Some((offset, duration, _)) = page.animation.sequence_timing(sequence) else {
                    return;
                };
                self.animation.time_ms.saturating_sub(offset).min(duration)
            }
            None => self.animation.time_ms.min(page.animation.duration_ms),
        };
        let track = match self.animation.active_sequence {
            Some(sequence) => page
                .animation
                .sequence(sequence)
                .and_then(|sequence| sequence.track(id, property)),
            None => page.animation.track(id, property),
        };
        let existing = track.and_then(|track| track.keys.iter().find(|key| key.time_ms == time_ms));
        let mut easing = existing.map_or(self.animation.easing, |key| key.easing);
        if change_easing {
            easing = easing.next();
        }
        self.animation.easing = easing;
        let value = crate::timeline::value(node, property) + delta;
        let value = match property {
            Property::Opacity => value.clamp(0.0, 1.0),
            Property::Width | Property::Height => value.max(0.0),
            _ => value,
        };
        self.animation.preview = true;
        let operation = if let Some(sequence) = self.animation.active_sequence {
            Operation::SetSequenceKeyframe {
                page: self.page,
                sequence,
                node: id,
                property,
                time_ms,
                value,
                easing,
            }
        } else {
            Operation::SetKeyframe {
                page: self.page,
                node: id,
                property,
                time_ms,
                value,
                easing,
            }
        };
        self.animation_command(operation, cx);
    }

    pub(super) fn timeline_editor(
        &self,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let Some(page) = self.session.document().page(self.page) else {
            return div().id("timeline-editor");
        };
        let global_time = self.animation.time_ms.min(page.animation.duration_ms);
        let active_sequence = self
            .animation
            .active_sequence
            .filter(|id| page.animation.sequence(*id).is_some());
        let sequence = active_sequence.and_then(|id| page.animation.sequence(id));
        let timing = active_sequence.and_then(|id| page.animation.sequence_timing(id));
        let sequence_offset = timing.map_or(0, |(offset, _, _)| offset);
        let duration = timing.map_or(page.animation.duration_ms, |(_, duration, _)| duration);
        let parent_duration =
            timing.map_or(page.animation.duration_ms, |(_, _, duration)| duration);
        let time = global_time.saturating_sub(sequence_offset).min(duration);
        let evaluated = page.at_time(global_time);
        let node = self.selection.and_then(|id| evaluated.node(id));
        let editable = node.is_some_and(|node| !node.locked);
        let track = self.selection.and_then(|id| match sequence {
            Some(sequence) => sequence.track(id, self.animation.property),
            None => page.animation.track(id, self.animation.property),
        });
        let key = track.and_then(|track| track.keys.iter().find(|key| key.time_ms == time));
        let easing = key.map_or(self.animation.easing, |key| key.easing);
        let step = if self.animation.property == Property::Opacity {
            0.1
        } else {
            10.0
        };
        let (parent_offset, parent_duration_for_add) = timing
            .map(|(offset, duration, _)| (offset, duration))
            .unwrap_or((0, page.animation.duration_ms));
        let local_parent_time = global_time
            .saturating_sub(parent_offset)
            .min(parent_duration_for_add);
        let can_add_sequence = parent_duration_for_add > local_parent_time
            && page.animation.next_sequence_id().is_some();
        let page_duration = page.animation.duration_ms;
        let mut sequence_rows = Vec::new();
        append_sequence_rows(&page.animation.sequences, 0, &mut sequence_rows);
        let sequence_controls = row()
            .child(
                button(
                    ("sequence-select", 0_u64),
                    strings::MAIN_SEQUENCE,
                    palette,
                    active_sequence.is_none(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.animation.active_sequence = None;
                    this.animation.pause();
                    this.redraw(cx);
                })),
            )
            .children(sequence_rows.into_iter().map(|(id, name, depth)| {
                let offset = page
                    .animation
                    .sequence_timing(id)
                    .map_or(0, |(offset, _, _)| offset);
                let label = format!("{}{}", "· ".repeat(depth + 1), name);
                button(
                    ("sequence-select", id.0),
                    label,
                    palette,
                    active_sequence == Some(id),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.animation.active_sequence = Some(id);
                    this.seek_animation(offset, cx);
                }))
            }))
            .child(command(
                "sequence-add",
                strings::ADD_SEQUENCE,
                can_add_sequence,
                palette,
                cx.listener(|this, _, _, cx| this.add_sequence(cx)),
            ))
            .child(command(
                "sequence-remove",
                strings::REMOVE_SEQUENCE,
                active_sequence.is_some(),
                palette,
                cx.listener(|this, _, _, cx| this.remove_sequence(cx)),
            ));
        let transport = row()
            .child(command(
                "animation-play",
                if self.animation.is_playing() {
                    strings::PAUSE
                } else {
                    strings::PLAY
                },
                !page.animation.tracks.is_empty() || has_tracks(&page.animation.sequences),
                palette,
                cx.listener(move |this, _, _, cx| {
                    this.drag = None;
                    this.preview = None;
                    this.animation
                        .toggle_play(std::time::Instant::now(), page_duration);
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
                    move |this, _, _, cx| this.seek_animation(global_time.saturating_sub(100), cx),
                )),
            )
            .child(
                button("time-forward", strings::TIME_FORWARD, palette, false).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.seek_animation(global_time.saturating_add(100), cx)
                    }),
                ),
            )
            .child(muted(strings::animation_time(time, duration), palette));
        let ruler = row().children((0..=4_u32).map(|index| {
            let local_at = duration * index / 4;
            let at = sequence_offset.saturating_add(local_at);
            button(
                ("time-mark", index),
                strings::milliseconds(local_at),
                palette,
                self.animation.preview && time == local_at,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.seek_animation(at, cx)))
        }));
        let current_sequence_offset = sequence.map_or(0, |sequence| sequence.offset_ms);
        let duration_less = duration.saturating_sub(500).max(1);
        let duration_more = if active_sequence.is_some() {
            duration
                .saturating_add(500)
                .min(parent_duration.saturating_sub(current_sequence_offset))
        } else {
            duration.saturating_add(500).min(MAX_DURATION_MS)
        };
        let duration_row = row()
            .child(muted(strings::DURATION, palette))
            .child(
                button("duration-less", strings::DURATION_LESS, palette, false).on_click(
                    cx.listener(move |this, _, _, cx| {
                        let operation = match this.animation.active_sequence {
                            Some(sequence) => Operation::SetSequenceTiming {
                                page: this.page,
                                sequence,
                                offset_ms: current_sequence_offset,
                                duration_ms: duration_less,
                            },
                            None => Operation::SetDuration {
                                page: this.page,
                                duration_ms: duration.saturating_sub(500).max(100),
                            },
                        };
                        this.animation_command(operation, cx);
                    }),
                ),
            )
            .child(
                button("duration-more", strings::DURATION_MORE, palette, false).on_click(
                    cx.listener(move |this, _, _, cx| {
                        let operation = match this.animation.active_sequence {
                            Some(sequence) => Operation::SetSequenceTiming {
                                page: this.page,
                                sequence,
                                offset_ms: current_sequence_offset,
                                duration_ms: duration_more,
                            },
                            None => Operation::SetDuration {
                                page: this.page,
                                duration_ms: duration.saturating_add(500).min(MAX_DURATION_MS),
                            },
                        };
                        this.animation_command(operation, cx);
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
                strings::easing_label(easing),
                editable,
                palette,
                cx.listener(|this, _, _, cx| this.write_key(0.0, true, cx)),
            ))
            .child(curve_preview(easing, palette))
            .child(command(
                "key-remove",
                strings::REMOVE_KEY,
                editable && key.is_some(),
                palette,
                cx.listener(move |this, _, _, cx| {
                    if let Some(node) = this.selection {
                        let operation = match this.animation.active_sequence {
                            Some(sequence) => Operation::RemoveSequenceKeyframe {
                                page: this.page,
                                sequence,
                                node,
                                property: this.animation.property,
                                time_ms: time,
                            },
                            None => Operation::RemoveKeyframe {
                                page: this.page,
                                node,
                                property: this.animation.property,
                                time_ms: time,
                            },
                        };
                        this.animation_command(operation, cx);
                    }
                }),
            ));
        let keys = row().children(track.into_iter().flat_map(|track| track.keys.iter()).map(
            |key| {
                let local_at = key.time_ms;
                let at = sequence_offset.saturating_add(local_at);
                button(
                    ("key-at", local_at),
                    strings::key_label(local_at, key.value),
                    palette,
                    time == local_at,
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
            .child(sequence_controls)
            .child(transport)
            .child(ruler)
            .child(duration_row)
            .child(properties)
            .child(muted(value_label, palette).truncate())
            .child(editor)
            .child(keys)
            .child(muted(
                if page.animation.tracks.is_empty() && !has_tracks(&page.animation.sequences) {
                    strings::NO_ANIMATIONS
                } else {
                    strings::ANIMATION_HINT
                },
                palette,
            ))
    }
}
