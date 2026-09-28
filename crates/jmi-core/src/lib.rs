//! Just Make It: el motor.
//!
//! Una **plantilla** es un video de fframes que lee sus parámetros de un JSON. El mismo JSON sale de
//! la GUI, de un script o de un agente, y el motor lo convierte en un `.mp4` (`render`) o en un
//! frame suelto para la vista previa (`preview`).
//!
//! ```ignore
//! let tpl = jmi_core::template("titulo").unwrap();
//! let params = serde_json::json!({ "title": "Hola" });
//! tpl.render(&params, &RenderJob::new("out.mp4"))?;
//! ```
pub mod beats;
pub mod fields;
pub mod media_store;
pub mod project;
pub mod render;
pub mod templates;

pub use fields::{Field, FieldKind};
pub use project::Project;
pub use render::{Diagnostic, Preview, Progress, RenderJob, VideoInfo};
pub use templates::{Template, TemplateInfo, template, templates};

use fframes::StaticMediaProvider;
use std::sync::OnceLock;

// Tipografías (y más adelante imágenes o audio) embebidas en el binario. La ruta es relativa a la
// raíz del workspace de Cargo. En la carpeta sólo puede haber medios (las licencias van en FONTS.md).
fframes::include_media_dir!(pub struct JmiMedia, "crates/jmi-core/media");

/// Los medios embebidos, preparados una sola vez.
pub fn media() -> &'static JmiMedia {
    static MEDIA: OnceLock<JmiMedia> = OnceLock::new();
    MEDIA.get_or_init(|| JmiMedia::prepare().expect("no se pudieron preparar los medios embebidos"))
}

/// Familias tipográficas disponibles para las plantillas: (valor, etiqueta, peso).
pub const FONTS: &[(&str, &str, u16)] = &[
    ("DM Sans", "DM Sans", 500),
    ("Inter 24pt", "Inter (bold)", 700),
    ("JetBrains Mono", "JetBrains Mono", 400),
];

/// Peso disponible para una familia (las plantillas sólo usan pesos que existen en `media/`).
pub fn font_weight(family: &str) -> u16 {
    FONTS.iter().find(|f| f.0 == family).map(|f| f.2).unwrap_or(500)
}

/// Opciones de `Field::select` para elegir tipografía.
pub fn font_choices() -> Vec<(&'static str, &'static str)> {
    FONTS.iter().map(|f| (f.0, f.1)).collect()
}
