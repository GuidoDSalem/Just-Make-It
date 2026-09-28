//! `jmi-server`: la API HTTP del motor y la GUI web.
//!
//!   GET  /api/templates            plantillas (campos y valores por defecto)
//!   POST /api/info                 {template, params} → duración, tamaño, fps
//!   POST /api/preview              {template, params, at, scale} → PNG (base64) y problemas
//!   POST /api/render               {template, params, scale} → {id}
//!   GET  /api/jobs/{id}            estado y progreso de un render
//!   POST /api/jobs/{id}/cancel     cancela un render
//!   GET  /api/jobs/{id}/file       el .mp4
//!   GET  /                         la GUI (gui/dist)
use anyhow::anyhow;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use clap::Parser;
use jmi_core::{Project, RenderJob, TemplateInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Parser)]
#[command(name = "jmi-server", about = "Just Make It: API y GUI")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8787", env = "JMI_ADDR")]
    addr: SocketAddr,
    /// Carpeta de la GUI compilada (`npm run build` en gui/).
    #[arg(long, default_value = "gui/dist", env = "JMI_GUI_DIR")]
    gui: PathBuf,
    /// Dónde quedan los videos renderizados.
    #[arg(long, default_value = "out/jobs", env = "JMI_OUT_DIR")]
    out: PathBuf,
}

#[derive(Clone)]
struct AppState {
    out: PathBuf,
    jobs: Arc<Mutex<HashMap<u64, Job>>>,
    next: Arc<AtomicU64>,
}

#[derive(Clone)]
struct Job {
    render: RenderJob,
    status: Arc<Mutex<JobStatus>>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum JobStatus {
    Running,
    Done { seconds: f32 },
    Failed { error: String },
    Cancelled,
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": format!("{:#}", self.0) }))).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    std::fs::create_dir_all(&args.out)?;
    let state = AppState { out: args.out.clone(), jobs: Arc::default(), next: Arc::new(AtomicU64::new(1)) };

    let api = Router::new()
        .route("/templates", get(list_templates))
        .route("/info", post(info))
        .route("/preview", post(preview))
        .route("/render", post(render))
        .route("/jobs/{id}", get(job_status))
        .route("/jobs/{id}/cancel", post(job_cancel))
        .route("/jobs/{id}/file", get(job_file))
        .with_state(state);

    let gui = ServeDir::new(&args.gui).fallback(ServeFile::new(args.gui.join("index.html")));
    let app = Router::new().nest("/api", api).fallback_service(gui).layer(CorsLayer::permissive());

    // (carga los medios antes del primer pedido)
    jmi_core::media();
    let listener = tokio::net::TcpListener::bind(args.addr).await?;
    println!("Just Make It en http://{}", args.addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn list_templates() -> Json<Vec<TemplateInfo>> {
    Json(jmi_core::templates().iter().map(|t| t.info()).collect())
}

async fn info(Json(p): Json<Project>) -> ApiResult<Json<jmi_core::VideoInfo>> {
    let info = tokio::task::spawn_blocking(move || p.template()?.video_info(&p.params)).await??;
    Ok(Json(info))
}

#[derive(Deserialize)]
struct PreviewReq {
    #[serde(flatten)]
    project: Project,
    #[serde(default = "default_at")]
    at: String,
    #[serde(default = "default_preview_scale")]
    scale: f64,
}

fn default_at() -> String {
    "0".into()
}
fn default_preview_scale() -> f64 {
    0.5
}

#[derive(Serialize)]
struct PreviewRes {
    png: String,
    frame: usize,
    seconds: f32,
    diagnostics: Vec<jmi_core::Diagnostic>,
}

async fn preview(Json(req): Json<PreviewReq>) -> ApiResult<Json<PreviewRes>> {
    let p = tokio::task::spawn_blocking(move || {
        req.project.template()?.preview(&req.project.params, &req.at, req.scale.clamp(0.1, 2.0))
    })
    .await??;
    Ok(Json(PreviewRes {
        png: base64::engine::general_purpose::STANDARD.encode(&p.png),
        frame: p.frame,
        seconds: p.seconds,
        diagnostics: p.diagnostics,
    }))
}

#[derive(Deserialize)]
struct RenderReq {
    #[serde(flatten)]
    project: Project,
    #[serde(default = "one")]
    scale: f64,
}

fn one() -> f64 {
    1.0
}

async fn render(State(s): State<AppState>, Json(req): Json<RenderReq>) -> ApiResult<Json<Value>> {
    let tpl = req.project.template()?;
    let id = s.next.fetch_add(1, Ordering::Relaxed);
    let mut render = RenderJob::new(s.out.join(format!("{id}-{}.mp4", req.project.template)));
    render.scale = req.scale.clamp(0.25, 2.0);
    let job = Job { render: render.clone(), status: Arc::new(Mutex::new(JobStatus::Running)) };
    s.jobs.lock().unwrap().insert(id, job.clone());

    let params = req.project.params;
    tokio::task::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let result = tpl.render(&params, &render);
        *job.status.lock().unwrap() = match result {
            _ if render.abort.is_aborted() => JobStatus::Cancelled,
            Ok(()) => JobStatus::Done { seconds: t0.elapsed().as_secs_f32() },
            Err(e) => JobStatus::Failed { error: format!("{e:#}") },
        };
    });
    Ok(Json(serde_json::json!({ "id": id })))
}

fn find(s: &AppState, id: u64) -> ApiResult<Job> {
    s.jobs.lock().unwrap().get(&id).cloned().ok_or_else(|| ApiError(anyhow!("no existe el render {id}")))
}

async fn job_status(State(s): State<AppState>, Path(id): Path<u64>) -> ApiResult<Json<Value>> {
    let job = find(&s, id)?;
    let mut v = serde_json::to_value(&*job.status.lock().unwrap())?;
    v["id"] = id.into();
    v["progress"] = job.render.progress.fraction().into();
    v["frames"] = job.render.progress.done.load(Ordering::Relaxed).into();
    v["total"] = job.render.progress.total.load(Ordering::Relaxed).into();
    Ok(Json(v))
}

async fn job_cancel(State(s): State<AppState>, Path(id): Path<u64>) -> ApiResult<StatusCode> {
    find(&s, id)?.render.abort.abort();
    Ok(StatusCode::NO_CONTENT)
}

async fn job_file(State(s): State<AppState>, Path(id): Path<u64>) -> ApiResult<Response> {
    let job = find(&s, id)?;
    if !matches!(*job.status.lock().unwrap(), JobStatus::Done { .. }) {
        return Err(ApiError(anyhow!("el render {id} no terminó")));
    }
    let bytes = tokio::fs::read(&job.render.output).await?;
    let name = job.render.output.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok((
        [(header::CONTENT_TYPE, "video/mp4".to_string()), (header::CONTENT_DISPOSITION, format!("inline; filename=\"{name}\""))],
        bytes,
    )
        .into_response())
}
