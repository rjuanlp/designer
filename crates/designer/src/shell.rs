//! The application shell: navigation, the design workspace and its panels.

use std::{cell::RefCell, rc::Rc};

use designer_core::{Document, Node, NodeId, NodeKind, Page, PageId, sample_document};
use designer_gpu_bridge::SharedFrame;
use gpui::{
    AnyElement, App, Bounds, Context, Div, ElementId, ExternalFencePoint, ExternalTexture,
    FocusHandle, Pixels, SharedString, Stateful, Window, actions, canvas, div, prelude::*, px, rgb,
    transparent_black,
};

use crate::{
    LaunchOptions,
    canvas::CanvasHost,
    strings::{self, NarrowPanel, Screen, Tool},
    theme::{self, LayoutMode, Palette, layout_mode, metrics},
};

actions!(
    designer,
    [
        /// Show or hide the timeline.
        ToggleTimeline,
        /// Switch between the light and dark themes.
        ToggleTheme,
        /// Move keyboard focus to the next control.
        FocusNext,
        /// Move keyboard focus to the previous control.
        FocusPrevious
    ]
);

/// Key context of the shell, used by the key bindings.
pub const KEY_CONTEXT: &str = "Shell";

/// Root view of the application.
pub struct Shell {
    document: Document,
    page: PageId,
    selection: Option<NodeId>,
    screen: Screen,
    tool: Tool,
    timeline_open: bool,
    dark: bool,
    narrow_panel: NarrowPanel,
    canvas: Rc<RefCell<CanvasHost>>,
    focus: FocusHandle,
}

impl Shell {
    /// Create the shell with the built-in sample document.
    pub fn new(options: &LaunchOptions, cx: &mut Context<Self>) -> Self {
        let document = sample_document();
        let page = document
            .pages
            .get(options.page)
            .or(document.pages.first())
            .map(|page| page.id)
            .unwrap_or(PageId(0));
        let selection = options.select.map(NodeId).filter(|id| {
            document
                .page(page)
                .is_some_and(|page| page.node(*id).is_some())
        });
        Self {
            document,
            page,
            selection,
            screen: options.screen,
            tool: Tool::Move,
            timeline_open: options.timeline,
            dark: !options.light,
            narrow_panel: options.panel,
            canvas: Rc::new(RefCell::new(CanvasHost::new())),
            focus: cx.focus_handle(),
        }
    }

    /// Focus handle of the shell root, focused when the window opens.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn palette(&self) -> Palette {
        if self.dark { theme::DARK } else { theme::LIGHT }
    }

    fn current_page(&self) -> Option<&Page> {
        self.document.page(self.page)
    }

    fn toggle_timeline(&mut self, _: &ToggleTimeline, _: &mut Window, cx: &mut Context<Self>) {
        self.timeline_open = !self.timeline_open;
        cx.notify();
    }

    fn toggle_theme(&mut self, _: &ToggleTheme, _: &mut Window, cx: &mut Context<Self>) {
        self.dark = !self.dark;
        cx.notify();
    }

    fn focus_next(&mut self, _: &FocusNext, window: &mut Window, _: &mut Context<Self>) {
        window.focus_next();
    }

    fn focus_previous(&mut self, _: &FocusPrevious, window: &mut Window, _: &mut Context<Self>) {
        window.focus_prev();
    }
}

/// A keyboard-reachable control with hover, active and focus styles.
fn control(id: impl Into<ElementId>, palette: &Palette, active: bool) -> Stateful<Div> {
    let (hover, selected, selected_text) = (palette.hover, palette.selected, palette.selected_text);
    div()
        .id(id)
        .tab_index(0)
        .flex()
        .flex_none()
        .items_center()
        .min_h(px(metrics::ROW_HEIGHT))
        .px(px(metrics::SPACE_S))
        .rounded(px(metrics::RADIUS))
        .border_1()
        .border_color(transparent_black())
        .cursor_pointer()
        .focus(|style| style.border_color(rgb(palette.focus_ring)))
        .when(active, |this| {
            this.bg(rgb(selected)).text_color(rgb(selected_text))
        })
        .when(!active, |this| this.hover(|style| style.bg(rgb(hover))))
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    palette: &Palette,
    active: bool,
) -> Stateful<Div> {
    control(id, palette, active).child(label.into())
}

