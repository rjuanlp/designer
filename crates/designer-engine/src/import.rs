//! Import the supported subset of Figma's structured file JSON.

use std::{
    collections::HashSet,
    error::Error,
    fmt, fs,
    io::{self, Read},
    path::Path,
};

use designer_core::{
    Color, Document, MAX_NAME_CHARS, MAX_TEXT_CHARS, ModelError, Node, NodeId, NodeKind, Page,
    PageId, Rect,
};
use serde::Serialize;
use serde_json::Value;

/// Largest source JSON accepted by the structured importer.
pub const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;
/// Largest number of source nodes accepted by one import.
pub const MAX_IMPORT_NODES: usize = 50_000;
/// Largest number of pages accepted by one import.
pub const MAX_IMPORT_PAGES: usize = 256;
const MAX_WARNINGS: usize = 10_000;

/// Why a structured import could not be completed.
#[derive(Debug)]
pub enum ImportError {
    /// The source file could not be read.
    Io(io::Error),
    /// The source is not valid JSON.
    InvalidJson(String),
    /// The JSON does not contain a Figma document tree.
    MissingDocument,
    /// The source document has no pages.
    NoPages,
    /// A source limit was exceeded.
    TooLarge(&'static str),
    /// The imported model violates a document invariant.
    InvalidDocument(ModelError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "could not read source: {error}"),
            Self::InvalidJson(error) => write!(f, "invalid source JSON: {error}"),
            Self::MissingDocument => write!(f, "source JSON has no Figma document tree"),
            Self::NoPages => write!(f, "source document has no pages"),
            Self::TooLarge(what) => write!(f, "source {what} exceeds the import limit"),
            Self::InvalidDocument(error) => write!(f, "imported document is invalid: {error}"),
        }
    }
}

impl Error for ImportError {}

impl From<io::Error> for ImportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A source node whose identity was mapped into the numeric document model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImportMapping {
    /// ID in the source JSON.
    pub source_id: String,
    /// ID in the imported document.
    pub node: NodeId,
}

/// A reported loss or normalization during import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImportWarning {
    /// Location in the source tree.
    pub source_path: String,
    /// What could not be represented exactly.
    pub detail: String,
}

/// Counts and details needed to review the import before saving it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    /// Source tree nodes visited.
    pub source_nodes: usize,
    /// Nodes represented in the destination.
    pub imported_nodes: usize,
    /// Stable source-to-destination identifier map.
    pub id_map: Vec<ImportMapping>,
    /// Unsupported data and approximations.
    pub warnings: Vec<ImportWarning>,
    /// Additional warnings omitted after reaching the in-memory report limit.
    pub omitted_warnings: usize,
}

/// Imported editable document plus its loss report.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportResult {
    /// Destination model.
    pub document: Document,
    /// Source mapping and unsupported-feature report.
    pub report: ImportReport,
}

/// Read a structured JSON export from a local file.
pub fn import_figma_file(path: &Path) -> Result<ImportResult, ImportError> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge("file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMPORT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge("file"));
    }
    import_figma_json(&bytes)
}

