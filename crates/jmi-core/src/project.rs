//! Un proyecto: qué plantilla usar y con qué parámetros. Es el formato común de la GUI, la CLI y
//! la API (un archivo `.json`).
use crate::templates::{Template, template};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub template: String,
    #[serde(default)]
    pub params: Value,
}

impl Project {
    /// Un proyecto nuevo con los parámetros por defecto de la plantilla.
    pub fn new(template_id: &str) -> Result<Self> {
        let t = template(template_id).ok_or_else(|| anyhow!("no existe la plantilla '{template_id}'"))?;
        Ok(Self { template: template_id.into(), params: t.info().defaults })
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|e| anyhow!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| anyhow!("{}: {e}", path.display()))
    }

    pub fn template(&self) -> Result<Box<dyn Template>> {
        template(&self.template).ok_or_else(|| anyhow!("no existe la plantilla '{}'", self.template))
    }

    /// Copia con algunos parámetros reemplazados (para lotes: una fila = un video).
    pub fn with_overrides(&self, overrides: &serde_json::Map<String, Value>) -> Self {
        let mut p = self.clone();
        if !p.params.is_object() {
            p.params = Value::Object(Default::default());
        }
        let obj = p.params.as_object_mut().unwrap();
        for (k, v) in overrides {
            obj.insert(k.clone(), v.clone());
        }
        p
    }
}