fn section_title(label: &'static str, palette: &Palette) -> Div {
    div()
        .flex_none()
        .px(px(metrics::SPACE_M))
        .pt(px(metrics::SPACE_M))
        .pb(px(metrics::SPACE_XS))
        .text_size(px(metrics::TEXT_S))
        .text_color(rgb(palette.text_muted))
        .child(label)
}

fn muted(text: impl Into<SharedString>, palette: &Palette) -> Div {
    div().text_color(rgb(palette.text_muted)).child(text.into())
}

/// Format a document number without a trailing `.0`.
pub fn format_number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded}")
    }
}

fn flatten<'a>(nodes: &'a [Node], depth: usize, rows: &mut Vec<(usize, &'a Node)>) {
    // Topmost layer first, as painted last.
    for node in nodes.iter().rev() {
        rows.push((depth, node));
        flatten(&node.children, depth + 1, rows);
    }
}

fn to_external(frame: SharedFrame) -> ExternalTexture {
    ExternalTexture {
        id: frame.texture_id,
        shared_handle: frame.texture_handle,
        wait: Some(ExternalFencePoint {
            fence_id: frame.ready.fence_id,
            shared_handle: frame.ready.shared_handle,
            value: frame.ready.value,
        }),
        signal: Some(ExternalFencePoint {
            fence_id: frame.released.fence_id,
            shared_handle: frame.released.shared_handle,
            value: frame.released.value,
        }),
    }
}

