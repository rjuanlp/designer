//! Interface text. The initial language is Spanish; keeping every label here
//! prepares the interface for translation. All copy is original.

use std::{fmt::Display, path::Path};

use designer_core::NodeKind;

/// Application name, also used in the window title.
pub const APP_NAME: &str = "Designer";

/// Top-level sections of the application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Local documents (S01).
    Documents,
    /// Design workspace (S02), which also hosts the timeline (S04).
    Design,
    /// Components and styles (S03).
    Components,
    /// Interactive preview (S05).
    Preview,
    /// Import and migration (S06).
    Import,
    /// Preferences and agents (S07).
    Preferences,
}

impl Screen {
    /// Every section, in navigation order.
    pub const ALL: [Self; 6] = [
        Self::Documents,
        Self::Design,
        Self::Components,
        Self::Preview,
        Self::Import,
        Self::Preferences,
    ];

    /// Navigation label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Documents => "Documentos",
            Self::Design => "Diseño",
            Self::Components => "Componentes",
            Self::Preview => "Vista previa",
            Self::Import => "Importar",
            Self::Preferences => "Preferencias",
        }
    }

    /// What the section will do, shown while it is not implemented.
    pub fn pending_description(self) -> &'static str {
        match self {
            Self::Documents | Self::Design | Self::Preferences => "",
            Self::Components => {
                "Aquí vivirán los componentes, las variantes y los estilos locales del documento."
            }
            Self::Preview => {
                "Aquí se probarán los estados y las transiciones interactivas del diseño."
            }
            Self::Import => {
                "Aquí se importarán paquetes de datos estructurados y se mostrará el informe de pérdidas."
            }
        }
    }
}

/// Canvas tools offered by the tool bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Select and move.
    Move,
    /// Create a frame.
    Frame,
    /// Create a rectangle.
    Rectangle,
    /// Create an ellipse.
    Ellipse,
    /// Create a text layer.
    Text,
    /// Draw a Bézier path.
    Pen,
}

impl Tool {
    /// Every tool, in tool bar order.
    pub const ALL: [Self; 6] = [
        Self::Move,
        Self::Frame,
        Self::Rectangle,
        Self::Ellipse,
        Self::Text,
        Self::Pen,
    ];

    /// Tool bar label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Mover",
            Self::Frame => "Marco",
            Self::Rectangle => "Rectángulo",
            Self::Ellipse => "Elipse",
            Self::Text => "Texto",
            Self::Pen => "Pluma",
        }
    }
}

/// Panel shown in the narrow workspace layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NarrowPanel {
    /// Pages and layers.
    Layers,
    /// The canvas.
    Canvas,
    /// The inspector.
    Inspector,
}

impl NarrowPanel {
    /// Every panel, in switcher order.
    pub const ALL: [Self; 3] = [Self::Layers, Self::Canvas, Self::Inspector];

    /// Switcher label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Layers => "Capas",
            Self::Canvas => "Lienzo",
            Self::Inspector => "Propiedades",
        }
    }
}

/// User-facing name of a node kind.
pub fn kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Frame => "Marco",
        NodeKind::Group => "Grupo",
        NodeKind::Rectangle { .. } => "Rectángulo",
        NodeKind::Ellipse => "Elipse",
        NodeKind::Text { .. } => "Texto",
    }
}

pub const PAGES: &str = "Páginas";
pub const LAYERS: &str = "Capas";
pub const NO_LAYERS: &str = "Esta página todavía no tiene capas.";
pub const HIDDEN: &str = "Oculta";
pub const LOCKED: &str = "Bloqueada";

pub const PROPERTIES: &str = "Propiedades";
pub const NOTHING_SELECTED: &str = "Selecciona una capa para ver sus propiedades.";
pub const READ_ONLY_NOTE: &str =
    "Las capas se crean, mueven y borran en el lienzo. Estos campos todavía no se editan.";
pub const NAME: &str = "Nombre";
pub const KIND: &str = "Tipo";
pub const POSITION_X: &str = "X";
pub const POSITION_Y: &str = "Y";
pub const WIDTH: &str = "Ancho";
pub const HEIGHT: &str = "Alto";
pub const RADIUS: &str = "Radio";
pub const FILL: &str = "Relleno";
pub const NO_FILL: &str = "Sin relleno";
pub const OPACITY: &str = "Opacidad";
pub const CONTENT: &str = "Contenido";
pub const VISIBLE: &str = "Visible";
pub const YES: &str = "Sí";
pub const NO: &str = "No";
pub const TEXT_NOT_RENDERED: &str = "El texto aún no se dibuja en el lienzo.";

