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
            Self::Documents | Self::Design | Self::Preview | Self::Preferences => "",
            Self::Components => {
                "Aquí vivirán los componentes, las variantes y los estilos locales del documento."
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
        NodeKind::Path { .. } => "Trazado",
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
pub const FILL_OPACITY: &str = "Opacidad del relleno";
pub const CONTENT: &str = "Contenido";
pub const VISIBLE: &str = "Visible";
pub const YES: &str = "Sí";
pub const NO: &str = "No";
pub const TEXT_NOT_RENDERED: &str = "El texto aún no se dibuja en el lienzo.";

pub const TIMELINE: &str = "Línea de tiempo";
pub const SHOW_TIMELINE: &str = "Mostrar línea de tiempo";
pub const HIDE_TIMELINE: &str = "Ocultar línea de tiempo";
pub const NO_ANIMATIONS: &str = "Este documento todavía no tiene animaciones.";
pub const PLAY: &str = "Reproducir";
/// Stable playback announcement while the clock is running.
pub const PLAYING: &str = "Reproduciendo";
pub const PAUSE: &str = "Pausar";
pub const BASE_VALUES: &str = "Editar base";
pub const TIME_BACK: &str = "−100 ms";
pub const TIME_FORWARD: &str = "+100 ms";
pub const DURATION: &str = "Duración";
pub const DURATION_LESS: &str = "−500 ms";
pub const DURATION_MORE: &str = "+500 ms";
pub const ADD_KEY: &str = "Añadir clave";
pub const REPLACE_KEY: &str = "Actualizar clave";
pub const REMOVE_KEY: &str = "Quitar clave";
pub const VALUE_LESS: &str = "− Valor";
pub const VALUE_MORE: &str = "+ Valor";
pub const LINEAR: &str = "Lineal";
pub const CONSTANT: &str = "Constante";
pub const EASE_IN: &str = "Entrada suave";
pub const EASE_OUT: &str = "Salida suave";
pub const EASE_IN_OUT: &str = "Entrada/salida suave";
pub const ANIMATION_SELECT: &str = "Selecciona una capa para crear claves.";
pub const ANIMATION_HINT: &str =
    "Claves de la propiedad seleccionada. Mayús: ajuste fino del valor.";
pub const ANIMATION_EDIT_HINT: &str =
    "Previsualizando animación. Usa Editar base para dibujar o mover capas.";
pub const MAIN_SEQUENCE: &str = "Principal";
pub const ADD_SEQUENCE: &str = "Añadir secuencia";
pub const REMOVE_SEQUENCE: &str = "Quitar secuencia";
pub const PREVIEW_HELP: &str =
    "Prueba los estados sobre el lienzo. Selecciona una capa en Diseño para crear interacciones.";
pub const PROTOTYPE_STATES: &str = "Estados interactivos";
pub const CREATE_HOVER_STATE: &str = "Crear estado al pasar";
pub const CREATE_CLICK_STATE: &str = "Crear estado al pulsar";
pub const NO_PROTOTYPE_STATES: &str = "Añade un estado desde una capa seleccionada.";
pub const SELECTED_LAYER: &str = "Capa seleccionada";
pub const NO_SELECTED_LAYER: &str = "Ninguna";
pub const PROTOTYPE_LAYERS: &str = "Capas para interacción";
pub const PREVIEW_INITIAL: &str = "Inicial";
pub const PREVIEW_SELECT_NODE: &str = "Selecciona una capa en el panel para crear su interacción.";
pub const PROTOTYPE_STATE_LIMIT: &str = "No se pueden crear más estados en esta página.";
pub const PROTOTYPE_TRIGGER_EXISTS: &str =
    "Esta capa ya tiene una interacción para ese disparador desde el estado inicial.";
pub const PROTOTYPE_STATE_CREATED: &str = "Se añadió el estado interactivo.";
pub const PROTOTYPE_STATE_REMOVED: &str = "Se quitó el estado interactivo.";
pub const REMOVE_PROTOTYPE_STATE: &str = "Quitar estado activo";
pub const IMPORT_DESCRIPTION: &str =
    "Carga JSON estructurado de Figma. Revisa las pérdidas antes de abrirlo como documento.";
pub const IMPORT_PICK_FILE: &str = "Elegir archivo JSON";
pub const IMPORT_APPLY: &str = "Abrir como documento";
pub const IMPORT_LOADING: &str = "Leyendo y validando el archivo…";
pub const IMPORT_NO_RESULT: &str = "Selecciona un archivo JSON estructurado para empezar.";
pub const IMPORT_REPORT: &str = "Informe de migración";
pub const IMPORT_WARNINGS: &str = "Elementos que requieren revisión";
pub const IMPORT_NO_WARNINGS: &str = "No se detectaron pérdidas en los elementos importados.";
pub fn import_more_warnings(count: usize) -> String {
    format!("Se muestran los primeros 100 de {count} avisos.")
}

pub fn import_omitted_warnings(count: usize) -> String {
    format!("No se muestran los detalles de otros {count} avisos por el límite del informe.")
}

pub fn import_summary(nodes: usize, warnings: usize) -> String {
    format!("{nodes} capas preparadas · {warnings} avisos")
}

pub fn import_applied(nodes: usize, warnings: usize) -> String {
    format!("Documento importado: {nodes} capas · {warnings} avisos")
}

pub fn import_failed(error: &dyn Display) -> String {
    format!("No se pudo importar el archivo. Detalle: {error}")
}

pub fn property_label(property: designer_core::Property) -> &'static str {
    use designer_core::Property;
    match property {
        Property::X => POSITION_X,
        Property::Y => POSITION_Y,
        Property::Width => WIDTH,
        Property::Height => HEIGHT,
        Property::Opacity => OPACITY,
    }
}

