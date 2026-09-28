//! Registro de plantillas.
//!
//! Para agregar una plantilla: un módulo con una struct de parámetros (`Deserialize + Default`,
//! con `#[serde(default)]` para que falte lo que falte), un video de fframes que se construye a
//! partir de ellos, un `impl VideoTemplate` y una línea en `templates()`.
use crate::fields::Field;
use crate::render::{self, Preview, RenderJob, VideoInfo};
use anyhow::{Context, Result};
use fframes::Video;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

pub mod texto;
pub mod titulo;

/// Lo que la GUI y la CLI necesitan saber de una plantilla.
#[derive(Debug, Clone, Serialize)]
pub struct TemplateInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub fields: Vec<Field>,
    /// Parámetros por defecto (un ejemplo completo y válido).
    pub defaults: Value,
}

/// Una plantilla sin tipos: recibe parámetros como JSON. Es lo que usan la CLI y el servidor.
pub trait Template: Send + Sync {
    fn info(&self) -> TemplateInfo;
    fn video_info(&self, params: &Value) -> Result<VideoInfo>;
    fn preview(&self, params: &Value, at: &str, scale: f64) -> Result<Preview>;
    fn render(&self, params: &Value, job: &RenderJob) -> Result<()>;
}

/// Una plantilla con tipos. `Template` se implementa solo para cualquier `VideoTemplate`.
pub trait VideoTemplate: Send + Sync + 'static {
    type Params: DeserializeOwned + Serialize + Default;
    type Video: Video + Send + Sync;
    const ID: &'static str;
    const NAME: &'static str;
    const DESCRIPTION: &'static str;
    fn fields() -> Vec<Field>;
    fn build(params: Self::Params) -> Self::Video;
}

struct Registered<T: VideoTemplate>(std::marker::PhantomData<T>);

impl<T: VideoTemplate> Registered<T> {
    fn video(params: &Value) -> Result<T::Video> {
        let p: T::Params = serde_json::from_value(params.clone()).with_context(|| format!("parámetros de '{}'", T::ID))?;
        Ok(T::build(p))
    }
}

impl<T: VideoTemplate> Template for Registered<T> {
    fn info(&self) -> TemplateInfo {
        TemplateInfo {
            id: T::ID,
            name: T::NAME,
            description: T::DESCRIPTION,
            fields: T::fields(),
            defaults: serde_json::to_value(T::Params::default()).unwrap_or(Value::Null),
        }
    }
    fn video_info(&self, params: &Value) -> Result<VideoInfo> {
        render::video_info(&Self::video(params)?)
    }
    fn preview(&self, params: &Value, at: &str, scale: f64) -> Result<Preview> {
        render::render_preview(&Self::video(params)?, at, scale)
    }
    fn render(&self, params: &Value, job: &RenderJob) -> Result<()> {
        render::render_video(&Self::video(params)?, job)
    }
}

fn reg<T: VideoTemplate>() -> Box<dyn Template> {
    Box::new(Registered::<T>(std::marker::PhantomData))
}

/// Todas las plantillas disponibles.
pub fn templates() -> Vec<Box<dyn Template>> {
    vec![reg::<titulo::Titulo>(), reg::<texto::Texto>()]
}

pub fn template(id: &str) -> Option<Box<dyn Template>> {
    templates().into_iter().find(|t| t.info().id == id)
}

// ------------------------------------------------------------------ utilidades para plantillas

/// 0..1 lineal entre `a` y `b`, recortado.
pub(crate) fn prog(t: f32, a: f32, b: f32) -> f32 {
    if b <= a { return if t >= b { 1.0 } else { 0.0 }; }
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

pub(crate) fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

pub(crate) fn ease_in_out(x: f32) -> f32 {
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
}
