//! The application shell: navigation, the design workspace and its panels.

mod timeline_view;

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Instant,
};

use crate::accessibility::*;
use accesskit::{Action as A11yAction, ActionRequest, NodeId as A11yNodeId};
use accesskit::{Rect as A11yRect, TreeUpdate};
use designer_core::{
    Color, DEFAULT_PROTOTYPE_DURATION_MS, Document, Easing, FillOverride, INITIAL_STATE,
    MAX_PROTOTYPE_STATES, Node, NodeId, NodeKind, NodeOverride, Operation, Page, PageId,
    PrototypeState, PrototypeStateId, PrototypeTransition, PrototypeTrigger, Rect, sample_document,
};
use designer_engine::{
    Session,
    bridge::{SessionReply, SessionRequest, execute_session_request},
    dsg,
    import::{self, ImportReport, ImportResult},
};
use designer_gpu_bridge::SharedFrame;
use designer_render::document::hit_test;
use gpui::{
    AnyElement, App, Bounds, Context, Div, ElementId, ExternalFencePoint, ExternalTexture,
    FocusHandle, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathPromptOptions,
    Pixels, Point, PromptLevel, SharedString, Stateful, Window, actions, canvas, div, prelude::*,
    px, rgb, transparent_black,
};

use crate::{
    LaunchOptions,
    canvas::CanvasHost,
    prototype_preview::PrototypePreview,
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
        FocusPrevious,
        /// Undo the last change.
        Undo,
        /// Redo the change undone last.
        Redo,
        /// Save the document to its file.
        Save,
        /// Save the document to a new file.
        SaveAs,
        /// Open a document file.
        Open,
        /// Start a new, empty document.
        NewDocument,
        /// Delete the selected layer.
        DeleteSelection,
        /// Cancel the current drag, or clear the selection.
        Cancel
    ]
);

/// Key context of the shell, used by the key bindings.
pub const KEY_CONTEXT: &str = "Shell";

/// Fill of a new rectangle or ellipse. Document data, not an interface token.
const SHAPE_FILL: Color = Color::rgb(196, 196, 196);
/// Fill of a new frame.
const FRAME_FILL: Color = Color::rgb(255, 255, 255);
/// Size of a shape created by a click without a drag, in document units.
const CLICK_SIZE: f64 = 100.0;

/// A pointer drag on the canvas, in page coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// Drawing a new node with the current tool.
    Create {
        /// Where the drag started.
        start: (f64, f64),
    },
    /// Moving an existing node.
    Move {
        /// The node being moved.
        node: NodeId,
        /// Where the drag started.
        start: (f64, f64),
        /// Bounds of the node when the drag started.
        original: Rect,
    },
}

/// Something that replaces or closes the document, and so may need the user
/// to confirm losing unsaved changes first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    New,
    Sample,
    Open,
    Import,
    Close,
}

/// The rectangle between two points, snapped to whole document units.
fn drawn_rect(start: (f64, f64), end: (f64, f64)) -> Rect {
    let (left, right) = (start.0.min(end.0).round(), start.0.max(end.0).round());
    let (top, bottom) = (start.1.min(end.1).round(), start.1.max(end.1).round());
    Rect::new(left, top, right - left, bottom - top)
}

/// The operations a drag stands for at pointer position `point`, if any.
///
/// While the pointer is still down (`finished` is false) a drag that has not
/// moved produces nothing. A finished create drag that never moved produces
/// a shape of [`CLICK_SIZE`] at the click.
fn drag_operations(
    drag: Drag,
    point: (f64, f64),
    finished: bool,
    tool: Tool,
    page: PageId,
    next_node_id: u64,
) -> Option<Vec<Operation>> {
    match drag {
        Drag::Create { start } => {
            let (kind, fill) = match tool {
                Tool::Frame => (NodeKind::Frame, FRAME_FILL),
                Tool::Rectangle => (NodeKind::Rectangle { corner_radius: 0.0 }, SHAPE_FILL),
                Tool::Ellipse => (NodeKind::Ellipse, SHAPE_FILL),
                Tool::Move | Tool::Text | Tool::Pen => return None,
            };
            let mut bounds = drawn_rect(start, point);
            if bounds.width < 1.0 && bounds.height < 1.0 {
                if !finished {
                    return None;
                }
                bounds = Rect::new(bounds.x, bounds.y, CLICK_SIZE, CLICK_SIZE);
            }
            bounds.width = bounds.width.max(1.0);
            bounds.height = bounds.height.max(1.0);
            Some(vec![Operation::AddNode {
                page,
                parent: None,
                name: format!("{} {next_node_id}", strings::kind_label(&kind)),
                kind,
                bounds,
                fill: Some(fill),
            }])
        }
        Drag::Move {
            node,
            start,
            original,
        } => {
            let dx = (point.0 - start.0).round();
            let dy = (point.1 - start.1).round();
            (dx != 0.0 || dy != 0.0).then(|| {
                vec![Operation::SetBounds {
                    page,
                    node,
                    bounds: original.translated(dx, dy),
                }]
            })
        }
    }
}

/// Add the `.dsg` extension unless the path already has it.
fn with_extension(path: PathBuf) -> PathBuf {
    let has_it = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(dsg::EXTENSION));
    if has_it {
        return path;
    }
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".");
    name.push(dsg::EXTENSION);
    path.with_file_name(name)
}

/// Root view of the application.
pub struct Shell {
    session: Session,
    /// The uncommitted result of the drag in progress, shown instead of the
    /// session document.
    preview: Option<Document>,
    drag: Option<Drag>,
    prompt_pending: bool,
    a11y_focus: A11yNodeId,
    a11y_cache: Option<TreeUpdate>,
    /// Changes whenever the canvas content or the selection changes.
    generation: u64,
    /// Changes whenever the canvas view must be fitted again.
    view_epoch: u64,
    status: Option<SharedString>,
    import_preview: Option<ImportResult>,
    import_report: Option<ImportReport>,
    pending_import: Option<ImportResult>,
    window_title: String,
    page: PageId,
    selection: Option<NodeId>,
    screen: Screen,
    tool: Tool,
    timeline_open: bool,
    animation: crate::timeline::Timeline,
    prototype_preview: PrototypePreview,
    dark: bool,
    narrow_panel: NarrowPanel,
    canvas: Rc<RefCell<CanvasHost>>,
    focus: FocusHandle,
    accessibility_sender: tokio::sync::mpsc::UnboundedSender<ActionRequest>,
}

