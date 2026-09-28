//! Promo con música: frases que entran en los tiempos fuertes de una canción.
//!
//! Cada frase aparece de golpe en el primer tiempo de un compás y dura `beats_per_phrase` beats;
//! todo late con el pulso (la frase, el fondo y un contador de tiempos). El BPM y el primer tiempo
//! salen del análisis de la canción (`media_store`), o de `bpm` si se fuerza a mano.
use super::{VideoTemplate, ease_in_out, ease_out, prog};
use crate::beats::Tempo;
use crate::fields::Field;
use crate::media_store::{self, MediaInfo};
use anyhow::Result;
use fframes::{AudioMap, AudioTimestamp, AudioTrack, Color, Duration, FFramesContext, FontQuery, Frame, Svgr, Transform, Video};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const W: usize = 1920;
const H: usize = 1080;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    /// Id de la canción subida (vacío = sin música, pulso de 120 BPM).
    pub audio: String,
    /// Volumen de la música en dB (0 = original).
    pub volume_db: f32,
    /// Una frase por línea.
    pub phrases: String,
    pub beats_per_phrase: String,
    /// Compás en el que entra la primera frase (para saltear una intro).
    pub start_bar: f32,
    /// BPM forzado a mano (0 = el detectado).
    pub bpm: f32,
    pub font: String,
    pub size: f32,
    pub uppercase: String,
    pub background: String,
    pub ink: String,
    pub accent: String,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            audio: String::new(),
            volume_db: -3.0,
            phrases: "Just Make It\nvideos desde un JSON\ncon música\nen el ritmo".into(),
            beats_per_phrase: "4".into(),
            start_bar: 1.0,
            bpm: 0.0,
            font: "Inter 24pt".into(),
            size: 170.0,
            uppercase: "yes".into(),
            background: "#111111".into(),
            ink: "#fafafa".into(),
            accent: "#f97316".into(),
        }
    }
}

pub struct Promo;

impl VideoTemplate for Promo {
    type Params = Params;
    type Video = PromoVideo;
    const ID: &'static str = "promo";
    const NAME: &'static str = "Promo con música";
    const DESCRIPTION: &'static str = "Frases que entran en los tiempos fuertes de una canción, con todo latiendo al pulso.";

    fn fields() -> Vec<Field> {
        vec![
            Field::audio("audio", "Canción").help("Subí una canción con su licencia; el BPM se detecta solo."),
            Field::number("volume_db", "Volumen (dB)", -30.0, 6.0, 1.0),
            Field::textarea("phrases", "Frases").help("Una por línea; cada una entra en un tiempo fuerte."),
            Field::select("beats_per_phrase", "Duración de cada frase", &[("2", "2 tiempos"), ("4", "1 compás"), ("8", "2 compases"), ("16", "4 compases")]),
            Field::number("start_bar", "Compás de entrada", 0.0, 64.0, 1.0).help("Para saltear la intro de la canción."),
            Field::number("bpm", "BPM a mano (0 = detectado)", 0.0, 220.0, 0.5),
            Field::select("font", "Tipografía", &crate::font_choices()),
            Field::number("size", "Tamaño máximo (px)", 60.0, 260.0, 2.0),
            Field::select("uppercase", "Mayúsculas", &[("yes", "Sí"), ("no", "No")]),
            Field::color("background", "Fondo"),
            Field::color("ink", "Texto"),
            Field::color("accent", "Acento"),
        ]
    }

    fn media(p: &Params) -> Result<Option<MediaInfo>> {
        if p.audio.trim().is_empty() { Ok(None) } else { media_store::get(p.audio.trim()).map(Some) }
    }

    fn build(p: Params) -> Result<PromoVideo> {
        let media = Self::media(&p)?;
        let mut tempo = media.as_ref().and_then(|m| m.tempo.clone()).unwrap_or_else(|| Tempo::fixed(120.0, 600.0));
        if p.bpm > 0.0 {
            tempo.bpm = p.bpm;
            tempo.period = 60.0 / p.bpm;
        }
        let upper = p.uppercase != "no";
        let phrases: Vec<String> = p
            .phrases
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| if upper { l.to_uppercase() } else { l.to_string() })
            .collect();
        let bpp: f32 = p.beats_per_phrase.parse().unwrap_or(4.0f32).clamp(1.0, 64.0);
        let first = p.start_bar.max(0.0).floor() * 4.0;
        // termina un compás después de la última frase, sin pasarse de la canción
        let end_beat = first + bpp * phrases.len().max(1) as f32 + 4.0;
        let mut duration = tempo.beat_time(end_beat).max(2.0);
        if media.is_some() {
            duration = duration.min(tempo.duration);
        }
        Ok(PromoVideo {
            weight: crate::font_weight(&p.font),
            file: media.map(|m| m.file),
            p,
            phrases,
            tempo,
            first,
            bpp,
            duration,
            sizes: OnceLock::new(),
        })
    }
}

pub struct PromoVideo {
    p: Params,
    weight: u16,
    /// Nombre del archivo de la canción dentro de su carpeta.
    file: Option<String>,
    phrases: Vec<String>,
    tempo: Tempo,
    /// Beat (desde el primer tiempo fuerte) en que entra la primera frase.
    first: f32,
    bpp: f32,
    duration: f32,
    sizes: OnceLock<Vec<f32>>,
}