/// Parse a structured Figma JSON export.
pub fn import_figma_json(bytes: &[u8]) -> Result<ImportResult, ImportError> {
    if bytes.len() as u64 > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge("JSON"));
    }
    let root: Value = serde_json::from_slice(bytes)
        .map_err(|error| ImportError::InvalidJson(error.to_string()))?;
    let document = root
        .get("document")
        .filter(|value| value.is_object())
        .ok_or(ImportError::MissingDocument)?;
    let source_pages = document
        .get("children")
        .and_then(Value::as_array)
        .ok_or(ImportError::MissingDocument)?;
    if source_pages.is_empty() {
        return Err(ImportError::NoPages);
    }
    if source_pages.len() > MAX_IMPORT_PAGES {
        return Err(ImportError::TooLarge("page count"));
    }

    let mut context = ImportContext {
        next_node: 1,
        report: ImportReport::default(),
        source_ids: HashSet::new(),
    };
    let mut pages = Vec::with_capacity(source_pages.len());
    for (index, source_page) in source_pages.iter().enumerate() {
        if source_page.get("type").and_then(Value::as_str) != Some("CANVAS") {
            context.warn(
                &format!("page-{index}"),
                "La entrada de página no está marcada como CANVAS.".to_owned(),
            );
        }
        let mut name = string(source_page, "name")
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("Página {}", index + 1));
        if name.chars().count() > MAX_NAME_CHARS {
            context.warn(
                &name,
                format!("El nombre de página se acortó a {MAX_NAME_CHARS} caracteres."),
            );
            name = name.chars().take(MAX_NAME_CHARS).collect();
        }
        let nodes = match source_page.get("children") {
            Some(children) => children
                .as_array()
                .map(|children| context.nodes(children, (0.0, 0.0), &name))
                .transpose()?
                .unwrap_or_else(|| {
                    context.warn(
                        &name,
                        "La lista de nodos de la página no es válida y se omitió.".to_owned(),
                    );
                    Vec::new()
                }),
            None => Vec::new(),
        };
        pages.push(Page {
            id: PageId(index as u64 + 1),
            name,
            background: Color::rgb(30, 31, 36),
            nodes,
            animation: Default::default(),
            prototype: Default::default(),
        });
    }
    let mut name = string(document, "name")
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Importación estructurada".to_owned());
    if name.chars().count() > MAX_NAME_CHARS {
        context.warn(
            "document",
            format!("El nombre del documento se acortó a {MAX_NAME_CHARS} caracteres."),
        );
        name = name.chars().take(MAX_NAME_CHARS).collect();
    }
    let document = Document {
        name,
        pages,
        next_node_id: context.next_node,
    };
    document.validate().map_err(ImportError::InvalidDocument)?;
    Ok(ImportResult {
        document,
        report: context.report,
    })
}

struct ImportContext {
    next_node: u64,
    report: ImportReport,
    source_ids: HashSet<String>,
}