impl Shell {
    /// Create the shell with the document named on the command line, or the
    /// built-in sample document.
    pub fn new(options: &LaunchOptions, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut status = None;
        let session = match &options.open {
            Some(path) => Session::open(path).unwrap_or_else(|error| {
                status = Some(strings::open_failed(path, &error).into());
                Session::new(Document::empty(strings::UNTITLED))
            }),
            None => Session::new(sample_document()),
        };
        let document = session.document();
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
        let prototype_preview = PrototypePreview::new(
            document
                .page(page)
                .expect("a validated document has its selected page"),
        );
        let (bridge_sender, mut bridge_receiver) =
            tokio::sync::mpsc::unbounded_channel::<crate::bridge::PendingRequest>();
        if let Err(error) = crate::bridge::start_pipe_server(bridge_sender) {
            status = Some(strings::mcp_server_failed(&error).into());
        }
        cx.spawn(async move |this, cx| {
            while let Some(request) = bridge_receiver.recv().await {
                let reply = this
                    .update(cx, |this, cx| this.handle_mcp_request(request.command, cx))
                    .unwrap_or_else(|error| SessionReply::Error {
                        error: format!("Designer window is unavailable: {error}"),
                    });
                let _ = request.reply.send(reply);
            }
        })
        .detach();
        let (accessibility_sender, mut accessibility_receiver) =
            tokio::sync::mpsc::unbounded_channel();
        cx.spawn_in(window, async move |this, cx| {
            while let Some(action) = accessibility_receiver.recv().await {
                if let Err(error) = this.update_in(cx, |this, window, cx| {
                    this.handle_accessibility_action(action, window, cx)
                }) {
                    eprintln!("Screen-reader action could not reach Designer: {error}");
                    break;
                }
            }
        })
        .detach();
        Self {
            session,
            preview: None,
            drag: None,
            prompt_pending: false,
            a11y_focus: A11yNodeId(0),
            a11y_cache: None,
            generation: 0,
            view_epoch: 0,
            status,
            import_preview: None,
            import_report: None,
            pending_import: None,
            window_title: String::new(),
            page,
            selection,
            screen: options.screen,
            tool: Tool::Move,
            timeline_open: options.timeline,
            animation: crate::timeline::Timeline::default(),
            prototype_preview,
            dark: !options.light,
            narrow_panel: options.panel,
            canvas: Rc::new(RefCell::new(CanvasHost::new())),
            focus: cx.focus_handle(),
            accessibility_sender,
        }
    }

