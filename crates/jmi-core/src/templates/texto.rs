//! Texto palabra por palabra: un texto (relato, letra, frase) aparece a ritmo de lectura.
//!
//! Formato del texto: cada línea es una línea en pantalla; una línea en blanco separa pantallas;
//! `palabra~` sostiene esa palabra un poco más (cada `~` suma medio segundo).
use super::{VideoTemplate, ease_in_out, ease_out, prog};
use crate::fields::Field;
use fframes::{AudioMap, Color, Duration, FFramesContext, FontQuery, Frame, Svgr, Video};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const W: usize = 1920;
const H: usize = 1080;
const MARGIN: f32 = 160.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    pub text: String,
    /// Palabras por minuto.
    pub wpm: f32,
    pub font: String,
    /// Tamaño máximo del texto en px (se achica si una línea no entra).
    pub size: f32,
    pub align: String,
    pub background: String,
    /// Palabra que se está leyendo.
    pub ink: String,
    /// Palabras ya leídas.
    pub dim: String,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            text: "Esto es una prueba\nde texto palabra~ por palabra~\n\nUna línea en blanco\ncambia de pantalla~~".into(),
            wpm: 130.0,
            font: "DM Sans".into(),
            size: 84.0,
            align: "left".into(),
            background: "#5e0f0b".into(),
            ink: "#ffffff".into(),
            dim: "#d67870".into(),
        }
    }
}

pub struct Texto;

impl VideoTemplate for Texto {
    type Params = Params;
    type Video = TextoVideo;
    const ID: &'static str = "texto";
    const NAME: &'static str = "Texto palabra por palabra";
    const DESCRIPTION: &'static str = "Un texto que aparece a ritmo de lectura, pantalla por pantalla.";

    fn fields() -> Vec<Field> {
        vec![
            Field::textarea("text", "Texto")
                .help("Cada línea es una línea en pantalla; una línea en blanco cambia de pantalla; palabra~ la sostiene más."),
            Field::number("wpm", "Palabras por minuto", 60.0, 300.0, 5.0),
            Field::select("font", "Tipografía", &crate::font_choices()),
            Field::number("size", "Tamaño máximo (px)", 32.0, 200.0, 2.0),
            Field::select("align", "Alineación", &[("left", "Izquierda"), ("center", "Centro")]),
            Field::color("background", "Fondo"),
            Field::color("ink", "Palabra actual"),
            Field::color("dim", "Palabras leídas"),
        ]
    }

    fn build(p: Params) -> TextoVideo {
        let (screens, duration) = timing(&p.text, p.wpm);
        TextoVideo { weight: crate::font_weight(&p.font), p, screens, duration, sizes: OnceLock::new() }
    }
}

#[derive(Debug, Clone)]
struct Word {
    text: String,
    start: f32,
    end: f32,
}

#[derive(Debug, Clone)]
struct Screen {
    lines: Vec<Vec<Word>>,
    /// Texto de cada línea (para medirla).
    texts: Vec<String>,
}

const BASE: f32 = 0.16;
const PER_CHAR: f32 = 0.05;
const HOLD: f32 = 0.5;
const LINE_GAP: f32 = 0.6;
const SCREEN_GAP: f32 = 1.2;
const INTRO: f32 = 0.8;
const OUTRO: f32 = 2.0;

fn letters(w: &str) -> usize {
    w.chars().filter(|c| c.is_alphanumeric()).count()
}

fn pause_after(w: &str) -> f32 {
    match w.chars().last() {
        Some(',') => 0.3,
        Some(';' | ':') => 0.45,
        Some('.' | '!') => 0.6,
        Some('?') => 0.7,
        _ => 0.0,
    }
}

/// Tiempos de cada palabra: dura según su largo (calibrado para dar `wpm` en promedio), la
/// puntuación agrega pausas y `~` sostiene.
fn timing(text: &str, wpm: f32) -> (Vec<Screen>, f32) {
    let all: Vec<String> = text.split_whitespace().map(|w| w.replace('~', "")).filter(|w| !w.is_empty()).collect();
    let mean = all.iter().map(|w| BASE + PER_CHAR * letters(w) as f32).sum::<f32>() / all.len().max(1) as f32;
    let k = 60.0 / wpm.clamp(30.0, 600.0) / mean.max(1e-3);

    let mut screens = Vec::new();
    let mut t = INTRO;
    // pantallas: grupos de líneas separados por líneas en blanco
    let mut blocks: Vec<Vec<&str>> = vec![vec![]];
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            blocks.push(vec![]);
        } else {
            blocks.last_mut().unwrap().push(line);
        }
    }
    for block in blocks.into_iter().filter(|b| !b.is_empty()) {
        let mut lines = Vec::new();
        for line in block {
            let mut words = Vec::new();
            for tok in line.split_whitespace() {
                let holds = tok.matches('~').count() as f32;
                let w = tok.replace('~', "");
                if w.is_empty() {
                    continue;
                }
                let end = t + ((BASE + PER_CHAR * letters(&w) as f32) * k).max(0.2) + holds * HOLD;
                let pause = pause_after(&w);
                words.push(Word { text: w, start: t, end });
                t = end + pause;
            }
            if !words.is_empty() {
                lines.push(words);
                t += LINE_GAP;
            }
        }
        if lines.is_empty() {
            continue;
        }
        t += SCREEN_GAP - LINE_GAP;
        let texts = lines.iter().map(|l: &Vec<Word>| l.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")).collect();
        screens.push(Screen { lines, texts });
    }
    let duration = if screens.is_empty() { INTRO } else { t - SCREEN_GAP } + OUTRO;
    (screens, duration)
}

