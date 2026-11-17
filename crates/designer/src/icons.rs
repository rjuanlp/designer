//! Interface icons.
//!
//! Every icon is an original 16x16 outline drawn for Designer and embedded in
//! the binary. GPUI rasterizes them as alpha masks and tints them with the
//! element's text color, so the stroke color written here is irrelevant.

#[cfg(windows)]
use std::borrow::Cow;

/// An interface icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// Selection and move tool.
    Move,
    /// Frame tool.
    Frame,
    /// Rectangle tool.
    Rectangle,
    /// Ellipse tool.
    Ellipse,
    /// Text tool.
    Text,
    /// Pen tool.
    Pen,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Save.
    Save,
    /// Save as.
    SaveAs,
    /// Shown layer.
    Eye,
    /// Hidden layer.
    EyeOff,
    /// Locked layer.
    Lock,
    /// Unlocked layer.
    Unlock,
    /// Raise in the layer order.
    ArrowUp,
    /// Lower in the layer order.
    ArrowDown,
    /// Start playback.
    Play,
    /// Pause playback.
    Pause,
    /// Add or increase.
    Plus,
    /// Decrease.
    Minus,
    /// Delete.
    Trash,
    /// New document.
    FilePlus,
    /// Open a document.
    FolderOpen,
    /// Sample document.
    Shapes,
    /// Step back.
    ChevronLeft,
    /// Step forward.
    ChevronRight,
    /// Write a keyframe.
    Key,
    /// Remove a keyframe.
    KeyOff,
    /// Add a sequence.
    SequenceAdd,
    /// Remove a sequence.
    SequenceRemove,
    /// Edit base values.
    Pencil,
    /// Hover interaction.
    PointerHover,
    /// Click interaction.
    PointerClick,
    /// Choose a file to import.
    Import,
    /// Confirm or apply.
    Check,
    /// Light theme.
    Sun,
    /// Dark theme.
    Moon,
    /// Timeline.
    Timeline,
    /// Reset to the inherited value.
    Reset,
    /// Remove the fill.
    FillNone,
    /// Add a fill.
    FillAdd,
    /// Checked toggle.
    ToggleOn,
    /// Unchecked toggle.
    ToggleOff,
    /// Zoom in.
    ZoomIn,
    /// Zoom out.
    ZoomOut,
    /// Fit the content in view.
    ZoomFit,
}

impl Icon {
    /// Every icon, for tests.
    pub const ALL: [Self; 46] = [
        Self::Move,
        Self::Frame,
        Self::Rectangle,
        Self::Ellipse,
        Self::Text,
        Self::Pen,
        Self::Undo,
        Self::Redo,
        Self::Save,
        Self::SaveAs,
        Self::Eye,
        Self::EyeOff,
        Self::Lock,
        Self::Unlock,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::Play,
        Self::Pause,
        Self::Plus,
        Self::Minus,
        Self::Trash,
        Self::FilePlus,
        Self::FolderOpen,
        Self::Shapes,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::Key,
        Self::KeyOff,
        Self::SequenceAdd,
        Self::SequenceRemove,
        Self::Pencil,
        Self::PointerHover,
        Self::PointerClick,
        Self::Import,
        Self::Check,
        Self::Sun,
        Self::Moon,
        Self::Timeline,
        Self::Reset,
        Self::FillNone,
        Self::FillAdd,
        Self::ToggleOn,
        Self::ToggleOff,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ZoomFit,
    ];

