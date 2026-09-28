//! Descripción de los parámetros de una plantilla. La GUI arma el formulario a partir de esto y
//! la CLI lo muestra con `jmi schema <plantilla>`.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    /// Clave en el JSON de parámetros.
    pub key: &'static str,
    pub label: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<&'static str>,
    #[serde(flatten)]
    pub kind: FieldKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FieldKind {
    Text { multiline: bool },
    Number { min: f64, max: f64, step: f64 },
    Color,
    Select { options: Vec<Choice> },
}

#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

impl Field {
    pub fn text(key: &'static str, label: &'static str) -> Self {
        Self { key, label, help: None, kind: FieldKind::Text { multiline: false } }
    }
    pub fn textarea(key: &'static str, label: &'static str) -> Self {
        Self { key, label, help: None, kind: FieldKind::Text { multiline: true } }
    }
    pub fn number(key: &'static str, label: &'static str, min: f64, max: f64, step: f64) -> Self {
        Self { key, label, help: None, kind: FieldKind::Number { min, max, step } }
    }
    pub fn color(key: &'static str, label: &'static str) -> Self {
        Self { key, label, help: None, kind: FieldKind::Color }
    }
    pub fn select(key: &'static str, label: &'static str, options: &[(&'static str, &'static str)]) -> Self {
        let options = options.iter().map(|&(value, label)| Choice { value, label }).collect();
        Self { key, label, help: None, kind: FieldKind::Select { options } }
    }
    pub fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);
        self
    }
}