    /// Focus handle of the shell root, focused when the window opens.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn palette(&self) -> Palette {
        if self.dark { theme::DARK } else { theme::LIGHT }
    }

    /// The document to show: the drag preview if there is one.
    fn document(&self) -> &Document {
        self.preview.as_ref().unwrap_or(self.session.document())
    }

    fn current_page(&self) -> Option<&Page> {
        self.document().page(self.page)
    }

    /// The selection to show. While a new node is being drawn it is the
    /// node of the preview.
    fn shown_selection(&self) -> Option<NodeId> {
        match (self.drag, &self.preview) {
            (Some(Drag::Create { .. }), Some(_)) => {
                Some(NodeId(self.session.document().next_node_id))
            }
            _ => self.selection,
        }
    }

    /// Repaint after the canvas content or the selection changed.
    fn redraw(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        cx.notify();
    }

    fn title(&self) -> String {
        let name = match self.session.path().and_then(Path::file_name) {
            Some(file) => file.to_string_lossy().into_owned(),
            None => self.session.document().name.clone(),
        };
        let mark = if self.session.is_dirty() { " •" } else { "" };
        format!("{name}{mark}")
    }

    fn replace_session(&mut self, session: Session, cx: &mut Context<Self>) {
        self.animation = crate::timeline::Timeline::default();
        self.page = session
            .document()
            .pages
            .first()
            .map_or(PageId(0), |page| page.id);
        self.session = session.succeeding(&self.session);
        if let Some(page) = self.session.document().page(self.page) {
            self.prototype_preview = PrototypePreview::new(page);
        }
        self.selection = None;
        self.drag = None;
        self.preview = None;
        self.import_preview = None;
        self.tool = Tool::Move;
        self.view_epoch += 1;
        self.screen = Screen::Design;
        self.redraw(cx);
    }

    fn handle_mcp_request(
        &mut self,
        request: SessionRequest,
        cx: &mut Context<Self>,
    ) -> SessionReply {
        let replace_document = matches!(
            &request,
            SessionRequest::NewDocument { .. } | SessionRequest::ImportFigmaJson { .. }
        );
        let changes_session = !matches!(
            &request,
            SessionRequest::ReadDocument
                | SessionRequest::DescribeCapabilities
                | SessionRequest::RenderSnapshot { .. }
        );
        if changes_session && (self.drag.is_some() || self.prompt_pending) {
            return SessionReply::Error {
                error: concat!(
                    "Designer is busy with a canvas drag or unsaved-changes prompt; ",
                    "finish it before retrying"
                )
                .into(),
            };
        }
        match execute_session_request(&mut self.session, request) {
            Ok(result) => {
                if changes_session {
                    if replace_document {
                        self.page = self
                            .session
                            .document()
                            .pages
                            .first()
                            .map_or(PageId(0), |page| page.id);
                        self.selection = None;
                        self.animation = crate::timeline::Timeline::default();
                        self.screen = Screen::Design;
                        self.import_preview = None;
                        self.import_report = None;
                        self.pending_import = None;
                    }
                    self.status = Some(strings::MCP_UPDATED.into());
                    self.settle(cx);
                }
                SessionReply::Ok { result }
            }
            Err(error) => SessionReply::Error { error },
        }
    }

    fn handle_accessibility_action(
        &mut self,
        request: ActionRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.a11y_cache.as_ref().is_some_and(|tree| {
            tree.nodes.iter().any(|(id, node)| {
                *id == request.target_node && node.supports_action(request.action)
            })
        }) {
            return;
        }
        self.a11y_focus = request.target_node;
        if request.action == A11yAction::Focus {
            cx.notify();
            return;
        }
        if request.action != A11yAction::Click {
            return;
        }
        let target = request.target_node.0;
        match target {
            A11Y_NEW => {
                self.guarded(Pending::New, window, cx);
                return;
            }
            A11Y_OPEN => {
                self.guarded(Pending::Open, window, cx);
                return;
            }
            A11Y_SAVE_AS => {
                self.save_as(cx);
                return;
            }
            A11Y_SAMPLE => {
                self.guarded(Pending::Sample, window, cx);
                return;
            }
            A11Y_TIMELINE => {
                self.toggle_timeline(&ToggleTimeline, window, cx);
                return;
            }
            A11Y_PLAY => {
                self.play_animation(cx);
                return;
            }
            A11Y_HOVER => {
                self.create_prototype_state(PrototypeTrigger::Hover, cx);
                return;
            }
            A11Y_CLICK => {
                self.create_prototype_state(PrototypeTrigger::Click, cx);
                return;
            }
            A11Y_REMOVE_STATE => {
                self.remove_prototype_state(self.prototype_preview.state(), cx);
                return;
            }
            A11Y_IMPORT_PICK => {
                self.open_import(cx);
                return;
            }
            A11Y_IMPORT_APPLY => {
                self.apply_import(window, cx);
                return;
            }
            A11Y_THEME => {
                self.toggle_theme(&ToggleTheme, window, cx);
                return;
            }
            A11Y_PANEL_BASE..=A11Y_PANEL_LAST => {
                self.select_panel(NarrowPanel::ALL[(target - A11Y_PANEL_BASE) as usize], cx);
                return;
            }
            _ => {}
        }
        if let Some(id) = target.checked_sub(A11Y_STATE_BASE) {
            self.select_prototype_state(PrototypeStateId(id), cx);
            return;
        }
        if (A11Y_NAV_BASE..A11Y_NAV_BASE + Screen::ALL.len() as u64).contains(&target) {
            let screen = Screen::ALL[(target - A11Y_NAV_BASE) as usize];
            self.select_screen(screen, cx);
        } else if (A11Y_TOOL_BASE..A11Y_TOOL_BASE + Tool::ALL.len() as u64).contains(&target) {
            self.select_tool(Tool::ALL[(target - A11Y_TOOL_BASE) as usize], cx);
        } else if target == A11Y_UNDO {
            self.undo(&Undo, window, cx);
        } else if target == A11Y_REDO {
            self.redo(&Redo, window, cx);
        } else if target == A11Y_DELETE {
            self.delete_selection(&DeleteSelection, window, cx);
        } else if let Some(id) = target
            .checked_sub(A11Y_LAYER_BASE)
            .map(NodeId)
            .filter(|id| {
                self.current_page()
                    .is_some_and(|page| page.node(*id).is_some())
            })
        {
            self.select_layer(id, cx);
        }
    }

    fn accessibility_tree(&self, window: &Window) -> TreeUpdate {
        let bounds = window.bounds();
        build_accessibility_tree(A11ySnapshot {
            dark: self.dark,
            timeline: self.timeline_open,
            playing: self.animation.is_playing(),
            time_ms: self.animation.time_ms,
            can_play: self
                .current_page()
                .is_some_and(|p| timeline_view::has_tracks(&p.animation)),
            narrow: layout_mode(f32::from(window.bounds().size.width)) == LayoutMode::Narrow,
            panel: self.narrow_panel,
            prototype_state: self.prototype_preview.state(),
            can_import: self.import_preview.is_some(),
            import_summary: self
                .import_preview
                .as_ref()
                .map(|r| &r.report)
                .or(self.import_report.as_ref())
                .map_or_else(
                    || strings::IMPORT_NO_RESULT.to_owned(),
                    |r| strings::import_summary(r.imported_nodes, import_warning_count(r)),
                ),
            focus: self.a11y_focus,
            screen: self.screen,
            tool: self.tool,
            selection: self.selection,
            page: self.current_page(),
            can_undo: self.session.can_undo(),
            can_redo: self.session.can_redo(),
            status: self.status.as_deref().map(|status| &**status),
            title: format!("{} — {}", self.title(), strings::APP_NAME),
            bounds: A11yRect::new(
                f64::from(f32::from(bounds.origin.x)),
                f64::from(f32::from(bounds.origin.y)),
                f64::from(f32::from(bounds.origin.x + bounds.size.width)),
                f64::from(f32::from(bounds.origin.y + bounds.size.height)),
            ),
        })
    }

    /// Drop a selection that no longer exists, as after undo.
    fn settle(&mut self, cx: &mut Context<Self>) {
        self.animation.pause();
        self.drag = None;
        self.preview = None;
        let exists = self.selection.is_some_and(|id| {
            self.current_page()
                .is_some_and(|page| page.node(id).is_some())
        });
        if !exists {
            self.selection = None;
        }
        if let Some(page) = self.current_page() {
            self.prototype_preview = PrototypePreview::new(page);
        }
        if self.animation.active_sequence.is_some_and(|id| {
            !self
                .current_page()
                .is_some_and(|page| page.animation.sequence(id).is_some())
        }) {
            self.animation.active_sequence = None;
        }
        self.redraw(cx);
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_UNDO);
        if self.session.undo() {
            self.status = None;
        }
        self.settle(cx);
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_REDO);
        if self.session.redo() {
            self.status = None;
        }
        self.settle(cx);
    }

    fn delete_selection(&mut self, _: &DeleteSelection, _: &mut Window, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_DELETE);
        let Some(node) = self.selection else {
            return;
        };
        let page = self.page;
        self.status = self
            .session
            .apply(&[Operation::DeleteNode { page, node }])
            .err()
            .map(|error| strings::change_failed(&error).into());
        self.settle(cx);
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.take().is_none() {
            self.selection = None;
        }
        self.preview = None;
        self.redraw(cx);
    }

    fn save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        match self.session.save() {
            Ok(true) => self.status = Some(strings::saved(self.session.path()).into()),
            Ok(false) => return self.save_as(cx),
            Err(error) => self.status = Some(strings::save_failed(&error).into()),
        }
        cx.notify();
    }

    fn save_as_action(&mut self, _: &SaveAs, _: &mut Window, cx: &mut Context<Self>) {
        self.save_as(cx);
    }

    fn save_as(&mut self, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_SAVE_AS);
        let (directory, name) = match self.session.path() {
            Some(path) => (
                path.parent().map(Path::to_owned),
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned()),
            ),
            None => (None, None),
        };
        let directory = directory
            .or_else(std::env::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let name =
            name.unwrap_or_else(|| format!("{}.{}", self.session.document().name, dsg::EXTENSION));
        let chosen = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn(async move |this, cx| {
            let result = chosen.await;
            this.update(cx, |this, cx| {
                this.status = match result {
                    Ok(Ok(Some(path))) => {
                        let path = with_extension(path);
                        Some(match this.session.save_as(&path) {
                            Ok(()) => strings::saved(Some(&path)).into(),
                            Err(error) => strings::save_failed(&error).into(),
                        })
                    }
                    Ok(Ok(None)) | Err(_) => return,
                    Ok(Err(error)) => Some(strings::dialog_failed(&error).into()),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_dialog(&mut self, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let result = chosen.await;
            this.update(cx, |this, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.first() {
                        this.open_path(path, cx);
                    }
                }
                Ok(Ok(None)) | Err(_) => {}
                Ok(Err(error)) => {
                    this.status = Some(strings::dialog_failed(&error).into());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn open_import(&mut self, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let result = chosen.await;
            match result {
                Ok(Ok(Some(paths))) => {
                    let Some(path) = paths.first().cloned() else {
                        return;
                    };
                    this.update(cx, |this, cx| {
                        this.status = Some(strings::IMPORT_LOADING.into());
                        this.import_preview = None;
                        this.import_report = None;
                        cx.notify();
                    })
                    .ok();
                    let imported = cx
                        .background_spawn(async move { import::import_figma_file(&path) })
                        .await;
                    this.update(cx, |this, cx| match imported {
                        Ok(imported) => {
                            this.status = Some(
                                strings::import_summary(
                                    imported.report.imported_nodes,
                                    import_warning_count(&imported.report),
                                )
                                .into(),
                            );
                            this.import_report = None;
                            this.import_preview = Some(imported);
                            this.screen = Screen::Import;
                            this.redraw(cx);
                        }
                        Err(error) => {
                            this.status = Some(strings::import_failed(&error).into());
                            cx.notify();
                        }
                    })
                    .ok();
                }
                Ok(Ok(None)) | Err(_) => {}
                Ok(Err(error)) => {
                    this.update(cx, |this, cx| {
                        this.status = Some(strings::dialog_failed(&error).into());
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn open_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        match self.session.reopen(path) {
            Ok(session) => {
                self.status = None;
                self.replace_session(session, cx);
            }
            // The current document stays open and untouched.
            Err(error) => {
                self.status = Some(strings::open_failed(path, &error).into());
                cx.notify();
            }
        }
    }

    fn perform(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        match pending {
            Pending::New => {
                self.status = None;
                self.replace_session(Session::new(Document::empty(strings::UNTITLED)), cx);
            }
            Pending::Sample => {
                self.status = None;
                self.replace_session(Session::new(sample_document()), cx);
            }
            Pending::Open => self.open_dialog(cx),
            Pending::Import => {
                if let Some(imported) = self.pending_import.take() {
                    let imported_count = imported.report.imported_nodes;
                    let warning_count = import_warning_count(&imported.report);
                    self.import_report = Some(imported.report);
                    self.import_preview = None;
                    self.replace_session(Session::new(imported.document), cx);
                    self.status =
                        Some(strings::import_applied(imported_count, warning_count).into());
                }
            }
            Pending::Close => window.remove_window(),
        }
    }

    /// Run `pending`, asking first if it would lose unsaved changes.
    fn guarded(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        if !self.session.is_dirty() {
            return self.perform(pending, window, cx);
        }
        self.prompt_pending = true;
        let answer = window.prompt(
            PromptLevel::Warning,
            strings::UNSAVED_TITLE,
            Some(strings::UNSAVED_DETAIL),
            &[strings::DISCARD, strings::KEEP_EDITING],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let answer = answer.await;
            this.update(cx, |this, _| this.prompt_pending = false).ok();
            if answer == Ok(0) {
                this.update_in(cx, |this, window, cx| this.perform(pending, window, cx))
                    .ok();
            } else if pending == Pending::Import {
                this.update(cx, |this, _| {
                    this.import_preview = this.pending_import.take();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Whether the window may close now. With unsaved changes the user is
    /// asked, and the window closes later if they agree.
    pub fn should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.session.is_dirty() {
            return true;
        }
        cx.defer_in(window, |this, window, cx| {
            this.guarded(Pending::Close, window, cx);
        });
        false
    }

    fn new_document(&mut self, _: &NewDocument, window: &mut Window, cx: &mut Context<Self>) {
        self.guarded(Pending::New, window, cx);
    }

    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        self.guarded(Pending::Open, window, cx);
    }

    fn pointer(&self, position: Point<Pixels>) -> Option<(f64, f64)> {
        self.canvas
            .borrow()
            .page_point(f32::from(position.x), f32::from(position.y))
    }

    fn pointer_operations(&self, point: (f64, f64), finished: bool) -> Option<Vec<Operation>> {
        drag_operations(
            self.drag?,
            point,
            finished,
            self.tool,
            self.page,
            self.session.document().next_node_id,
        )
    }

    fn reset_prototype_preview(&mut self) {
        if let Some(page) = self.session.document().page(self.page) {
            self.prototype_preview = PrototypePreview::new(page);
        }
    }

    fn preview_page_at(&self, now: Instant) -> Option<Page> {
        let page = self.session.document().page(self.page)?;
        self.prototype_preview.page_at(page, now)
    }

    fn preview_hit(&self, position: Point<Pixels>) -> Option<NodeId> {
        let point = self.pointer(position)?;
        let page = self.preview_page_at(Instant::now())?;
        hit_test(&page, point.0, point.1)
    }

    fn create_prototype_state(&mut self, trigger: PrototypeTrigger, cx: &mut Context<Self>) {
        let Some(node_id) = self.selection else {
            self.status = Some(strings::PREVIEW_SELECT_NODE.into());
            cx.notify();
            return;
        };
        let Some(page) = self.session.document().page(self.page).cloned() else {
            return;
        };
        let Some(node) = page.node(node_id) else {
            self.selection = None;
            self.status = Some(strings::PREVIEW_SELECT_NODE.into());
            cx.notify();
            return;
        };
        if page.prototype.states.len() >= MAX_PROTOTYPE_STATES {
            self.status = Some(strings::PROTOTYPE_STATE_LIMIT.into());
            cx.notify();
            return;
        }
        let Some(id) = page
            .prototype
            .states
            .iter()
            .map(|state| state.id.0)
            .max()
            .unwrap_or(INITIAL_STATE.0)
            .checked_add(1)
        else {
            self.status = Some(strings::PROTOTYPE_STATE_LIMIT.into());
            cx.notify();
            return;
        };
        if page
            .prototype
            .transition(node_id, INITIAL_STATE, trigger)
            .is_some()
        {
            self.status = Some(strings::PROTOTYPE_TRIGGER_EXISTS.into());
            cx.notify();
            return;
        }
        let state = PrototypeStateId(id);
        let fill = node.fill.map(|mut color| {
            let shift: i16 = if trigger == PrototypeTrigger::Hover {
                36
            } else {
                -36
            };
            color.r = (i16::from(color.r) + shift).clamp(0, 255) as u8;
            color.g = (i16::from(color.g) + shift).clamp(0, 255) as u8;
            color.b = (i16::from(color.b) + shift).clamp(0, 255) as u8;
            FillOverride::Set(color)
        });
        let opacity = node
            .fill
            .is_none()
            .then_some((node.opacity * 0.72).max(0.12));
        let mut prototype = page.prototype.clone();
        let name = match trigger {
            PrototypeTrigger::Hover => format!("Al pasar {id}"),
            PrototypeTrigger::HoverLeave | PrototypeTrigger::Click => format!("Al pulsar {id}"),
        };
        prototype.states.push(PrototypeState {
            id: state,
            name,
            overrides: vec![NodeOverride {
                node: node_id,
                bounds: None,
                fill,
                opacity,
                visible: None,
            }],
        });
        let transition = PrototypeTransition {
            node: node_id,
            from: INITIAL_STATE,
            trigger,
            to: state,
            duration_ms: DEFAULT_PROTOTYPE_DURATION_MS,
            easing: Easing::EaseOut,
        };
        prototype.transitions.push(transition);
        if trigger == PrototypeTrigger::Hover {
            prototype.transitions.push(PrototypeTransition {
                node: node_id,
                from: state,
                trigger: PrototypeTrigger::HoverLeave,
                to: INITIAL_STATE,
                duration_ms: DEFAULT_PROTOTYPE_DURATION_MS,
                easing: Easing::EaseOut,
            });
        } else {
            prototype.transitions.push(PrototypeTransition {
                node: node_id,
                from: state,
                trigger: PrototypeTrigger::Click,
                to: INITIAL_STATE,
                duration_ms: DEFAULT_PROTOTYPE_DURATION_MS,
                easing: Easing::EaseOut,
            });
        }
        match self.session.apply(&[Operation::SetPrototype {
            page: self.page,
            prototype,
        }]) {
            Ok(_) => {
                self.status = Some(strings::PROTOTYPE_STATE_CREATED.into());
                self.reset_prototype_preview();
                self.redraw(cx);
            }
            Err(error) => {
                self.status = Some(strings::change_failed(&error).into());
                cx.notify();
            }
        }
    }

    fn remove_prototype_state(&mut self, state: PrototypeStateId, cx: &mut Context<Self>) {
        if state == INITIAL_STATE {
            return;
        }
        let Some(page) = self.session.document().page(self.page).cloned() else {
            return;
        };
        let mut prototype = page.prototype.clone();
        if !prototype.states.iter().any(|entry| entry.id == state) {
            return;
        }
        prototype.states.retain(|entry| entry.id != state);
        prototype
            .transitions
            .retain(|transition| transition.from != state && transition.to != state);
        match self.session.apply(&[Operation::SetPrototype {
            page: self.page,
            prototype,
        }]) {
            Ok(_) => {
                self.status = Some(strings::PROTOTYPE_STATE_REMOVED.into());
                self.reset_prototype_preview();
                self.redraw(cx);
            }
            Err(error) => {
                self.status = Some(strings::change_failed(&error).into());
                cx.notify();
            }
        }
    }

    fn canvas_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Keyboard shortcuts act on the canvas after a click on it.
        window.focus(&self.focus);
        if self.screen == Screen::Preview {
            self.prototype_preview
                .pointer_down(self.preview_hit(event.position));
            return;
        }
        if self.animation.preview {
            self.animation.pause();
            self.status = Some(strings::ANIMATION_EDIT_HINT.into());
            cx.notify();
            return;
        }
        let Some(point) = self.pointer(event.position) else {
            return;
        };
        self.status = None;
        self.preview = None;
        self.drag = match self.tool {
            Tool::Move => {
                let page = self.session.document().page(self.page);
                let hit = page.and_then(|page| {
                    let node = page.node(hit_test(page, point.0, point.1)?)?;
                    Some((node.id, node.bounds))
                });
                self.selection = hit.map(|(node, _)| node);
                hit.map(|(node, original)| Drag::Move {
                    node,
                    start: point,
                    original,
                })
            }
            Tool::Frame | Tool::Rectangle | Tool::Ellipse => Some(Drag::Create { start: point }),
            Tool::Text | Tool::Pen => {
                self.status = Some(strings::TOOL_PENDING.into());
                None
            }
        };
        self.redraw(cx);
    }

    fn canvas_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Preview {
            let page = self.session.document().page(self.page).cloned();
            if let Some(page) = page
                && self.prototype_preview.pointer_moved(
                    &page,
                    self.preview_hit(event.position),
                    Instant::now(),
                )
            {
                self.redraw(cx);
            }
            return;
        }
        if self.drag.is_none() || !event.dragging() {
            return;
        }
        let Some(point) = self.pointer(event.position) else {
            return;
        };
        // An invalid preview is simply not shown; the commit reports it.
        self.preview = self
            .pointer_operations(point, false)
            .and_then(|operations| self.session.document().transact(&operations).ok())
            .map(|(document, _)| document);
        self.redraw(cx);
    }

    fn canvas_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.screen == Screen::Preview {
            let page = self.session.document().page(self.page).cloned();
            if let Some(page) = page
                && self.prototype_preview.pointer_up(
                    &page,
                    self.preview_hit(event.position),
                    Instant::now(),
                )
            {
                self.redraw(cx);
            }
            return;
        }
        let Some(drag) = self.drag else {
            return;
        };
        let operations = self
            .pointer(event.position)
            .and_then(|point| self.pointer_operations(point, true));
        self.drag = None;
        self.preview = None;
        if let Some(operations) = operations {
            match self.session.apply(&operations) {
                Ok(affected) => {
                    if matches!(drag, Drag::Create { .. }) {
                        self.selection = affected.first().copied();
                        self.tool = Tool::Move;
                    }
                }
                Err(error) => self.status = Some(strings::change_failed(&error).into()),
            }
        }
        self.redraw(cx);
    }

    fn select_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(
            A11Y_NAV_BASE + Screen::ALL.iter().position(|s| *s == screen).unwrap_or(0) as u64,
        );
        if screen == Screen::Preview {
            self.reset_prototype_preview();
        }
        self.drag = None;
        self.preview = None;
        self.screen = screen;
        self.redraw(cx);
    }
    fn select_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(
            A11Y_TOOL_BASE + Tool::ALL.iter().position(|t| *t == tool).unwrap_or(0) as u64,
        );
        self.tool = tool;
        self.drag = None;
        self.preview = None;
        self.redraw(cx);
    }
    fn select_layer(&mut self, id: NodeId, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_LAYER_BASE.saturating_add(id.0));
        self.selection = Some(id);
        self.redraw(cx);
    }
    fn select_panel(&mut self, panel: NarrowPanel, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(
            A11Y_PANEL_BASE
                + NarrowPanel::ALL
                    .iter()
                    .position(|p| *p == panel)
                    .unwrap_or(0) as u64,
        );
        self.narrow_panel = panel;
        cx.notify();
    }
    fn set_theme(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_THEME);
        self.dark = dark;
        cx.notify();
    }
    fn apply_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_IMPORT_APPLY);
        if let Some(imported) = self.import_preview.take() {
            self.import_report = Some(imported.report.clone());
            self.pending_import = Some(imported);
            self.guarded(Pending::Import, window, cx);
        }
    }
    fn select_prototype_state(&mut self, id: PrototypeStateId, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_STATE_BASE.saturating_add(id.0));
        if let Some(page) = self.session.document().page(self.page)
            && page.prototype.state(id).is_some()
        {
            self.prototype_preview.select_state(page, id);
            self.redraw(cx);
        }
    }
    fn play_animation(&mut self, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_PLAY);
        if let Some(page) = self.current_page() {
            let duration = page.animation.duration_ms;
            self.drag = None;
            self.preview = None;
            self.animation.toggle_play(Instant::now(), duration);
            self.redraw(cx);
        }
    }
    fn toggle_timeline(&mut self, _: &ToggleTimeline, _: &mut Window, cx: &mut Context<Self>) {
        self.a11y_focus = A11yNodeId(A11Y_TIMELINE);
        self.timeline_open = !self.timeline_open;
        if !self.timeline_open {
            self.animation.pause();
            self.animation.preview = false;
        }
        self.redraw(cx);
    }

    fn toggle_theme(&mut self, _: &ToggleTheme, _: &mut Window, cx: &mut Context<Self>) {
        self.set_theme(!self.dark, cx);
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

/// A tool bar command. When unavailable it is dimmed, cannot be focused and
/// does nothing.
fn command(
    id: &'static str,
    label: &'static str,
    available: bool,
    palette: &Palette,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    if available {
        button(id, label, palette, false)
            .on_click(on_click)
            .into_any_element()
    } else {
        div()
            .flex()
            .flex_none()
            .items_center()
            .min_h(px(metrics::ROW_HEIGHT))
            .px(px(metrics::SPACE_S))
            .border_1()
            .border_color(transparent_black())
            .text_color(rgb(palette.text_muted))
            .opacity(0.5)
            .child(label)
            .into_any_element()
    }
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

fn import_warning_count(report: &import::ImportReport) -> usize {
    report.warnings.len() + report.omitted_warnings
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
                    this.select_screen(screen, cx);
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
                        this.select_tool(tool, cx);
                    }),
                )
            }))
            .child(
                div()
                    .flex_none()
                    .w(px(1.0))
                    .h(px(metrics::ROW_HEIGHT))
                    .mx(px(metrics::SPACE_XS))
                    .bg(rgb(palette.border)),
            )
            .child(command(
                "undo",
                strings::UNDO,
                self.session.can_undo(),
                palette,
                cx.listener(|this, _, window, cx| this.undo(&Undo, window, cx)),
            ))
            .child(command(
                "redo",
                strings::REDO,
                self.session.can_redo(),
                palette,
                cx.listener(|this, _, window, cx| this.redo(&Redo, window, cx)),
            ))
            .child(command(
                "save",
                strings::SAVE,
                self.session.is_dirty() || self.session.path().is_none(),
                palette,
                cx.listener(|this, _, window, cx| this.save(&Save, window, cx)),
            ))
            .when(mode == LayoutMode::Wide, |this| {
                this.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .pl(px(metrics::SPACE_M))
                        .text_size(px(metrics::TEXT_S))
                        .text_color(rgb(palette.text_muted))
                        .child(self.title()),
                )
            })
    }

    /// Keep a fixed status row so messages cannot shift pointer coordinates.
    fn status_bar(&self, palette: &Palette) -> Div {
        div()
            .flex_none()
            .h(px(metrics::STATUS_HEIGHT))
            .overflow_hidden()
            .px(px(metrics::SPACE_M))
            .py(px(metrics::SPACE_XS))
            .bg(rgb(palette.panel))
            .border_b_1()
            .border_color(rgb(palette.border))
            .text_size(px(metrics::TEXT_S))
            .child(self.status.clone().unwrap_or_default())
    }
    fn layers_panel(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        // Collected eagerly: a lazy iterator would keep `cx` borrowed while
        // the layer rows below need it.
        let pages: Vec<Stateful<Div>> = self
            .document()
            .pages
            .iter()
            .map(|page| {
                let id = page.id;
                button(("page", id.0), page.name.clone(), palette, self.page == id)
                    .mx(px(metrics::SPACE_XS))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = id;
                        this.animation = crate::timeline::Timeline::default();
                        this.selection = None;
                        this.drag = None;
                        this.preview = None;
                        this.redraw(cx);
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
        let selected = self.shown_selection() == Some(id);
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
                this.select_layer(id, cx);
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
            .shown_selection()
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
                .child(row(
                    strings::OPACITY,
                    text(format!("{} %", format_number(node.opacity * 100.0))),
                ))
                .when_some(node.fill, |this, color| {
                    this.child(row(
                        strings::FILL_OPACITY,
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

    fn canvas_panel(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let panel = div()
            .relative()
            .size_full()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::canvas_mouse_down))
            .on_mouse_move(cx.listener(Self::canvas_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::canvas_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::canvas_mouse_up));
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
        let Some(page) = (if self.screen == Screen::Preview {
            self.preview_page_at(Instant::now())
        } else {
            self.current_page().cloned()
        }) else {
            return panel;
        };
        let page = if self.screen == Screen::Design && self.animation.preview {
            page.at_time(self.animation.time_ms)
        } else {
            page
        };
        let empty = page.nodes.is_empty();
        let selection = (self.screen != Screen::Preview)
            .then(|| self.shown_selection())
            .flatten();
        let (generation, view_epoch) = (self.generation, self.view_epoch);
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
                let mut host = host.borrow_mut();
                host.set_placement(
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                    scale,
                );
                match host.frame(&page, selection, generation, view_epoch, width, height) {
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
                            |this, _, window, cx| {
                                this.toggle_timeline(&ToggleTimeline, window, cx);
                            },
                        )),
                    ),
            )
            .when(self.timeline_open, |this| {
                this.child(self.timeline_editor(palette, cx))
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
                        .child(self.canvas_panel(palette, cx)),
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
                                        this.select_panel(panel, cx);
                                    },
                                ))
                            },
                        ));
                let panel = match self.narrow_panel {
                    NarrowPanel::Layers => self.layers_panel(palette, cx),
                    NarrowPanel::Canvas => self.canvas_panel(palette, cx),
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
            .child(self.status_bar(palette))
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
        let location = match self.session.path() {
            Some(path) => path.display().to_string(),
            None => strings::NOT_SAVED_YET.to_owned(),
        };
        self.page_frame(Screen::Documents.label())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_XS))
                    .child(muted(strings::CURRENT_DOCUMENT, palette))
                    .child(div().child(self.title()))
                    .child(muted(location, palette).text_size(px(metrics::TEXT_S))),
            )
            .children(self.status.clone().map(|status| div().child(status)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(metrics::SPACE_S))
                    .child(
                        button("new-document", strings::NEW_DOCUMENT, palette, true).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.guarded(Pending::New, window, cx)
                            }),
                        ),
                    )
                    .child(
                        button("open-document", strings::OPEN_DOCUMENT, palette, false).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.guarded(Pending::Open, window, cx)
                            }),
                        ),
                    )
                    .child(
                        button("save-document-as", strings::SAVE_AS, palette, false)
                            .on_click(cx.listener(|this, _, _, cx| this.save_as(cx))),
                    )
                    .child(
                        button("open-sample", strings::OPEN_SAMPLE, palette, false).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.guarded(Pending::Sample, window, cx)
                            }),
                        ),
                    ),
            )
            .child(muted(strings::NO_RECENT_DOCUMENTS, palette))
    }

    fn pending(&self, screen: Screen, palette: &Palette) -> Div {
        self.page_frame(screen.label())
            .child(div().child(strings::PENDING_TITLE))
            .child(muted(screen.pending_description(), palette))
    }

    fn import_screen(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let report = self
            .import_preview
            .as_ref()
            .map(|result| &result.report)
            .or(self.import_report.as_ref());
        let summary = report.map_or_else(
            || strings::IMPORT_NO_RESULT.to_owned(),
            |report| strings::import_summary(report.imported_nodes, import_warning_count(report)),
        );
        let warnings: Vec<Div> = report
            .into_iter()
            .flat_map(|report| report.warnings.iter())
            .take(100)
            .map(|warning| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_XS))
                    .child(warning.source_path.clone())
                    .child(muted(warning.detail.clone(), palette))
            })
            .collect();
        let can_apply = self.import_preview.is_some();
        self.page_frame(Screen::Import.label())
            .child(muted(strings::IMPORT_DESCRIPTION, palette))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(metrics::SPACE_S))
                    .child(
                        button(
                            "import-pick-file",
                            strings::IMPORT_PICK_FILE,
                            palette,
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.open_import(cx))),
                    )
                    .child(command(
                        "import-apply",
                        strings::IMPORT_APPLY,
                        can_apply,
                        palette,
                        cx.listener(|this, _, window, cx| {
                            this.apply_import(window, cx);
                        }),
                    )),
            )
            .child(div().child(strings::IMPORT_REPORT))
            .child(div().child(summary))
            .child(div().child(strings::IMPORT_WARNINGS))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_S))
                    .children(warnings)
                    .when(
                        report.is_some_and(|report| import_warning_count(report) > 100),
                        |this| {
                            this.child(muted(
                                report.map_or_else(String::new, |report| {
                                    strings::import_more_warnings(import_warning_count(report))
                                }),
                                palette,
                            ))
                        },
                    ),
            )
            .when(
                report.is_some_and(|report| {
                    report.warnings.is_empty() && report.omitted_warnings == 0
                }),
                |this| this.child(muted(strings::IMPORT_NO_WARNINGS, palette)),
            )
            .when(
                report.is_some_and(|report| report.omitted_warnings > 0),
                |this| {
                    this.child(muted(
                        strings::import_omitted_warnings(
                            report.map_or(0, |report| report.omitted_warnings),
                        ),
                        palette,
                    ))
                },
            )
            .when_some(self.status.clone(), |this, status| {
                this.child(div().child(status))
            })
    }

    fn preview(&self, mode: LayoutMode, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let Some(page) = self.session.document().page(self.page) else {
            return self.page_frame(Screen::Preview.label());
        };
        let mut rows = Vec::new();
        flatten(&page.nodes, 0, &mut rows);
        let state_rows = page
            .prototype
            .states
            .iter()
            .enumerate()
            .map(|(index, state)| {
                let id = state.id;
                let label = if id == INITIAL_STATE {
                    strings::PREVIEW_INITIAL.to_owned()
                } else {
                    state.name.clone()
                };
                button(
                    ("prototype-state", index),
                    label,
                    palette,
                    self.prototype_preview.state() == id,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_prototype_state(id, cx);
                }))
            });
        let selected = self
            .selection
            .and_then(|id| page.node(id))
            .map(|node| node.name.clone())
            .unwrap_or_else(|| strings::NO_SELECTED_LAYER.to_owned());
        let node_rows = if rows.is_empty() {
            div().child(muted(strings::NO_PROTOTYPE_STATES, palette))
        } else {
            div().flex().flex_col().gap(px(metrics::SPACE_XS)).children(
                rows.iter().enumerate().map(|(index, (depth, node))| {
                    let id = node.id;
                    let label = format!("{}{}", " ".repeat(depth.saturating_mul(2)), node.name);
                    button(
                        ("prototype-node", index),
                        label,
                        palette,
                        self.selection == Some(id),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_layer(id, cx);
                    }))
                }),
            )
        };
        let make_hover = command(
            "prototype-create-hover",
            strings::CREATE_HOVER_STATE,
            self.selection.is_some() && page.prototype.states.len() < MAX_PROTOTYPE_STATES,
            palette,
            cx.listener(|this, _, _, cx| {
                this.create_prototype_state(PrototypeTrigger::Hover, cx);
            }),
        );
        let make_click = command(
            "prototype-create-click",
            strings::CREATE_CLICK_STATE,
            self.selection.is_some() && page.prototype.states.len() < MAX_PROTOTYPE_STATES,
            palette,
            cx.listener(|this, _, _, cx| {
                this.create_prototype_state(PrototypeTrigger::Click, cx);
            }),
        );
        let remove_state = command(
            "prototype-remove-state",
            strings::REMOVE_PROTOTYPE_STATE,
            self.prototype_preview.state() != INITIAL_STATE,
            palette,
            cx.listener(|this, _, _, cx| {
                let state = this.prototype_preview.state();
                this.remove_prototype_state(state, cx);
            }),
        );
        let controls = div()
            .id("prototype-controls")
            .flex()
            .flex_col()
            .flex_none()
            .min_h_0()
            .gap(px(metrics::SPACE_S))
            .when(mode == LayoutMode::Wide, |this| {
                this.w(px(metrics::INSPECTOR_WIDTH))
                    .h_full()
                    .overflow_y_scroll()
            })
            .when(mode == LayoutMode::Narrow, |this| {
                this.w_full()
                    .max_h(px(metrics::PREVIEW_CONTROLS_MAX_HEIGHT))
                    .overflow_y_scroll()
            })
            .child(div().child(strings::PROTOTYPE_STATES))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(metrics::SPACE_XS))
                    .children(state_rows),
            )
            .child(remove_state)
            .child(div().child(strings::SELECTED_LAYER))
            .child(muted(selected, palette))
            .child(make_hover)
            .child(make_click)
            .child(div().child(strings::PROTOTYPE_LAYERS))
            .child(node_rows);
        let canvas = div()
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .border_1()
            .border_color(rgb(palette.border))
            .child(self.canvas_panel(palette, cx));
        let body = match mode {
            LayoutMode::Wide => div()
                .flex()
                .flex_1()
                .min_h_0()
                .gap(px(metrics::SPACE_M))
                .child(canvas)
                .child(controls),
            LayoutMode::Narrow => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap(px(metrics::SPACE_M))
                .child(controls)
                .child(canvas),
        };
        self.page_frame(Screen::Preview.label())
            .child(muted(strings::PREVIEW_HELP, palette))
            .child(body)
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
                                        this.set_theme(dark, cx);
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
        let accessibility_sender = self.accessibility_sender.clone();
        window.init_accessibility(move |request| {
            if accessibility_sender.send(request).is_err() {
                eprintln!("Screen-reader action channel is closed");
            }
        });
        let tree = self.accessibility_tree(window);
        if self.a11y_cache.as_ref() != Some(&tree) {
            self.a11y_cache = Some(tree.clone());
            window.update_accessibility_tree(tree);
        }
        if self.screen != Screen::Design || !self.timeline_open {
            self.animation.pause();
        }
        let duration = self
            .session
            .document()
            .page(self.page)
            .map_or(2000, |page| page.animation.duration_ms);
        let previous_time = self.animation.time_ms;
        self.animation.tick(std::time::Instant::now(), duration);
        if previous_time != self.animation.time_ms {
            self.generation += 1;
        }
        if self.animation.is_playing() {
            window.request_animation_frame();
        }
        let now = Instant::now();
        if self.screen == Screen::Preview && self.prototype_preview.has_transition() {
            self.generation += 1;
            if self.prototype_preview.is_animating(now) {
                window.request_animation_frame();
            } else {
                self.prototype_preview.finish_transition(now);
            }
        }
        let title = format!("{} — {}", self.title(), strings::APP_NAME);
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }
        let palette = self.palette();
        let mode = layout_mode(f32::from(window.viewport_size().width));
        let content = match self.screen {
            Screen::Documents => self.documents(&palette, cx),
            Screen::Design => self.workspace(mode, &palette, cx),
            Screen::Preview => self.preview(mode, &palette, cx),
            Screen::Import => self.import_screen(&palette, cx),
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
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::save_as_action))
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::new_document))
            .on_action(cx.listener(Self::delete_selection))
            .on_action(cx.listener(Self::cancel))
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

    const PAGE: PageId = PageId(1);

    #[test]
    fn a_create_drag_becomes_one_named_node_in_whole_units() {
        let drag = Drag::Create {
            start: (10.4, 20.6),
        };
        // Dragging up and to the left still gives a positive size.
        let operations = drag_operations(drag, (-29.6, 0.4), false, Tool::Ellipse, PAGE, 7)
            .expect("a drag that moved");
        assert_eq!(
            operations,
            [Operation::AddNode {
                page: PAGE,
                parent: None,
                name: "Elipse 7".into(),
                kind: NodeKind::Ellipse,
                bounds: Rect::new(-30.0, 0.0, 40.0, 21.0),
                fill: Some(SHAPE_FILL),
            }]
        );
        // A straight line still gets a visible thickness.
        let Some(Operation::AddNode { bounds, .. }) =
            drag_operations(drag, (60.4, 20.6), true, Tool::Frame, PAGE, 7)
                .and_then(|operations| operations.into_iter().next())
        else {
            panic!("expected a node");
        };
        assert_eq!(bounds, Rect::new(10.0, 21.0, 50.0, 1.0));
    }

    #[test]
    fn a_click_creates_a_default_shape_only_when_released() {
        let drag = Drag::Create { start: (5.0, 5.0) };
        assert_eq!(
            drag_operations(drag, (5.2, 5.1), false, Tool::Rectangle, PAGE, 1),
            None
        );
        let Some(Operation::AddNode { bounds, .. }) =
            drag_operations(drag, (5.2, 5.1), true, Tool::Rectangle, PAGE, 1)
                .and_then(|operations| operations.into_iter().next())
        else {
            panic!("expected a node");
        };
        assert_eq!(bounds, Rect::new(5.0, 5.0, CLICK_SIZE, CLICK_SIZE));
        // Tools that cannot create anything yet never produce operations.
        for tool in [Tool::Move, Tool::Text, Tool::Pen] {
            assert_eq!(
                drag_operations(drag, (90.0, 90.0), true, tool, PAGE, 1),
                None
            );
        }
    }

    #[test]
    fn a_move_drag_offsets_the_original_bounds_and_ignores_no_movement() {
        let drag = Drag::Move {
            node: NodeId(3),
            start: (100.0, 100.0),
            original: Rect::new(1.0, 2.0, 30.0, 40.0),
        };
        assert_eq!(
            drag_operations(drag, (100.3, 99.8), true, Tool::Move, PAGE, 9),
            None
        );
        assert_eq!(
            drag_operations(drag, (112.4, 95.0), true, Tool::Move, PAGE, 9),
            Some(vec![Operation::SetBounds {
                page: PAGE,
                node: NodeId(3),
                bounds: Rect::new(13.0, -3.0, 30.0, 40.0),
            }])
        );
    }

    #[test]
    fn drawn_shapes_are_accepted_by_the_document_and_can_be_undone() {
        let mut session = Session::new(Document::empty(strings::UNTITLED));
        let next = session.document().next_node_id;
        let drag = Drag::Create { start: (0.0, 0.0) };
        let operations =
            drag_operations(drag, (80.0, 40.0), true, Tool::Rectangle, PAGE, next).expect("ops");
        let created = session.apply(&operations).expect("valid");
        assert_eq!(created, [NodeId(next)]);
        assert_eq!(session.document().pages[0].nodes[0].name, "Rectángulo 1");
        assert!(session.undo());
        assert!(session.document().pages[0].nodes.is_empty());
    }

    #[test]
    fn the_file_extension_is_added_once() {
        for (given, expected) in [
            ("a/b/logo", "a/b/logo.dsg"),
            ("a/b/logo.dsg", "a/b/logo.dsg"),
            ("a/b/logo.DSG", "a/b/logo.DSG"),
            ("a/b/logo.v2", "a/b/logo.v2.dsg"),
        ] {
            assert_eq!(
                with_extension(PathBuf::from(given)),
                PathBuf::from(expected)
            );
        }
    }
}