pub const TIMELINE: &str = "Línea de tiempo";
pub const SHOW_TIMELINE: &str = "Mostrar línea de tiempo";
pub const HIDE_TIMELINE: &str = "Ocultar línea de tiempo";
pub const NO_ANIMATIONS: &str = "Este documento todavía no tiene animaciones.";
pub const TOOL_PENDING: &str = "Las herramientas Texto y Pluma todavía no crean capas.";
pub const UNDO: &str = "Deshacer";
pub const REDO: &str = "Rehacer";
pub const SAVE: &str = "Guardar";

pub const CANVAS_UNAVAILABLE: &str = "No se puede mostrar el lienzo";
pub const EMPTY_PAGE: &str = "Página vacía";

/// Name of a document that has no file yet.
pub const UNTITLED: &str = "Sin título";
pub const CURRENT_DOCUMENT: &str = "Documento abierto";
pub const NOT_SAVED_YET: &str = "Todavía no se ha guardado en un archivo.";
pub const NEW_DOCUMENT: &str = "Nuevo documento";
pub const OPEN_DOCUMENT: &str = "Abrir un archivo .dsg…";
pub const SAVE_AS: &str = "Guardar como…";
pub const NO_RECENT_DOCUMENTS: &str =
    "La lista de documentos recientes todavía no está disponible.";
pub const UNSAVED_TITLE: &str = "Hay cambios sin guardar";
pub const UNSAVED_DETAIL: &str = "Si continúas, los cambios de este documento se perderán.";
pub const DISCARD: &str = "Descartar cambios";
pub const KEEP_EDITING: &str = "Seguir editando";

/// Status after a successful save.
pub fn saved(path: Option<&Path>) -> String {
    match path {
        Some(path) => format!("Guardado en {}", path.display()),
        None => "Guardado".to_owned(),
    }
}

/// Status after a failed save. The technical reason is kept in English.
pub fn save_failed(error: &dyn Display) -> String {
    format!("No se pudo guardar; el archivo anterior no ha cambiado. Detalle: {error}")
}

/// Status after a file could not be opened.
pub fn open_failed(path: &Path, error: &dyn Display) -> String {
    format!("No se pudo abrir {}. Detalle: {error}", path.display())
}

/// Status after a change was rejected by the document model.
pub fn change_failed(error: &dyn Display) -> String {
    format!("No se pudo aplicar el cambio. Detalle: {error}")
}

/// Status after the system file dialog failed.
pub fn dialog_failed(error: &dyn Display) -> String {
    format!("No se pudo mostrar el diálogo de archivos. Detalle: {error}")
}
pub const OPEN_SAMPLE: &str = "Abrir el documento de ejemplo";

pub const PENDING_TITLE: &str = "Sección pendiente";
pub const APPEARANCE: &str = "Apariencia";
pub const THEME: &str = "Tema";
pub const THEME_DARK: &str = "Oscuro";
pub const THEME_LIGHT: &str = "Claro";
pub const GRAPHICS: &str = "Gráficos";
pub const GRAPHICS_ADAPTER: &str = "Adaptador";
pub const GRAPHICS_UNAVAILABLE: &str = "No disponible";
pub const AGENTS: &str = "Agentes";
pub const AGENTS_PENDING: &str = "La conexión con agentes (MCP) todavía no está implementada.";
pub const SHORTCUTS: &str = "Atajos";
pub const SHORTCUT_LIST: [(&str, &str); 11] = [
    ("Ctrl+N", "Nuevo documento"),
    ("Ctrl+O", "Abrir un archivo .dsg"),
    ("Ctrl+S / Ctrl+Mayús+S", "Guardar / Guardar como"),
    ("Ctrl+Z", "Deshacer"),
    ("Ctrl+Mayús+Z o Ctrl+Y", "Rehacer"),
    ("Supr o Retroceso", "Borrar la capa seleccionada"),
    ("Esc", "Cancelar el arrastre o quitar la selección"),
    ("Tab / Mayús+Tab", "Mover el foco"),
    ("Intro o Espacio", "Activar el control enfocado"),
    ("Ctrl+T", "Mostrar u ocultar la línea de tiempo"),
    ("Ctrl+Mayús+L", "Cambiar entre tema claro y oscuro"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_label_is_present_and_unique_within_its_group() {
        fn check(labels: &[&str]) {
            for (index, label) in labels.iter().enumerate() {
                assert!(!label.trim().is_empty());
                assert!(!labels[..index].contains(label), "duplicate label {label}");
            }
        }
        check(&Screen::ALL.map(Screen::label));
        check(&Tool::ALL.map(Tool::label));
        check(&NarrowPanel::ALL.map(NarrowPanel::label));
    }

    #[test]
    fn only_unimplemented_sections_have_a_pending_description() {
        for screen in Screen::ALL {
            let implemented = matches!(
                screen,
                Screen::Documents | Screen::Design | Screen::Preferences
            );
            assert_eq!(screen.pending_description().is_empty(), implemented);
        }
    }
}