impl ImportContext {
    fn nodes(
        &mut self,
        source_nodes: &[Value],
        parent_origin: (f64, f64),
        parent_path: &str,
    ) -> Result<Vec<Node>, ImportError> {
        source_nodes
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let source_id = string(source, "id")
                    .unwrap_or_else(|| format!("generated-node-{}", self.next_node));
                let name = string(source, "name")
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| format!("Capa {}", index + 1));
                if string(source, "name").is_none_or(|name| name.trim().is_empty()) {
                    self.warn(
                        parent_path,
                        "Una capa sin nombre recibió un nombre generado.".to_owned(),
                    );
                }
                let source_path = limited_path(parent_path, &name);
                self.node(source, parent_origin, &source_path, source_id, name)
            })
            .collect()
    }

    fn node(
        &mut self,
        source: &Value,
        parent_origin: (f64, f64),
        source_path: &str,
        source_id: String,
        mut name: String,
    ) -> Result<Node, ImportError> {
        self.report.source_nodes += 1;
        if self.report.source_nodes > MAX_IMPORT_NODES {
            return Err(ImportError::TooLarge("node count"));
        }
        if !self.source_ids.insert(source_id.clone()) {
            self.warn(
                source_path,
                format!("El identificador de origen {source_id:?} está repetido."),
            );
        }
        let id = NodeId(self.next_node);
        self.next_node = self
            .next_node
            .checked_add(1)
            .ok_or(ImportError::TooLarge("node identifier"))?;
        self.report.imported_nodes += 1;
        self.report.id_map.push(ImportMapping {
            source_id,
            node: id,
        });
        if name.chars().count() > MAX_NAME_CHARS {
            self.warn(
                source_path,
                format!("El nombre se acortó a {MAX_NAME_CHARS} caracteres."),
            );
            name = name.chars().take(MAX_NAME_CHARS).collect();
        }

        let kind_name = string(source, "type").unwrap_or_default();
        let kind = match kind_name.as_str() {
            "FRAME" | "COMPONENT" | "INSTANCE" | "COMPONENT_SET" | "SECTION" => NodeKind::Frame,
            "GROUP" => NodeKind::Group,
            "RECTANGLE" => {
                let corner_radius = number(source, "cornerRadius").unwrap_or(0.0);
                if source.get("cornerRadius").is_some() && number(source, "cornerRadius").is_none()
                {
                    self.warn(
                        source_path,
                        "El radio de esquina no es numérico y se reemplazó por cero.".to_owned(),
                    );
                }
                if corner_radius < 0.0 {
                    self.warn(
                        source_path,
                        "El radio negativo de esquina se limitó a cero.".to_owned(),
                    );
                }
                NodeKind::Rectangle {
                    corner_radius: corner_radius.max(0.0),
                }
            }
            "ELLIPSE" => NodeKind::Ellipse,
            "TEXT" => {
                if source.get("style").is_some() || source.get("characterStyleOverrides").is_some()
                {
                    self.warn(
                        source_path,
                        "Se conserva el texto, pero no su tipografía ni estilo.".to_owned(),
                    );
                }
                let mut content = string(source, "characters").unwrap_or_default();
                if content.chars().count() > MAX_TEXT_CHARS {
                    self.warn(
                        source_path,
                        format!("El texto se acortó a {MAX_TEXT_CHARS} caracteres."),
                    );
                    content = content.chars().take(MAX_TEXT_CHARS).collect();
                }
                NodeKind::Text { content }
            }
            "DOCUMENT" | "CANVAS" => {
                return Err(ImportError::MissingDocument);
            }
            _ => {
                self.warn(
                    source_path,
                    format!("La geometría {kind_name:?} se conserva como grupo sin dibujo."),
                );
                NodeKind::Group
            }
        };
        if matches!(
            kind_name.as_str(),
            "COMPONENT" | "INSTANCE" | "COMPONENT_SET"
        ) {
            self.warn(
                source_path,
                "Las referencias a componentes y sus propiedades no se importan.".to_owned(),
            );
        }
        if source.get("effects").is_some_and(|effects| {
            effects
                .as_array()
                .is_some_and(|effects| !effects.is_empty())
        }) {
            self.warn(
                source_path,
                "Los efectos visuales no se importan.".to_owned(),
            );
        }
        if number(source, "rotation").is_some_and(|rotation| rotation != 0.0) {
            self.warn(
                source_path,
                "La rotación se aproxima con los límites absolutos del nodo.".to_owned(),
            );
        }
        if source
            .get("strokes")
            .and_then(Value::as_array)
            .is_some_and(|strokes| !strokes.is_empty())
        {
            self.warn(
                source_path,
                "Los trazos de contorno no se importan.".to_owned(),
            );
        }
        if source.get("layoutMode").is_some() || source.get("constraints").is_some() {
            self.warn(
                source_path,
                "Las reglas de auto-layout y constraints no se importan.".to_owned(),
            );
        }
        if source
            .get("blendMode")
            .and_then(Value::as_str)
            .is_some_and(|mode| mode != "NORMAL")
        {
            self.warn(source_path, "El modo de fusión no se importa.".to_owned());
        }
        if source.get("clipsContent").and_then(Value::as_bool) == Some(true)
            || source.get("isMask").and_then(Value::as_bool) == Some(true)
        {
            self.warn(
                source_path,
                "El recorte de contenido y las máscaras no se importan.".to_owned(),
            );
        }
        if source.get("opacity").is_some() && number(source, "opacity").is_none() {
            self.warn(
                source_path,
                "La opacidad no es numérica y se reemplazó por 1.".to_owned(),
            );
        }
        let bounds = self.bounds(source, parent_origin, source_path);
        let fill = self.fill(source, source_path);
        let opacity = match number(source, "opacity") {
            Some(opacity) if opacity.is_finite() => {
                if !(0.0..=1.0).contains(&opacity) {
                    self.warn(
                        source_path,
                        "La opacidad se limitó al intervalo 0–1.".to_owned(),
                    );
                }
                opacity.clamp(0.0, 1.0)
            }
            Some(_) => {
                self.warn(
                    source_path,
                    "La opacidad inválida se reemplazó por 1.".to_owned(),
                );
                1.0
            }
            None => 1.0,
        };
        let visible = source
            .get("visible")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if source
            .get("visible")
            .is_some_and(|visible| !visible.is_boolean())
        {
            self.warn(
                source_path,
                "El valor visible no es booleano y se reemplazó por verdadero.".to_owned(),
            );
        }
        let locked = source
            .get("locked")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let children = match source.get("children") {
            Some(value) => value
                .as_array()
                .map(|children| {
                    self.nodes(
                        children,
                        (bounds.x + parent_origin.0, bounds.y + parent_origin.1),
                        source_path,
                    )
                })
                .transpose()?
                .unwrap_or_else(|| {
                    self.warn(
                        source_path,
                        "La lista de nodos hijos no es válida y se omitió.".to_owned(),
                    );
                    Vec::new()
                }),
            None => Vec::new(),
        };
        Ok(Node {
            id,
            name,
            kind,
            bounds,
            fill,
            opacity,
            visible,
            locked,
            children,
        })
    }

    fn bounds(&mut self, source: &Value, parent_origin: (f64, f64), path: &str) -> Rect {
        let absolute = source.get("absoluteBoundingBox");
        let read = |key| absolute.and_then(|bounds| number(bounds, key));
        match (read("x"), read("y"), read("width"), read("height")) {
            (Some(x), Some(y), Some(width), Some(height))
                if [x, y, width, height].iter().all(|value| value.is_finite())
                    && width >= 0.0
                    && height >= 0.0 =>
            {
                Rect::new(x - parent_origin.0, y - parent_origin.1, width, height)
            }
            _ => {
                self.warn(
                    path,
                    "No hay límites absolutos válidos; se usa un marco de 100 × 100.".to_owned(),
                );
                Rect::new(0.0, 0.0, 100.0, 100.0)
            }
        }
    }

    fn fill(&mut self, source: &Value, path: &str) -> Option<Color> {
        let fills = source.get("fills")?;
        let Some(paints) = fills.as_array() else {
            self.warn(
                path,
                "La lista de rellenos no es válida y se omitió.".to_owned(),
            );
            return None;
        };
        let visible_paints = paints
            .iter()
            .filter(|paint| paint.get("visible").and_then(Value::as_bool) != Some(false))
            .count();
        if visible_paints > 1 {
            self.warn(
                path,
                "Solo se importa un relleno sólido visible; los siguientes se descartan."
                    .to_owned(),
            );
        }
        for paint in paints {
            if paint.get("visible").and_then(Value::as_bool) == Some(false) {
                continue;
            }
            if paint
                .get("visible")
                .is_some_and(|visible| !visible.is_boolean())
            {
                self.warn(
                    path,
                    "La visibilidad del relleno no es booleana y se trató como visible.".to_owned(),
                );
            }
            if string(paint, "type").as_deref() != Some("SOLID") {
                self.warn(
                    path,
                    "Se omitió un relleno que no es de color sólido.".to_owned(),
                );
                continue;
            }
            let Some(color) = paint.get("color") else {
                self.warn(path, "Un relleno sólido no tiene color válido.".to_owned());
                continue;
            };
            if ["r", "g", "b"]
                .iter()
                .any(|key| number(color, key).is_none())
            {
                self.warn(
                    path,
                    "Un relleno sólido tiene componentes incompletos.".to_owned(),
                );
                continue;
            }
            if color.get("a").is_some() && number(color, "a").is_none() {
                self.warn(
                    path,
                    "El canal alfa del relleno no es numérico y se reemplazó por 1.".to_owned(),
                );
            }
            if ["r", "g", "b", "a"]
                .iter()
                .filter_map(|key| number(color, key))
                .any(|value| !(0.0..=1.0).contains(&value))
            {
                self.warn(
                    path,
                    "Los componentes de color fuera de 0–1 se limitaron al intervalo.".to_owned(),
                );
            }
            let component = |key| number(color, key).unwrap_or(0.0).clamp(0.0, 1.0);
            if paint.get("opacity").is_some() && number(paint, "opacity").is_none() {
                self.warn(
                    path,
                    "La opacidad del relleno no es numérica y se reemplazó por 1.".to_owned(),
                );
            }
            let paint_opacity = match number(paint, "opacity") {
                Some(opacity) if opacity.is_finite() => {
                    if !(0.0..=1.0).contains(&opacity) {
                        self.warn(path, "La opacidad del relleno se limitó a 0–1.".to_owned());
                    }
                    opacity.clamp(0.0, 1.0)
                }
                Some(_) => {
                    self.warn(
                        path,
                        "La opacidad inválida del relleno se reemplazó por 1.".to_owned(),
                    );
                    1.0
                }
                None => 1.0,
            };
            let alpha = number(color, "a").unwrap_or(1.0).clamp(0.0, 1.0);
            return Some(Color {
                r: (component("r") * 255.0).round() as u8,
                g: (component("g") * 255.0).round() as u8,
                b: (component("b") * 255.0).round() as u8,
                a: (alpha * paint_opacity * 255.0).round() as u8,
            });
        }
        None
    }

    fn warn(&mut self, path: &str, detail: String) {
        if self.report.warnings.len() < MAX_WARNINGS {
            self.report.warnings.push(ImportWarning {
                source_path: path.to_owned(),
                detail,
            });
        } else {
            self.report.omitted_warnings += 1;
        }
    }
}

