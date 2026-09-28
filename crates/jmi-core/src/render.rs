//! Render de un video completo y de frames sueltos, genérico sobre cualquier `fframes::Video`.
use anyhow::{Context, Result, anyhow};
use fframes::{
    AbortSignal, CombinedMediaProvider, CpuFrameRenderer, EncoderOptions, FFramesLogger, FFramesLoggerVariant,
    MediaDirectory, MediaProvider, Previewer, RenderOptions, Video, cpu::CpuRenderingBackend,
};
use serde::Serialize;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Progreso de un render: frames hechos sobre el total. Se comparte entre hilos (la GUI lo consulta).
#[derive(Debug, Default)]
pub struct Progress {
    pub total: AtomicUsize,
    pub done: AtomicUsize,
}

impl Progress {
    /// 0..1
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 { 0.0 } else { self.done.load(Ordering::Relaxed) as f32 / total as f32 }
    }
}

struct ProgressLogger(Arc<Progress>);

impl FFramesLogger for ProgressLogger {
    fn init_frames_rendering(&self, frames: usize) -> fframes::FFramesRendererResult<()> {
        self.0.total.store(frames, Ordering::Relaxed);
        Ok(())
    }
    fn log_frame(&self, _index: usize, _thread: usize) {
        self.0.done.fetch_add(1, Ordering::Relaxed);
    }
    fn success(&self, _output: &std::path::Path, _tmp: Option<&PathBuf>) {}
}

/// Un pedido de render: a dónde va el archivo y cómo seguirlo o cancelarlo.
#[derive(Clone)]
pub struct RenderJob {
    pub output: PathBuf,
    /// 1.0 = tamaño de la plantilla (1920x1080), 0.5 = borrador a la mitad, 2.0 = 4K.
    pub scale: f64,
    /// Rango de tiempo en la sintaxis de fframes (`"2s..5s"`), o todo el video.
    pub range: Option<String>,
    pub progress: Arc<Progress>,
    pub abort: Arc<AbortSignal>,
}

impl RenderJob {
    pub fn new(output: impl Into<PathBuf>) -> Self {
        Self { output: output.into(), scale: 1.0, range: None, progress: Arc::default(), abort: Arc::default() }
    }
}

/// Un problema que fframes encontró en un frame (texto cortado, tipografía faltante, …).
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub severity: String,
    pub message: String,
}

/// Un frame renderizado para la vista previa.
#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    #[serde(skip)]
    pub png: Vec<u8>,
    pub frame: usize,
    pub seconds: f32,
    pub diagnostics: Vec<Diagnostic>,
}

/// Datos del video para la GUI (duración, tamaño, fps).
#[derive(Debug, Clone, Serialize)]
pub struct VideoInfo {
    pub width: usize,
    pub height: usize,
    pub fps: usize,
    pub frames: usize,
    pub seconds: f32,
}

fn options<'a, 'm>(media: &'m (dyn MediaProvider<'m> + 'm), scale: f64) -> RenderOptions<'a, 'm> {
    RenderOptions {
        media: Some(media),
        default_font: "DM Sans",
        scale_resolution: scale,
        video_encoder_options: EncoderOptions {
            preferred_encoder: Some("libx264"),
            codec_params: Some(&[("crf", "20"), ("preset", "medium")]),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Deja en `$m` los medios embebidos (tipografías) más, si hay, los de una carpeta del usuario
/// (una canción). Es una macro porque el proveedor combinado toma prestadas variables locales.
macro_rules! media_provider {
    ($extra:expr => $m:ident) => {
        let folder = $extra.map(|dir: &Path| MediaDirectory::read_folder(dir).with_context(|| dir.display().to_string())).transpose()?;
        let dynamic = folder.as_ref().map(|f| f.process_media_source()).transpose()?;
        let combined;
        let $m: &dyn MediaProvider = match &dynamic {
            Some(d) => {
                combined = CombinedMediaProvider::from([crate::media() as &dyn MediaProvider, d as &dyn MediaProvider]);
                &combined
            }
            None => crate::media(),
        };
    };
}

/// Renderiza el video a `job.output`. Bloquea hasta terminar (usar en un hilo aparte).
pub fn render_video<V: Video + Send + Sync>(video: &V, media: Option<&Path>, job: &RenderJob) -> Result<()> {
    if let Some(parent) = job.output.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    media_provider!(media => m);
    let mut opts = options(m, job.scale);
    opts.logger = FFramesLoggerVariant::Custom(Arc::new(ProgressLogger(job.progress.clone())));
    opts.abort_signal = Some(&job.abort);
    if let Some(range) = &job.range {
        let previewer = Previewer::new(video, &options(m, 1.0))?;
        opts.frame_range = Some(previewer.timeline().resolve_range(range).map_err(|e| anyhow!("rango {range:?}: {e}"))?);
    }
    fframes::render(&job.output, video, CpuRenderingBackend::default(), &opts)
        .with_context(|| format!("render de {}", job.output.display()))
}

/// Renderiza un frame (tiempo en la sintaxis de fframes: `"1.5s"`, `"50%"`, `"end"`, `"42"`) a PNG.
pub fn render_preview<V: Video + Send + Sync>(video: &V, media: Option<&Path>, at: &str, scale: f64) -> Result<Preview> {
    media_provider!(media => m);
    let mut previewer = Previewer::new(video, &options(m, scale))?;
    let frame = previewer.timeline().resolve_frame(at).map_err(|e| anyhow!("tiempo {at:?}: {e}"))?;
    let (pixels, report) = previewer.render_inspected(frame, &mut CpuFrameRenderer::default())?;
    let mut png = Vec::new();
    pixels.into_image().write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)?;
    Ok(Preview {
        png,
        frame,
        seconds: report.seconds,
        diagnostics: report
            .diagnostics
            .into_iter()
            .map(|d| Diagnostic { severity: format!("{:?}", d.severity).to_lowercase(), message: d.message })
            .collect(),
    })
}

pub fn video_info<V: Video + Send + Sync>(video: &V, media: Option<&Path>) -> Result<VideoInfo> {
    media_provider!(media => m);
    let previewer = Previewer::new(video, &options(m, 1.0))?;
    let t = previewer.timeline_report();
    Ok(VideoInfo { width: t.width, height: t.height, fps: t.fps, frames: t.duration_frames, seconds: t.duration_seconds })
}