pub struct TextoVideo {
    p: Params,
    weight: u16,
    screens: Vec<Screen>,
    duration: f32,
    /// Tamaño de letra de cada pantalla (el máximo que entra en el ancho).
    sizes: OnceLock<Vec<f32>>,
}

impl std::fmt::Debug for TextoVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextoVideo").field("screens", &self.screens.len()).finish()
    }
}

impl TextoVideo {
    fn sizes<'a>(&'a self, frame: &mut Frame, ctx: &FFramesContext<'a, '_>) -> Vec<f32> {
        if let Some(s) = self.sizes.get() {
            return s.clone();
        }
        let max_w = W as f32 - 2.0 * MARGIN;
        let mut measured = true;
        let sizes = self
            .screens
            .iter()
            .map(|s| {
                let mut size = self.p.size.clamp(16.0, 300.0);
                for text in &s.texts {
                    let font = FontQuery { family: &self.p.font, size: 100, weight: self.weight, ..Default::default() };
                    match frame.text_width(ctx, font, text) {
                        Some(w100) if w100 > 0 => size = size.min(100.0 * max_w / w100 as f32),
                        Some(_) => {}
                        None => measured = false,
                    }
                }
                size.floor()
            })
            .collect::<Vec<_>>();
        if measured {
            let _ = self.sizes.set(sizes.clone());
        }
        sizes
    }
}

impl Video for TextoVideo {
    const FPS: usize = 30;
    const WIDTH: usize = W;
    const HEIGHT: usize = H;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(self.duration)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let sizes = self.sizes(&mut frame, ctx);
        let p = &self.p;
        let center = p.align == "center";
        let fade_all = 1.0 - ease_in_out(prog(t, self.duration - 1.2, self.duration - 0.2));

        let mut groups: Vec<Svgr<'a>> = Vec::new();
        for (i, s) in self.screens.iter().enumerate() {
            // cada pantalla entra con su primera palabra y sale cuando empieza la siguiente
            let next = self.screens.get(i + 1).map(|n| n.lines[0][0].start);
            let first = s.lines[0][0].start;
            let vis = ease_out(prog(t, first - 0.35, first)) * next.map_or(1.0, |n| 1.0 - ease_in_out(prog(t, n - 0.45, n - 0.1)));
            if vis <= 0.0 {
                continue;
            }
            let size = sizes[i];
            let lead = size * 1.25;
            let top = H as f32 / 2.0 - lead * s.lines.len() as f32 / 2.0 + size * 0.8;
            let mut texts: Vec<Svgr<'a>> = Vec::new();
            for (li, line) in s.lines.iter().enumerate() {
                let line_end = line.last().map_or(0.0, |w| w.end);
                let dim = ease_in_out(prog(t, line_end + 0.5, line_end + 1.1));
                let spans: Vec<Svgr<'a>> = line
                    .iter()
                    .enumerate()
                    .map(|(wi, w)| {
                        let k = ease_out(prog(t, w.start - 0.05, w.start + 0.25));
                        let fill = if dim > 0.5 { p.dim.as_str() } else { p.ink.as_str() };
                        let opacity = k * if dim > 0.0 { 1.0 - 0.25 * (1.0 - (2.0 * dim - 1.0).abs()) } else { 1.0 };
                        let txt = if wi + 1 < line.len() { format!("{} ", w.text) } else { w.text.clone() };
                        fframes::svgr!(<tspan fill={fill} fill-opacity={opacity}>{txt}</tspan>)
                    })
                    .collect();
                let x = if center { W as f32 / 2.0 } else { MARGIN };
                let y = top + li as f32 * lead;
                let anchor = if center { "middle" } else { "start" };
                texts.push(fframes::svgr!(
                    <text x={x} y={y} font-family={p.font.as_str()} font-size={size} font-weight={self.weight} text-anchor={anchor}>
                        {spans}
                    </text>
                ));
            }
            groups.push(fframes::svgr!(<g opacity={vis * fade_all}>{texts}</g>));
        }

        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox={format!("0 0 {W} {H}")} width={W} height={H}>
                <rect width={W} height={H} fill={p.background.as_str()} />
                {groups}
            </svg>
        )
    }
}