fn limited_path(parent: &str, name: &str) -> String {
    const MAX_PATH_CHARS: usize = 1024;
    let path = format!("{parent}/{name}");
    if path.chars().count() <= MAX_PATH_CHARS {
        return path;
    }
    let mut shortened: String = path.chars().take(MAX_PATH_CHARS - 3).collect();
    shortened.push_str("...");
    shortened
}

fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

fn number(value: &Value, key: &str) -> Option<f64> {
    value.get(key)?.as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"{
      "document": {
        "name": "Kit",
        "children": [{
          "id": "page:1",
          "name": "Main",
          "type": "CANVAS",
          "children": [{
            "id": "shape:1",
            "name": "Button",
            "type": "RECTANGLE",
            "absoluteBoundingBox": {"x": 20, "y": 30, "width": 120, "height": 40},
            "fills": [{"type": "SOLID", "color": {"r": 0.2, "g": 0.4, "b": 0.6}}]
          }]
        }]
      }
    }"#;

    #[test]
    fn imports_supported_hierarchy_and_reports_identifier_mapping() {
        let result = import_figma_json(SOURCE.as_bytes()).expect("valid import");
        assert_eq!(result.document.name, "Kit");
        assert_eq!(result.document.pages[0].name, "Main");
        let node = &result.document.pages[0].nodes[0];
        assert_eq!(node.id, NodeId(1));
        assert_eq!(node.bounds, Rect::new(20.0, 30.0, 120.0, 40.0));
        assert_eq!(
            node.fill,
            Some(Color {
                r: 51,
                g: 102,
                b: 153,
                a: 255
            })
        );
        assert_eq!(result.report.id_map[0].source_id, "shape:1");
        assert_eq!(result.report.imported_nodes, 1);
        assert!(result.report.warnings.is_empty());
        assert!(result.document.validate().is_ok());
    }

    #[test]
    fn unsupported_geometry_is_preserved_as_a_reported_group() {
        let source = SOURCE.replace("RECTANGLE", "VECTOR");
        let result = import_figma_json(source.as_bytes()).expect("valid import");
        assert!(matches!(
            result.document.pages[0].nodes[0].kind,
            NodeKind::Group
        ));
        assert_eq!(result.report.warnings.len(), 1);
    }

    #[test]
    fn nested_bounds_are_relative_to_the_parent() {
        let mut root = serde_json::from_str::<Value>(SOURCE).expect("valid fixture");
        root["document"]["children"][0]["children"][0]["children"] = serde_json::json!([{
            "id": "shape:child",
            "name": "Icon",
            "type": "ELLIPSE",
            "absoluteBoundingBox": {"x": 25, "y": 40, "width": 10, "height": 10}
        }]);
        let result = import_figma_json(root.to_string().as_bytes()).expect("valid import");
        assert_eq!(
            result.document.pages[0].nodes[0].children[0].bounds,
            Rect::new(5.0, 10.0, 10.0, 10.0)
        );
    }

    #[test]
    fn lossy_text_style_and_out_of_range_values_are_reported() {
        let mut root = serde_json::from_str::<Value>(SOURCE).expect("valid fixture");
        let node = &mut root["document"]["children"][0]["children"][0];
        node["type"] = Value::String("TEXT".to_owned());
        node["name"] = Value::String("N".repeat(MAX_NAME_CHARS + 5));
        node["characters"] = Value::String("x".repeat(MAX_TEXT_CHARS + 5));
        node["style"] = serde_json::json!({"fontFamily": "Example"});
        node["rotation"] = serde_json::json!(12);
        node["opacity"] = serde_json::json!(1.5);
        node["layoutMode"] = Value::String("HORIZONTAL".to_owned());
        node["strokes"] = serde_json::json!([{"type": "SOLID"}]);
        node["fills"] = serde_json::json!([
            {"type": "SOLID", "color": {"r": 0.1}},
            {"type": "SOLID", "color": {"r": 0.2, "g": 0.3, "b": 0.4}}
        ]);
        let result = import_figma_json(root.to_string().as_bytes()).expect("valid import");
        let imported = &result.document.pages[0].nodes[0];
        assert_eq!(imported.name.chars().count(), MAX_NAME_CHARS);
        assert!(matches!(
            &imported.kind,
            NodeKind::Text { content } if content.chars().count() == MAX_TEXT_CHARS
        ));
        assert_eq!(imported.opacity, 1.0);
        assert_eq!(
            imported.fill,
            Some(Color {
                r: 51,
                g: 77,
                b: 102,
                a: 255
            })
        );
        for expected in [
            "nombre se acortó",
            "texto se acortó",
            "tipografía",
            "rotación",
            "opacidad se limitó",
            "contorno",
            "auto-layout",
            "componentes incompletos",
            "Solo se importa",
        ] {
            assert!(
                result
                    .report
                    .warnings
                    .iter()
                    .any(|warning| warning.detail.contains(expected)),
                "missing warning containing {expected:?}"
            );
        }
    }

    #[test]
    fn malformed_optional_values_are_reported_instead_of_silently_normalized() {
        let mut root = serde_json::from_str::<Value>(SOURCE).expect("valid fixture");
        let node = &mut root["document"]["children"][0]["children"][0];
        node["opacity"] = Value::String("opaque".to_owned());
        node["visible"] = Value::String("yes".to_owned());
        node["fills"] = serde_json::json!({"not": "an array"});
        node["children"] = Value::String("not an array".to_owned());
        let result = import_figma_json(root.to_string().as_bytes()).expect("valid import");
        let details: Vec<_> = result
            .report
            .warnings
            .iter()
            .map(|warning| warning.detail.as_str())
            .collect();
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("no es numérica"))
        );
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("no es booleano"))
        );
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("lista de rellenos"))
        );
        assert!(details.iter().any(|detail| detail.contains("nodos hijos")));
    }

    #[test]
    fn malformed_fill_metadata_is_reported() {
        let mut root = serde_json::from_str::<Value>(SOURCE).expect("valid fixture");
        root["document"]["children"][0]["children"][0]["fills"] = serde_json::json!([{
            "type": "SOLID",
            "visible": "yes",
            "opacity": "half",
            "color": {"r": 0.1, "g": 0.2, "b": 0.3, "a": "opaque"}
        }]);
        let result = import_figma_json(root.to_string().as_bytes()).expect("valid import");
        let details: Vec<_> = result
            .report
            .warnings
            .iter()
            .map(|warning| warning.detail.as_str())
            .collect();
        assert!(details.iter().any(|detail| detail.contains("visibilidad")));
        assert!(details.iter().any(|detail| detail.contains("canal alfa")));
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("opacidad del relleno"))
        );
    }

    #[test]
    fn warning_report_limit_counts_omitted_details() {
        let mut context = ImportContext {
            next_node: 1,
            report: ImportReport::default(),
            source_ids: HashSet::new(),
        };
        for index in 0..MAX_WARNINGS + 3 {
            context.warn("page/node", format!("warning {index}"));
        }
        assert_eq!(context.report.warnings.len(), MAX_WARNINGS);
        assert_eq!(context.report.omitted_warnings, 3);
    }

    #[test]
    fn malformed_sources_and_resource_limits_are_rejected() {
        assert!(matches!(
            import_figma_json(b"{}"),
            Err(ImportError::MissingDocument)
        ));
        assert!(matches!(
            import_figma_json(b"["),
            Err(ImportError::InvalidJson(_))
        ));
        assert!(matches!(
            import_figma_json(&vec![b' '; MAX_IMPORT_BYTES as usize + 1]),
            Err(ImportError::TooLarge("JSON"))
        ));
    }
}