impl std::fmt::Debug for PromoVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromoVideo").field("bpm", &self.tempo.bpm).field("phrases", &self.phrases.len()).finish()
    }
}

impl PromoVideo {
    fn sizes<'a>(&'a self, frame: &mut Frame, ctx: &FFramesContext<'a, '_>) -> Vec<f32> {
        if let Some(s) = self.sizes.get() {
            return s.clone();
        }
        let mut measured = true;
        let sizes: Vec<f32> = self
            .phrases
            .iter()
            .map(|t| {
                let font = FontQuery { family: &self.p.font, size: 100, weight: self.weight, ..Default::default() };
                match frame.text_width(ctx, font, t) {
                    Some(w) if w > 0 => self.p.size.min(100.0 * W as f32 * 0.86 / w as f32).floor(),
                    Some(_) => self.p.size,
                    None => {
                        measured = false;
                        self.p.size
                    }
                }
            })
            .collect();
        if measured {
            let _ = self.sizes.set(sizes.clone());
        }
        sizes
    }
}

/// Pulso que decae después de cada beat: 1 en el golpe, ~0 a mitad de camino.
fn beat_pulse(beat: f32) -> f32 {
    if beat < 0.0 { 0.0 } else { (-(beat.fract()) * 7.0).exp() }
}

impl Video for PromoVideo {
    const FPS: usize = 30;
    const WIDTH: usize = W;
    const HEIGHT: usize = H;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(self.duration)
    }

    fn audio(&self) -> AudioMap<'_> {
        match &self.file {
            Some(file) => AudioMap::from([AudioTrack::new(file, AudioTimestamp::Second(0.0)..AudioTimestamp::Second(self.duration))
                .gain_db(self.p.volume_db)
                .fade_out(1.5)]),
            None => AudioMap::none(),
        }
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let sizes = self.sizes(&mut frame, ctx);
        let p = &self.p;
        let beat = self.tempo.beat_at(t);
        let pulse = beat_pulse(beat);
        let fade = 1.0 - ease_in_out(prog(t, self.duration - 1.0, self.duration - 0.05));

        // frase actual (entra de golpe en su tiempo fuerte y queda hasta la siguiente)
        let k = ((beat - self.first) / self.bpp).floor();
        let current = if k < 0.0 { None } else { Some((k as usize).min(self.phrases.len().saturating_sub(1))) };
        let since = beat - (self.first + k.max(0.0) * self.bpp);
        let entering = ease_out(prog(since, 0.0, 0.5));
        let flash = if current.is_some() && (k as usize) < self.phrases.len() { (-since * 5.0).exp() } else { 0.0 };

        let phrase: Svgr<'a> = match current.and_then(|i| self.phrases.get(i).map(|s| (i, s))) {
            Some((i, text)) => {
                let size = sizes[i];
                let s = 1.12 - 0.12 * entering + 0.025 * pulse;
                let (cx, cy) = (W as f32 / 2.0, H as f32 / 2.0 + size * 0.35);
                let tr = Transform { scale: s.into(), ..Transform::translate(cx * (1.0 - s), cy * (1.0 - s)) };
                fframes::svgr!(
                    <g transform={tr} opacity={prog(since, 0.0, 0.08)}>
                        <text x={cx} y={cy} font-family={p.font.as_str()} font-size={size} font-weight={self.weight} fill={p.ink.as_str()} text-anchor="middle">
                            {text.as_str()}
                        </text>
                    </g>
                )
            }
            None => Svgr::empty(),
        };

        // contador de tiempos del compás
        let in_bar = beat.floor().rem_euclid(4.0) as usize;
        let dots: Vec<Svgr<'a>> = (0..4)
            .map(|i| {
                let on = beat >= 0.0 && i == in_bar;
                let r = if on { 9.0 + 5.0 * pulse } else { 7.0 };
                let fill = if on { p.accent.as_str() } else { p.ink.as_str() };
                let op = if on { 1.0 } else { 0.25 };
                fframes::svgr!(<circle cx={96.0 + i as f32 * 36.0} cy={H as f32 - 90.0} r={r} fill={fill} fill-opacity={op} />)
            })
            .collect();

        let progress = (t / self.duration).clamp(0.0, 1.0);
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox={format!("0 0 {W} {H}")} width={W} height={H}>
                <rect width={W} height={H} fill={p.background.as_str()} />
                <rect width={W} height={H} fill={p.accent.as_str()} fill-opacity={0.22 * flash} />
                <g opacity={fade}>
                    {phrase}
                    {dots}
                    <rect x="0" y={H as f32 - 6.0} width={(W as f32 * progress).max(0.5)} height="6" fill={p.accent.as_str()} />
                    <text x={W as f32 - 96.0} y={H as f32 - 80.0} font-family="JetBrains Mono" font-size="28" font-weight="400" fill={p.ink.as_str()} fill-opacity="0.45" text-anchor="end">
                        {format!("{:.0} BPM", self.tempo.bpm)}
                    </text>
                </g>
            </svg>
        )
    }
}