pub fn milliseconds(time: u32) -> String {
    format!("{time} ms")
}
pub fn animation_time(time: u32, duration: u32) -> String {
    format!("{time} / {duration} ms")
}
pub fn animation_value(name: &str, value: f64) -> String {
    format!("{name}: {value:.2}")
}
pub fn key_label(time: u32, value: f64) -> String {
    format!("◆ {time} ms · {value:.2}")
}

pub fn easing_label(easing: designer_core::Easing) -> &'static str {
    use designer_core::Easing;
    match easing {
        Easing::Constant => CONSTANT,
        Easing::Linear => LINEAR,
        Easing::EaseIn => EASE_IN,
        Easing::EaseOut => EASE_OUT,
        Easing::EaseInOut => EASE_IN_OUT,
    }
}

pub const TOOL_PENDING: &str = "Las herramientas Texto y Pluma todavía no crean capas.";
pub const UNDO: &str = "Deshacer";
pub const REDO: &str = "Rehacer";
pub const DELETE_SELECTION: &str = "Eliminar capa seleccionada";
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
pub fn save_failed(error: &designer_engine::dsg::DsgError) -> String {
    if matches!(error, designer_engine::dsg::DsgError::Locked { .. }) {
        return DOCUMENT_LOCKED.to_owned();
    }
    format!("No se pudo guardar; el archivo anterior no ha cambiado. Detalle: {error}")
}

/// Status after a file could not be opened.
pub fn open_failed(path: &Path, error: &designer_engine::dsg::DsgError) -> String {
    if matches!(error, designer_engine::dsg::DsgError::Locked { .. }) {
        return DOCUMENT_LOCKED.to_owned();
    }
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
pub const AGENTS_PENDING: &str =
    "Los clientes MCP locales pueden conectarse a la sesión abierta de Designer.";
pub const MCP_UPDATED: &str = "El documento se actualizó desde un cliente MCP.";

pub fn mcp_server_failed(error: &dyn Display) -> String {
    format!(
        "Otra ventana de Designer ya tiene la conexión MCP en vivo, o no se pudo iniciar el puente local. Detalle: {error}"
    )
}
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

/// Accessibility navigation group.
pub const SECTIONS: &str = "Secciones";
/// Accessibility toolbar group.
pub const TOOLS_ACTIONS: &str = "Herramientas y acciones";

/// Document lock error.
pub const DOCUMENT_LOCKED: &str =
    "Otro proceso tiene abierto este documento. Cierra esa sesión antes de abrirlo o guardarlo.";

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
                Screen::Documents | Screen::Design | Screen::Preview | Screen::Preferences
            );
            assert_eq!(screen.pending_description().is_empty(), implemented);
        }
    }
}
