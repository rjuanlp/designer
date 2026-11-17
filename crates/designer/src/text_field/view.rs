//! GPUI single-line input backed by the pure editing state.
use super::EditState;
use crate::theme::{Palette, metrics};
use gpui::{prelude::*, *};
use std::ops::Range;
type Commit = dyn Fn(String, &mut Window, &mut App) -> String;

pub(crate) struct FieldView {
    pub(crate) focus: FocusHandle,
    state: EditState,
    palette: Palette,
    line: Option<ShapedLine>,
    bounds: Bounds<Pixels>,
    scroll: Pixels,
    commit: Box<Commit>,
}

impl FieldView {
    pub(crate) fn new(
        value: String,
        palette: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
        commit: impl Fn(String, &mut Window, &mut App) -> String + 'static,
    ) -> Self {
        let focus = cx.focus_handle();
        // Commit after the frame that reports the blur: a change notified
        // while the window is drawing is not painted until the next event.
        cx.on_blur(&focus, window, |_, window, cx| {
            let field = cx.entity();
            window.defer(cx, move |window, cx| {
                field.update(cx, |this, cx| this.accept(window, cx));
            });
        })
        .detach();
        Self {
            focus,
            state: EditState::new(value),
            palette,
            line: None,
            bounds: Bounds::default(),
            scroll: px(0.),
            commit: Box::new(commit),
        }
    }
    fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.text != self.state.original {
            let value = self.state.commit();
            self.state = EditState::new((self.commit)(value, window, cx));
        }
    }
    pub(crate) fn take_pending(&mut self) -> Option<String> {
        (self.state.text != self.state.original).then(|| self.state.commit())
    }
    pub(crate) fn sync(&mut self, value: String, palette: Palette) {
        self.palette = palette;
        // Typed text survives until it is committed or cancelled: the shell
        // renders once between losing focus and the blur notification, and
        // resetting an unfocused field here would drop the edit.
        if self.state.original != value {
            self.state = EditState::new(value);
        }
    }
    fn index(&self, point: Point<Pixels>) -> usize {
        self.line.as_ref().map_or(0, |line| {
            line.closest_index_for_x(point.x - self.bounds.left() + self.scroll)
        })
    }
}

impl Render for FieldView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let focus = self.focus.clone();
        let text = self.state.text.clone();
        let selection = self.state.selection();
        let caret = self.state.caret;
        let palette = self.palette;
        let focused = focus.is_focused(window);
        div()
            .id("text-field")
            .track_focus(&self.focus)
            .key_context("TextField")
            .border_1()
            .border_color(rgb(if focused {
                palette.focus_ring
            } else {
                palette.border
            }))
            .bg(rgb(palette.app))
            .h(px(metrics::ROW_HEIGHT))
            .w_full()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus);
                    let index = this.index(event.position);
                    this.state.caret = index;
                    if !event.modifiers.shift {
                        this.state.anchor = index;
                    }
                    this.state.unmark();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let shift = event.keystroke.modifiers.shift;
                let ctrl = event.keystroke.modifiers.control;
                if key == "tab" {
                    return;
                }
                match key {
                    "enter" => this.accept(window, cx),
                    "escape" => this.state.cancel(),
                    "left" => this.state.move_caret(false, shift),
                    "right" => this.state.move_caret(true, shift),
                    "home" | "end" => {
                        this.state.caret = if key == "home" {
                            0
                        } else {
                            this.state.text.len()
                        };
                        if !shift {
                            this.state.anchor = this.state.caret;
                        }
                    }
                    "backspace" => this.state.delete(false),
                    "delete" => this.state.delete(true),
                    "a" if ctrl => this.state.select_all(),
                    "c" | "x" if ctrl => {
                        let selected = this.state.selection();
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            this.state.text[selected].to_owned(),
                        ));
                        if key == "x" {
                            this.state.replace(None, "", false);
                        }
                    }
                    "v" if ctrl => {
                        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                            this.state.replace(None, &text, false);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let style = window.text_style();
                        let line = window.text_system().shape_line(
                            text.clone().into(),
                            px(metrics::TEXT_M),
                            &[TextRun {
                                len: text.len(),
                                font: style.font(),
                                color: rgb(palette.text).into(),
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            }],
                            None,
                        );
                        let scroll = if focused {
                            (line.x_for_index(caret) - bounds.size.width + px(metrics::SPACE_S))
                                .max(px(0.))
                        } else {
                            px(0.)
                        };
                        entity.update(cx, |this, _| {
                            this.line = Some(line.clone());
                            this.bounds = bounds;
                            this.scroll = scroll;
                        });
                        (line, entity, focus, scroll)
                    },
                    move |bounds, (line, entity, focus, scroll), window, cx| {
                        if focused {
                            let x = line.x_for_index(selection.start) - scroll;
                            let end = line.x_for_index(selection.end) - scroll;
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.left() + x, bounds.top()),
                                    size(end - x, bounds.size.height),
                                ),
                                rgb(palette.selected),
                            ));
                            let x = line.x_for_index(caret) - scroll;
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.left() + x, bounds.top()),
                                    size(px(1.), bounds.size.height),
                                ),
                                rgb(palette.text),
                            ));
                        }
                        let _ = line.paint(
                            point(bounds.left() - scroll, bounds.top()),
                            px(metrics::ROW_HEIGHT),
                            window,
                            cx,
                        );
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                    },
                )
                .size_full(),
            )
    }
}

impl EntityInputHandler for FieldView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let start = self.state.byte_offset(range.start);
        let end = self.state.byte_offset(range.end).max(start);
        *adjusted = Some(
            self.state.text[..start].encode_utf16().count()
                ..self.state.text[..end].encode_utf16().count(),
        );
        Some(self.state.text[start..end].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.state.selection_utf16(),
            reversed: self.state.caret < self.state.anchor,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.state.marked_utf16()
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.state.unmark();
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.replace(range, text, false);
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.replace(range, text, true);
        if let (Some(mark), Some(selected)) = (self.state.marked_utf16(), selected) {
            self.state.anchor = self.state.byte_offset(mark.start + selected.start);
            self.state.caret = self.state.byte_offset(mark.start + selected.end);
        }
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.line.as_ref()?;
        let x = line.x_for_index(self.state.byte_offset(range.start)) - self.scroll;
        let end = line.x_for_index(self.state.byte_offset(range.end)) - self.scroll;
        Some(Bounds::new(
            point(bounds.left() + x, bounds.top()),
            size(end - x, bounds.size.height),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.state.text[..self.index(point)].encode_utf16().count())
    }
}
