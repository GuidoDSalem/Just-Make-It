//! `jmi`: Just Make It desde la terminal. Todo lo que hace la GUI, para scripts y agentes.
//!
//!   jmi templates                          lista las plantillas
//!   jmi schema titulo                      campos y parámetros por defecto (JSON)
//!   jmi new titulo -o proyecto.json        crea un proyecto con los valores por defecto
//!   jmi info proyecto.json                 duración, tamaño, fps
//!   jmi frame proyecto.json --at 2s        un frame a PNG (y los problemas que encuentre)
//!   jmi render proyecto.json -o video.mp4  el video
//!   jmi batch proyecto.json filas.csv      un video por fila (cada columna reemplaza un parámetro)
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use jmi_core::{Project, RenderJob, templates};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "jmi", about = "Just Make It: videos desde plantillas y un JSON", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Lista las plantillas disponibles.
    Templates,
    /// Campos y parámetros por defecto de una plantilla, en JSON.
    Schema { template: String },
    /// Crea un proyecto con los parámetros por defecto de una plantilla.
    New {
        template: String,
        #[arg(short, long, default_value = "proyecto.json")]
        output: PathBuf,
    },
    /// Duración, tamaño y fps del video de un proyecto.
    Info { project: PathBuf },
    /// Renderiza un frame a PNG. Tiempos: 2s, 50%, end, 42 (frame).
    Frame {
        project: PathBuf,
        #[arg(long, default_value = "1s")]
        at: String,
        #[arg(short, long, default_value = "frame.png")]
        output: PathBuf,
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
    },
    /// Renderiza el video.
    Render {
        project: PathBuf,
        #[arg(short, long, default_value = "out.mp4")]
        output: PathBuf,
        /// 0.5 = borrador a la mitad, 2 = 4K.
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
        /// Sólo una parte: "2s..5s".
        #[arg(long)]
        range: Option<String>,
    },
    /// Un video por fila de un CSV o de un JSON (lista de objetos); cada columna reemplaza un parámetro.
    Batch {
        project: PathBuf,
        rows: PathBuf,
        #[arg(short, long, default_value = "out")]
        output_dir: PathBuf,
        /// Columna para nombrar los archivos (si no, 001.mp4, 002.mp4…).
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Templates => {
            for t in templates() {
                let i = t.info();
                println!("{:<10} {}  —  {}", i.id, i.name, i.description);
            }
        }
        Cmd::Schema { template } => {
            let t = jmi_core::template(&template).with_context(|| format!("no existe la plantilla '{template}'"))?;
            println!("{}", serde_json::to_string_pretty(&t.info())?);
        }
        Cmd::New { template, output } => {
            let p = Project::new(&template)?;
            std::fs::write(&output, serde_json::to_string_pretty(&p)? + "\n")?;
            println!("{}", output.display());
        }
        Cmd::Info { project } => {
            let p = Project::load(&project)?;
            println!("{}", serde_json::to_string_pretty(&p.template()?.video_info(&p.params)?)?);
        }
        Cmd::Frame { project, at, output, scale } => {
            let p = Project::load(&project)?;
            let prev = p.template()?.preview(&p.params, &at, scale)?;
            std::fs::write(&output, &prev.png)?;
            println!("{} (frame {}, {:.2}s)", output.display(), prev.frame, prev.seconds);
            for d in &prev.diagnostics {
                eprintln!("{}: {}", d.severity, d.message);
            }
        }
        Cmd::Render { project, output, scale, range } => {
            let p = Project::load(&project)?;
            let mut job = RenderJob::new(&output);
            job.scale = scale;
            job.range = range;
            render_with_progress(&p, &job)?;
        }
        Cmd::Batch { project, rows, output_dir, name, scale } => {
            let base = Project::load(&project)?;
            let rows = read_rows(&rows)?;
            std::fs::create_dir_all(&output_dir)?;
            for (i, row) in rows.iter().enumerate() {
                let file = name
                    .as_ref()
                    .and_then(|k| row.get(k))
                    .map(|v| slug(&v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("{:03}", i + 1));
                let mut job = RenderJob::new(output_dir.join(format!("{file}.mp4")));
                job.scale = scale;
                eprintln!("[{}/{}] {}", i + 1, rows.len(), job.output.display());
                render_with_progress(&base.with_overrides(row), &job)?;
            }
        }
    }
    Ok(())
}

fn render_with_progress(p: &Project, job: &RenderJob) -> Result<()> {
    let t = p.template()?;
    let t0 = Instant::now();
    let progress = job.progress.clone();
    let result = std::thread::scope(|s| {
        let h = s.spawn(|| t.render(&p.params, job));
        while !h.is_finished() {
            let (d, n) = (progress.done.load(Ordering::Relaxed), progress.total.load(Ordering::Relaxed));
            if n > 0 {
                eprint!("\r{d}/{n} frames ({:.0}%)   ", 100.0 * d as f32 / n as f32);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        h.join().expect("el hilo de render entró en pánico")
    });
    eprintln!();
    result?;
    println!("{} en {:.1}s", job.output.display(), t0.elapsed().as_secs_f32());
    Ok(())
}

/// Filas de un CSV (encabezados = parámetros; los números se leen como números) o de un JSON.
fn read_rows(path: &Path) -> Result<Vec<Map<String, Value>>> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    if path.extension().is_some_and(|e| e == "json") {
        let v: Value = serde_json::from_str(&text)?;
        let Value::Array(items) = v else { bail!("{}: se esperaba una lista de objetos", path.display()) };
        return items
            .into_iter()
            .map(|i| match i {
                Value::Object(m) => Ok(m),
                _ => bail!("{}: cada fila tiene que ser un objeto", path.display()),
            })
            .collect();
    }
    let mut rdr = csv::Reader::from_reader(text.as_bytes());
    let headers = rdr.headers()?.clone();
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        let mut m = Map::new();
        for (h, v) in headers.iter().zip(rec.iter()) {
            let val = v.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number).unwrap_or_else(|| Value::String(v.replace("\\n", "\n")));
            m.insert(h.to_string(), val);
        }
        rows.push(m);
    }
    Ok(rows)
}

fn slug(s: &str) -> String {
    let s: String = s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}
