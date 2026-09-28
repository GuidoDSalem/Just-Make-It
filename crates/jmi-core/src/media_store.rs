//! Los medios que sube el usuario (por ahora, canciones). Cada archivo vive en su carpeta:
//!
//! ```text
//! <raíz>/<id>/cancion.mp3
//! <raíz>/<id>/meta.json     nombre, licencia, fuente y el análisis de tempo
//! ```
//!
//! El id sale del contenido del archivo: subir dos veces la misma canción no la duplica.
//! La raíz se toma de `JMI_MEDIA_DIR` (por defecto `media`) o de `set_root`.
use crate::beats::{self, Tempo};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

static ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Cambia la carpeta de medios (el servidor y la CLI la fijan al arrancar).
pub fn set_root(path: impl Into<PathBuf>) {
    *ROOT.write().unwrap() = Some(path.into());
}

pub fn root() -> PathBuf {
    ROOT.read()
        .unwrap()
        .clone()
        .unwrap_or_else(|| std::env::var_os("JMI_MEDIA_DIR").map(PathBuf::from).unwrap_or_else(|| "media".into()))
}

/// De dónde salió un archivo y bajo qué licencia se usa. Queda en los créditos de cada video.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct License {
    /// Por ejemplo "Pixabay Content License", "CC BY 4.0", "Epidemic Sound (suscripción)", "Propia".
    pub license: String,
    /// Link a la página de la canción o al comprobante.
    #[serde(default)]
    pub source: String,
    /// Autor, para la atribución que piden algunas licencias (CC BY).
    #[serde(default)]
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub id: String,
    /// Nombre original del archivo.
    pub name: String,
    /// Nombre del archivo dentro de su carpeta.
    pub file: String,
    pub kind: String,
    #[serde(flatten)]
    pub license: License,
    pub tempo: Option<Tempo>,
    pub bytes: u64,
}

impl MediaInfo {
    pub fn dir(&self) -> PathBuf {
        root().join(&self.id)
    }
    pub fn path(&self) -> PathBuf {
        self.dir().join(&self.file)
    }
    /// Línea de créditos: "Canción — Autor · Licencia · fuente".
    pub fn credit(&self) -> String {
        let l = &self.license;
        let mut s = self.name.clone();
        if !l.author.is_empty() {
            s += &format!(" — {}", l.author);
        }
        s += &format!(" · {}", if l.license.is_empty() { "licencia sin indicar" } else { &l.license });
        if !l.source.is_empty() {
            s += &format!(" · {}", l.source);
        }
        s
    }
}

const AUDIO: &[&str] = &["mp3", "wav", "flac", "ogg", "aac"];

fn content_id(bytes: &[u8]) -> String {
    // FNV-1a de 64 bits: alcanza para no duplicar, no es criptográfico
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn safe_name(name: &str, ext: &str) -> String {
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("archivo");
    let s: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let s = s.trim_matches('-');
    format!("{}.{ext}", if s.is_empty() { "archivo" } else { s })
}

/// Guarda una canción, la analiza y devuelve su ficha. La licencia es obligatoria.
pub fn add_audio(name: &str, bytes: &[u8], license: License) -> Result<MediaInfo> {
    if license.license.trim().is_empty() {
        bail!("indicá la licencia de la canción (por ejemplo \"Pixabay Content License\", \"CC BY 4.0\" o \"Propia\")");
    }
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    if !AUDIO.contains(&ext.as_str()) {
        bail!("formato no soportado: .{ext} (se aceptan {})", AUDIO.join(", "));
    }
    let id = content_id(bytes);
    let dir = root().join(&id);
    std::fs::create_dir_all(&dir).with_context(|| dir.display().to_string())?;
    let file = safe_name(name, &ext);
    std::fs::write(dir.join(&file), bytes)?;

    let tempo = match analyze_file(&dir.join(&file)) {
        Ok(t) => Some(t),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e.context("no se pudo leer el audio"));
        }
    };
    let info = MediaInfo { id, name: name.to_string(), file, kind: "audio".into(), license, tempo, bytes: bytes.len() as u64 };
    save(&info)?;
    Ok(info)
}

/// Decodifica un archivo de audio (mono, 22.05 kHz) y detecta su tempo.
pub fn analyze_file(path: &Path) -> Result<Tempo> {
    let audio = fframes::media::PreloadedAudioData::decode_raw_file(Some(22050), &path.to_path_buf())
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(beats::analyze(&audio.samples, audio.sample_rate))
}

pub fn save(info: &MediaInfo) -> Result<()> {
    std::fs::write(info.dir().join("meta.json"), serde_json::to_string_pretty(info)? + "\n")?;
    Ok(())
}

pub fn get(id: &str) -> Result<MediaInfo> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("id de medio inválido: {id:?}");
    }
    let meta = root().join(id).join("meta.json");
    let text = std::fs::read_to_string(&meta).with_context(|| format!("no existe el medio '{id}' en {}", root().display()))?;
    Ok(serde_json::from_str(&text)?)
}

/// Actualiza la licencia.
pub fn update(id: &str, license: Option<License>) -> Result<MediaInfo> {
    let mut info = get(id)?;
    if let Some(l) = license {
        info.license = l;
    }
    save(&info)?;
    Ok(info)
}

pub fn list() -> Vec<MediaInfo> {
    let mut v: Vec<MediaInfo> = std::fs::read_dir(root())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| get(&e.file_name().to_string_lossy()).ok())
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}