impl Shell {
    fn navigation(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(metrics::SPACE_XS))
            .min_h(px(metrics::BAR_HEIGHT))
            .px(px(metrics::SPACE_S))
            .py(px(metrics::SPACE_XS))
            .bg(rgb(palette.panel))
            .border_b_1()
            .border_color(rgb(palette.border))
            .children(Screen::ALL.into_iter().enumerate().map(|(index, screen)| {
                button(
                    ("screen", index),
                    screen.label(),
                    palette,
                    self.screen == screen,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.screen = screen;
                    cx.notify();
                }))
            }))
    }

    fn tool_bar(&self, mode: LayoutMode, palette: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(metrics::SPACE_XS))
            .min_h(px(metrics::BAR_HEIGHT))
            .px(px(metrics::SPACE_S))
            .py(px(metrics::SPACE_XS))
            .bg(rgb(palette.panel))
            .border_b_1()
            .border_color(rgb(palette.border))
            .children(Tool::ALL.into_iter().enumerate().map(|(index, tool)| {
                button(("tool", index), tool.label(), palette, self.tool == tool).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.tool = tool;
                        cx.notify();
                    }),
                )
            }))
            .when(mode == LayoutMode::Wide, |this| {
                this.child(
                    muted(strings::TOOLS_NOTE, palette)
                        .pl(px(metrics::SPACE_M))
                        .text_size(px(metrics::TEXT_S)),
                )
            })
    }

    fn layers_panel(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        // Collected eagerly: a lazy iterator would keep `cx` borrowed while
        // the layer rows below need it.
        let pages: Vec<Stateful<Div>> = self
            .document
            .pages
            .iter()
            .map(|page| {
                let id = page.id;
                button(("page", id.0), page.name.clone(), palette, self.page == id)
                    .mx(px(metrics::SPACE_XS))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = id;
                        this.selection = None;
                        cx.notify();
                    }))
            })
            .collect();

        let mut rows = Vec::new();
        if let Some(page) = self.current_page() {
            flatten(&page.nodes, 0, &mut rows);
        }
        let layers: Vec<AnyElement> = if rows.is_empty() {
            vec![
                muted(strings::NO_LAYERS, palette)
                    .px(px(metrics::SPACE_M))
                    .py(px(metrics::SPACE_S))
                    .into_any_element(),
            ]
        } else {
            rows.into_iter()
                .map(|(depth, node)| self.layer_row(depth, node, palette, cx))
                .collect()
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(palette.panel))
            .child(section_title(strings::PAGES, palette))
            .children(pages)
            .child(section_title(strings::LAYERS, palette))
            .child(
                div()
                    .id("layers-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(layers),
            )
    }

    fn layer_row(
        &self,
        depth: usize,
        node: &Node,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = node.id;
        let selected = self.selection == Some(id);
        let state = |label: &'static str| {
            div()
                .flex_none()
                .text_size(px(metrics::TEXT_S))
                .when(!selected, |this| this.text_color(rgb(palette.text_muted)))
                .child(label)
        };
        control(("layer", id.0), palette, selected)
            .mx(px(metrics::SPACE_XS))
            .gap(px(metrics::SPACE_S))
            .pl(px(metrics::SPACE_S + metrics::INDENT * depth as f32))
            .when(!node.visible && !selected, |this| {
                this.text_color(rgb(palette.text_muted))
            })
            .child(
                // `min_w_0` lets a long name shrink and end in an ellipsis
                // instead of pushing the state labels out of the panel.
                div().flex_1().min_w_0().truncate().child(node.name.clone()),
            )
            .when(!node.visible, |this| this.child(state(strings::HIDDEN)))
            .when(node.locked, |this| this.child(state(strings::LOCKED)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selection = Some(id);
                cx.notify();
            }))
            .into_any_element()
    }

    fn inspector(&self, palette: &Palette) -> Div {
        let panel = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(palette.panel))
            .child(section_title(strings::PROPERTIES, palette));
        let node = self
            .selection
            .and_then(|id| self.current_page().and_then(|page| page.node(id)));
        let Some(node) = node else {
            return panel.child(
                muted(strings::NOTHING_SELECTED, palette)
                    .px(px(metrics::SPACE_M))
                    .py(px(metrics::SPACE_S)),
            );
        };

        let row = |label: &'static str, value: AnyElement| {
            div()
                .flex()
                .items_start()
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
        };
        let text = |value: String| div().child(value).into_any_element();
        let yes_no = |value: bool| if value { strings::YES } else { strings::NO };

        let fill = match node.fill {
            Some(color) => div()
                .flex()
                .items_center()
                .gap(px(metrics::SPACE_S))
                .child(
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
                .child(color.hex())
                .into_any_element(),
            None => muted(strings::NO_FILL, palette).into_any_element(),
        };

        let bounds = node.bounds;
        panel.child(
            div()
                .id("inspector-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(row(strings::NAME, text(node.name.clone())))
                .child(row(
                    strings::KIND,
                    text(strings::kind_label(&node.kind).to_owned()),
                ))
                .child(row(strings::POSITION_X, text(format_number(bounds.x))))
                .child(row(strings::POSITION_Y, text(format_number(bounds.y))))
                .child(row(strings::WIDTH, text(format_number(bounds.width))))
                .child(row(strings::HEIGHT, text(format_number(bounds.height))))
                .when_some(
                    match node.kind {
                        NodeKind::Rectangle { corner_radius } => Some(corner_radius),
                        _ => None,
                    },
                    |this, radius| this.child(row(strings::RADIUS, text(format_number(radius)))),
                )
                .child(row(strings::FILL, fill))
                .when_some(node.fill, |this, color| {
                    this.child(row(
                        strings::OPACITY,
                        text(format!("{} %", color.opacity_percent())),
                    ))
                })
                .child(row(strings::VISIBLE, text(yes_no(node.visible).to_owned())))
                .child(row(strings::LOCKED, text(yes_no(node.locked).to_owned())))
                .when_some(
                    match &node.kind {
                        NodeKind::Text { content } => Some(content.clone()),
                        _ => None,
                    },
                    |this, content| {
                        this.child(row(strings::CONTENT, text(content))).child(
                            muted(strings::TEXT_NOT_RENDERED, palette)
                                .px(px(metrics::SPACE_M))
                                .py(px(metrics::SPACE_XS))
                                .text_size(px(metrics::TEXT_S)),
                        )
                    },
                )
                .child(
                    muted(strings::READ_ONLY_NOTE, palette)
                        .px(px(metrics::SPACE_M))
                        .py(px(metrics::SPACE_M))
                        .text_size(px(metrics::TEXT_S)),
                ),
        )
    }

    fn canvas_panel(&self, palette: &Palette) -> Div {
        let panel = div().relative().size_full().overflow_hidden();
        let centered = |children: Vec<AnyElement>| {
            div()
                .absolute()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(metrics::SPACE_S))
                .p(px(metrics::SPACE_L))
                .children(children)
        };
        if let Some(error) = self.canvas.borrow().error() {
            return panel.child(centered(vec![
                div().child(strings::CANVAS_UNAVAILABLE).into_any_element(),
                muted(error.to_owned(), palette).into_any_element(),
            ]));
        }
        let Some(page) = self.current_page().cloned() else {
            return panel;
        };
        let empty = page.nodes.is_empty();
        let selection = self.selection;
        let host = self.canvas.clone();
        let vello_canvas = canvas(
            |_, _, _| (),
            move |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
                let scale = window.scale_factor();
                let width = (f32::from(bounds.size.width) * scale).round() as u32;
                let height = (f32::from(bounds.size.height) * scale).round() as u32;
                if width == 0 || height == 0 {
                    return;
                }
                match host.borrow_mut().frame(&page, selection, width, height) {
                    Ok(result) => {
                        if let Some(frame) = result.frame {
                            window.paint_external_texture(bounds, to_external(frame));
                        }
                        if result.retry {
                            window.request_animation_frame();
                        }
                    }
                    Err(error) => {
                        log::error!("Canvas rendering failed: {error:#}");
                        // Render again so the panel shows the stored error.
                        window.refresh();
                    }
                }
            },
        )
        .absolute()
        .size_full();
        panel.child(vello_canvas).when(empty, |this| {
            this.child(centered(vec![
                // The page background comes from the document, so use a
                // label with its own surface to stay readable on it.
                div()
                    .px(px(metrics::SPACE_M))
                    .py(px(metrics::SPACE_S))
                    .rounded(px(metrics::RADIUS))
                    .bg(rgb(palette.panel))
                    .text_color(rgb(palette.text_muted))
                    .child(strings::EMPTY_PAGE)
                    .into_any_element(),
            ]))
        })
    }

    fn timeline(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let label = if self.timeline_open {
            strings::HIDE_TIMELINE
        } else {
            strings::SHOW_TIMELINE
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .when(self.timeline_open, |this| {
                this.h(px(metrics::TIMELINE_HEIGHT))
            })
            .bg(rgb(palette.panel))
            .border_t_1()
            .border_color(rgb(palette.border))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .gap(px(metrics::SPACE_S))
                    .min_h(px(metrics::BAR_HEIGHT))
                    .px(px(metrics::SPACE_S))
                    .child(div().px(px(metrics::SPACE_XS)).child(strings::TIMELINE))
                    .child(
                        button("timeline-toggle", label, palette, false).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.timeline_open = !this.timeline_open;
                                cx.notify();
                            },
                        )),
                    ),
            )
            .when(self.timeline_open, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .p(px(metrics::SPACE_M))
                        .child(muted(strings::NO_ANIMATIONS, palette)),
                )
            })
    }

    fn workspace(&self, mode: LayoutMode, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let body = match mode {
            LayoutMode::Wide => div()
                .flex()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex_none()
                        .w(px(metrics::LAYERS_WIDTH))
                        .h_full()
                        .border_r_1()
                        .border_color(rgb(palette.border))
                        .child(self.layers_panel(palette, cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.canvas_panel(palette)),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(metrics::INSPECTOR_WIDTH))
                        .h_full()
                        .border_l_1()
                        .border_color(rgb(palette.border))
                        .child(self.inspector(palette)),
                ),
            LayoutMode::Narrow => {
                let switcher =
                    div()
                        .flex()
                        .flex_none()
                        .gap(px(metrics::SPACE_XS))
                        .px(px(metrics::SPACE_S))
                        .py(px(metrics::SPACE_XS))
                        .bg(rgb(palette.panel))
                        .border_b_1()
                        .border_color(rgb(palette.border))
                        .children(NarrowPanel::ALL.into_iter().enumerate().map(
                            |(index, panel)| {
                                button(
                                    ("panel", index),
                                    panel.label(),
                                    palette,
                                    self.narrow_panel == panel,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.narrow_panel = panel;
                                        cx.notify();
                                    },
                                ))
                            },
                        ));
                let panel = match self.narrow_panel {
                    NarrowPanel::Layers => self.layers_panel(palette, cx),
                    NarrowPanel::Canvas => self.canvas_panel(palette),
                    NarrowPanel::Inspector => self.inspector(palette),
                };
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(switcher)
                    .child(div().flex_1().min_h_0().child(panel))
            }
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(self.tool_bar(mode, palette, cx))
            .child(body)
            .child(self.timeline(palette, cx))
    }

    fn page_frame(&self, title: &'static str) -> Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(metrics::SPACE_M))
            .p(px(metrics::SPACE_L))
            .child(div().text_size(px(metrics::TEXT_L)).child(title))
    }

    fn documents(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        self.page_frame(strings::RECENT_DOCUMENTS)
            .child(muted(strings::NO_RECENT_DOCUMENTS, palette))
            .child(div().flex().child(
                button("open-sample", strings::OPEN_SAMPLE, palette, true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.screen = Screen::Design;
                        cx.notify();
                    },
                )),
            ))
    }

    fn pending(&self, screen: Screen, palette: &Palette) -> Div {
        self.page_frame(screen.label())
            .child(div().child(strings::PENDING_TITLE))
            .child(muted(screen.pending_description(), palette))
    }

    fn preferences(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let heading = |label: &'static str| div().pt(px(metrics::SPACE_M)).child(label);
        let themes = [(strings::THEME_DARK, true), (strings::THEME_LIGHT, false)];
        let host = self.canvas.borrow();
        let adapter = match (host.adapter(), host.error()) {
            (Some(name), _) => div().child(name.to_owned()),
            (None, error) => div()
                .flex()
                .flex_col()
                .gap(px(metrics::SPACE_XS))
                .child(strings::GRAPHICS_UNAVAILABLE)
                .children(error.map(|error| muted(error.to_owned(), palette))),
        };
        self.page_frame(Screen::Preferences.label())
            .child(heading(strings::APPEARANCE))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(metrics::SPACE_S))
                    .child(muted(strings::THEME, palette))
                    .children(
                        themes
                            .into_iter()
                            .enumerate()
                            .map(|(index, (label, dark))| {
                                button(("theme", index), label, palette, self.dark == dark)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.dark = dark;
                                        cx.notify();
                                    }))
                            }),
                    ),
            )
            .child(heading(strings::GRAPHICS))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(metrics::SPACE_S))
                    .child(muted(strings::GRAPHICS_ADAPTER, palette))
                    .child(adapter),
            )
            .child(heading(strings::AGENTS))
            .child(muted(strings::AGENTS_PENDING, palette))
            .child(heading(strings::SHORTCUTS))
            .children(strings::SHORTCUT_LIST.into_iter().map(|(keys, action)| {
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(metrics::SPACE_S))
                    .child(div().child(keys))
                    .child(muted(action, palette))
            }))
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let mode = layout_mode(f32::from(window.viewport_size().width));
        let content = match self.screen {
            Screen::Documents => self.documents(&palette, cx),
            Screen::Design => self.workspace(mode, &palette, cx),
            Screen::Preferences => self.preferences(&palette, cx),
            screen => self.pending(screen, &palette),
        };
        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::toggle_timeline))
            .on_action(cx.listener(Self::toggle_theme))
            .on_action(cx.listener(Self::focus_next))
            .on_action(cx.listener(Self::focus_previous))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.app))
            .text_color(rgb(palette.text))
            .text_size(px(metrics::TEXT_M))
            .child(self.navigation(&palette, cx))
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_drop_a_useless_fraction_and_keep_two_decimals() {
        assert_eq!(format_number(120.0), "120");
        assert_eq!(format_number(-0.5), "-0.5");
        assert_eq!(format_number(1.0 / 3.0), "0.33");
        assert_eq!(format_number(2.999), "3");
    }

    #[test]
    fn layer_rows_list_the_topmost_layer_first_with_depth() {
        let document = sample_document();
        let page = &document.pages[0];
        let mut rows = Vec::new();
        flatten(&page.nodes, 0, &mut rows);
        assert_eq!(rows.len(), page.node_count());
        // The last top-level node is painted on top, so it is listed first.
        assert_eq!(rows[0].1.id, page.nodes.last().expect("nodes").id);
        assert_eq!(rows[0].0, 0);
        assert!(rows.iter().any(|(depth, _)| *depth == 2));
    }
}