    /// Asset path GPUI loads the icon from.
    pub fn path(self) -> &'static str {
        self.entry().0
    }

    /// The drawing inside the 16x16 view box.
    fn body(self) -> &'static str {
        self.entry().1
    }

    fn entry(self) -> (&'static str, &'static str) {
        match self {
            Self::Move => (
                "icons/move.svg",
                r#"<path d="M3 2 12.5 6.4 8.4 7.9 6.9 12Z"/><path d="M8.7 8.7 12.5 12.5"/>"#,
            ),
            Self::Frame => (
                "icons/frame.svg",
                r#"<path d="M5 1.5v13M11 1.5v13M1.5 5h13M1.5 11h13"/>"#,
            ),
            Self::Rectangle => (
                "icons/rectangle.svg",
                r#"<rect x="2.5" y="3.5" width="11" height="9" rx="1"/>"#,
            ),
            Self::Ellipse => ("icons/ellipse.svg", r#"<circle cx="8" cy="8" r="5.5"/>"#),
            Self::Text => (
                "icons/text.svg",
                r#"<path d="M3.5 4.5V3h9v1.5M8 3v10M6.2 13h3.6"/>"#,
            ),
            Self::Pen => (
                "icons/pen.svg",
                r#"<path d="M8 1.5 12 9 10.2 12.5H5.8L4 9ZM8 1.5V7.6M5.8 14.5h4.4"/><circle cx="8" cy="8.8" r="1.1"/>"#,
            ),
            Self::Undo => (
                "icons/undo.svg",
                r#"<path d="M5.5 3 2.5 6 5.5 9M2.5 6h7a3.5 3.5 0 0 1 0 7H7"/>"#,
            ),
            Self::Redo => (
                "icons/redo.svg",
                r#"<path d="M10.5 3 13.5 6 10.5 9M13.5 6h-7a3.5 3.5 0 0 0 0 7H9"/>"#,
            ),
            Self::Save => (
                "icons/save.svg",
                r#"<path d="M2.5 2.5H11L13.5 5v8.5h-11ZM5 2.5V6h5V2.5M4.5 13.5V9.5h7v4"/>"#,
            ),
            Self::SaveAs => (
                "icons/save-as.svg",
                r#"<path d="M7.5 13.5h-5v-11H11L13.5 5v2.5M5 2.5V6h5V2.5M12 10v5M9.5 12.5h5"/>"#,
            ),
            Self::Eye => (
                "icons/eye.svg",
                r#"<path d="M1.5 8C3.5 4.8 5.7 3.5 8 3.5S12.5 4.8 14.5 8C12.5 11.2 10.3 12.5 8 12.5S3.5 11.2 1.5 8Z"/><circle cx="8" cy="8" r="2"/>"#,
            ),
            Self::EyeOff => (
                "icons/eye-off.svg",
                r#"<path d="M1.5 8C3.5 4.8 5.7 3.5 8 3.5S12.5 4.8 14.5 8C12.5 11.2 10.3 12.5 8 12.5S3.5 11.2 1.5 8ZM2.5 2.5l11 11"/>"#,
            ),
            Self::Lock => (
                "icons/lock.svg",
                r#"<rect x="3.5" y="7" width="9" height="7" rx="1"/><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2"/>"#,
            ),
            Self::Unlock => (
                "icons/unlock.svg",
                r#"<rect x="3.5" y="7" width="9" height="7" rx="1"/><path d="M5.5 7V5a2.5 2.5 0 0 1 4.8-1"/>"#,
            ),
            Self::ArrowUp => ("icons/arrow-up.svg", r#"<path d="M8 13V3M4 7l4-4 4 4"/>"#),
            Self::ArrowDown => ("icons/arrow-down.svg", r#"<path d="M8 3v10M4 9l4 4 4-4"/>"#),
            Self::Play => (
                "icons/play.svg",
                r#"<path d="M4.5 3 12.5 8 4.5 13Z" fill="black"/>"#,
            ),
            Self::Pause => (
                "icons/pause.svg",
                r#"<path d="M5.5 3.5v9M10.5 3.5v9" stroke-width="2.2"/>"#,
            ),
            Self::Plus => ("icons/plus.svg", r#"<path d="M8 3v10M3 8h10"/>"#),
            Self::Minus => ("icons/minus.svg", r#"<path d="M3 8h10"/>"#),
            Self::Trash => (
                "icons/trash.svg",
                r#"<path d="M3 4.5h10M6 4.5V3h4v1.5M4.5 4.5l.7 9h5.6l.7-9M6.8 7v4M9.2 7v4"/>"#,
            ),
            Self::FilePlus => (
                "icons/file-plus.svg",
                r#"<path d="M4 1.5h5L12.5 5v9.5H4ZM9 1.5V5h3.5M8.2 8v4M6.2 10h4"/>"#,
            ),
            Self::FolderOpen => (
                "icons/folder-open.svg",
                r#"<path d="M1.5 12.5V3.5h4L7 5h5.5v2M1.5 12.5l2-5.5h11l-2 5.5Z"/>"#,
            ),
            Self::Shapes => (
                "icons/shapes.svg",
                r#"<rect x="2" y="2.5" width="5" height="5"/><circle cx="11.5" cy="5" r="2.7"/><path d="M8 9.5 11.5 14h-7Z"/>"#,
            ),
            Self::ChevronLeft => (
                "icons/chevron-left.svg",
                r#"<path d="M10 3.5 5.5 8 10 12.5"/>"#,
            ),
            Self::ChevronRight => (
                "icons/chevron-right.svg",
                r#"<path d="M6 3.5 10.5 8 6 12.5"/>"#,
            ),
            Self::Key => (
                "icons/key.svg",
                r#"<path d="M8 2.5 13.5 8 8 13.5 2.5 8Z"/><circle cx="8" cy="8" r="1" fill="black"/>"#,
            ),
            Self::KeyOff => (
                "icons/key-off.svg",
                r#"<path d="M8 2.5 13.5 8 8 13.5 2.5 8ZM2.5 2.5l11 11"/>"#,
            ),
            Self::SequenceAdd => (
                "icons/sequence-add.svg",
                r#"<path d="M2 4.5h9M2 8h6M2 11.5h4M11.5 8.5v5M9 11h5"/>"#,
            ),
            Self::SequenceRemove => (
                "icons/sequence-remove.svg",
                r#"<path d="M2 4.5h9M2 8h6M2 11.5h4M9 11h5"/>"#,
            ),
            Self::Pencil => (
                "icons/pencil.svg",
                r#"<path d="M3 13l.6-3L11 2.6a1.4 1.4 0 0 1 2 2L5.6 12ZM9.8 3.8l2 2"/>"#,
            ),
            Self::PointerHover => (
                "icons/pointer-hover.svg",
                r#"<path d="M2.5 2 10 5.5 6.9 6.6 5.8 9.7Z"/><circle cx="11" cy="11" r="3" stroke-dasharray="1.6 1.6"/>"#,
            ),
            Self::PointerClick => (
                "icons/pointer-click.svg",
                r#"<path d="M6 6 14 9.7 10.6 10.9 9.4 14.3Z"/><path d="M5 2.2v1.6M2.2 5h1.6M2.9 2.9 4 4"/>"#,
            ),
            Self::Import => (
                "icons/import.svg",
                r#"<path d="M8 2.5V10M5 7l3 3 3-3M2.5 10.5v3h11v-3"/>"#,
            ),
            Self::Check => ("icons/check.svg", r#"<path d="M3 8.5 6.5 12 13 4.5"/>"#),
            Self::Sun => (
                "icons/sun.svg",
                r#"<circle cx="8" cy="8" r="2.8"/><path d="M8 1.5v1.7M8 12.8v1.7M1.5 8h1.7M12.8 8h1.7M3.4 3.4l1.2 1.2M11.4 11.4l1.2 1.2M3.4 12.6l1.2-1.2M11.4 4.6l1.2-1.2"/>"#,
            ),
            Self::Moon => (
                "icons/moon.svg",
                r#"<path d="M13 9.6A5.5 5.5 0 1 1 6.4 3 4.4 4.4 0 0 0 13 9.6Z"/>"#,
            ),
            Self::Timeline => (
                "icons/timeline.svg",
                r#"<path d="M2.5 2.5v11h11M5 10.8C8 10.8 8.5 5 13 4.8"/>"#,
            ),
            Self::Reset => (
                "icons/reset.svg",
                r#"<path d="M2.8 8A5.2 5.2 0 1 0 4.4 4.3M4.5 1.5v3h-3"/>"#,
            ),
            Self::FillNone => (
                "icons/fill-none.svg",
                r#"<circle cx="8" cy="8" r="5.5"/><path d="M4.1 11.9l7.8-7.8"/>"#,
            ),
            Self::FillAdd => (
                "icons/fill-add.svg",
                r#"<path d="M8 2C10.5 5.4 12 7.5 12 9.8a4 4 0 0 1-8 0C4 7.5 5.5 5.4 8 2Z"/>"#,
            ),
            Self::ToggleOn => (
                "icons/toggle-on.svg",
                r#"<rect x="2.5" y="2.5" width="11" height="11" rx="2"/><path d="M5 8.3 7.2 10.5 11 5.8"/>"#,
            ),
            Self::ToggleOff => (
                "icons/toggle-off.svg",
                r#"<rect x="2.5" y="2.5" width="11" height="11" rx="2"/>"#,
            ),
            Self::ZoomIn => (
                "icons/zoom-in.svg",
                r#"<circle cx="7" cy="7" r="4.5"/><path d="M10.3 10.3 14 14M7 5v4M5 7h4"/>"#,
            ),
            Self::ZoomOut => (
                "icons/zoom-out.svg",
                r#"<circle cx="7" cy="7" r="4.5"/><path d="M10.3 10.3 14 14M5 7h4"/>"#,
            ),
            Self::ZoomFit => (
                "icons/zoom-fit.svg",
                r#"<path d="M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10"/>"#,
            ),
        }
    }

    /// The complete SVG document of the icon.
    pub fn svg(self) -> String {
        format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" "#,
                r#"stroke="black" stroke-width="1.4" stroke-linecap="round" "#,
                r#"stroke-linejoin="round">{}</svg>"#
            ),
            self.body()
        )
    }

    /// The icon stored at an asset path.
    pub fn from_path(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|icon| icon.path() == path)
    }
}

/// Serves the embedded icons to GPUI.
#[cfg(windows)]
pub struct Assets;

#[cfg(windows)]
impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Icon::from_path(path).map(|icon| Cow::Owned(icon.svg().into_bytes())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(Icon::ALL
            .into_iter()
            .map(Icon::path)
            .filter(|icon| icon.starts_with(path))
            .map(Into::into)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_a_unique_path_and_a_well_formed_drawing() {
        let mut paths = std::collections::HashSet::new();
        for icon in Icon::ALL {
            assert!(paths.insert(icon.path()), "{icon:?} repeats a path");
            assert!(icon.path().starts_with("icons/") && icon.path().ends_with(".svg"));
            assert_eq!(Icon::from_path(icon.path()), Some(icon));
            let svg = icon.svg();
            assert!(svg.starts_with("<svg ") && svg.ends_with("</svg>"));
            // Each element is self-closing, so tags must balance trivially.
            let body = icon.body();
            assert_eq!(
                body.matches('<').count(),
                body.matches("/>").count(),
                "{icon:?} has an unclosed element"
            );
            assert_eq!(body.matches('"').count() % 2, 0, "{icon:?} quotes");
        }
        assert_eq!(Icon::from_path("icons/missing.svg"), None);
    }
}
