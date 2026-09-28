//! Placa de título: un título y un subtítulo que entran, se sostienen y salen.
use super::{VideoTemplate, ease_in_out, ease_out, prog};
use crate::fields::Field;
use fframes::{AudioMap, Color, Duration, FFramesContext, FontQuery, Frame, Svgr, Transform, Video};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const W: usize = 1920;
const H: usize = 1080;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    pub title: String,
    pub subtitle: String,
    pub font: String,
    pub background: String,
    pub ink: String,
    pub accent: String,
    /// Segundos.
    pub duration: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            title: "Just Make It".into(),
            subtitle: "videos desde un JSON".into(),
            font: "DM Sans".into(),
            background: "#0f172a".into(),
            ink: "#f8fafc".into(),
            accent: "#f97316".into(),
            duration: 5.0,
        }
    }
}

pub struct Titulo;

impl VideoTemplate for Titulo {
    type Params = Params;
    type Video = TituloVideo;
    const ID: &'static str = "titulo";
    const NAME: &'static str = "Placa de título";
    const DESCRIPTION: &'static str = "Un título y un subtítulo que entran con un resorte, se sostienen y salen.";

    fn fields() -> Vec<Field> {
        vec![
            Field::text("title", "Título"),
            Field::text("subtitle", "Subtítulo"),
            Field::select("font", "Tipografía", &crate::font_choices()),
            Field::color("background", "Fondo"),
            Field::color("ink", "Texto"),
            Field::color("accent", "Acento"),
            Field::number("duration", "Duración (s)", 2.0, 30.0, 0.5),
        ]
    }

    fn build(p: Params) -> TituloVideo {
        TituloVideo { weight: crate::font_weight(&p.font), p, size: OnceLock::new() }
    }
}

pub struct TituloVideo {
    p: Params,
    weight: u16,
    /// Tamaño del título, medido una vez para que entre en el ancho.
    size: OnceLock<usize>,
}

impl std::fmt::Debug for TituloVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TituloVideo").field("title", &self.p.title).finish()
    }
}

impl TituloVideo {
    fn title_size<'a>(&'a self, frame: &mut Frame, ctx: &FFramesContext<'a, '_>) -> usize {
        if let Some(s) = self.size.get() {
            return *s;
        }
        let mut size = 150;
        loop {
            let font = FontQuery { family: &self.p.font, size, weight: self.weight, ..Default::default() };
            match frame.text_width(ctx, font, &self.p.title) {
                Some(w) if w as f32 > W as f32 * 0.84 && size > 24 => size -= 4,
                Some(_) => return *self.size.get_or_init(|| size),
                None => return size,
            }
        }
    }
}

impl Video for TituloVideo {
    const FPS: usize = 30;
    const WIDTH: usize = W;
    const HEIGHT: usize = H;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(self.p.duration.max(1.0))
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let end = self.p.duration.max(1.0);
        let size = self.title_size(&mut frame, ctx) as f32;

        // entrada: sube con un rebote corto; salida: se desvanece en el último medio segundo
        let k = prog(t, 0.15, 0.85);
        let rise = 90.0 * (1.0 - k).powi(3) - 12.0 * (k * std::f32::consts::PI).sin() * (1.0 - k);
        let fade = ease_out(prog(t, 0.15, 0.55)) * (1.0 - ease_in_out(prog(t, end - 0.6, end - 0.1)));
        let sub = ease_out(prog(t, 0.7, 1.2));
        let line = 0.5 + ease_in_out(prog(t, 0.5, 1.2)) * 320.0;

        let (cx, cy) = (W as f32 / 2.0, H as f32 / 2.0 + size * 0.2);
        let p = &self.p;
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox={format!("0 0 {W} {H}")} width={W} height={H}>
                <rect width={W} height={H} fill={p.background.as_str()} />
                <g opacity={fade} transform={Transform::translate(0, rise)}>
                    <text x={cx} y={cy} font-family={p.font.as_str()} font-size={size} font-weight={self.weight} fill={p.ink.as_str()} text-anchor="middle">
                        {p.title.as_str()}
                    </text>
                    <rect x={cx - line / 2.0} y={cy + size * 0.3} width={line} height="10" rx="5" fill={p.accent.as_str()} />
                    <text x={cx} y={cy + size * 0.3 + 90.0} font-family={p.font.as_str()} font-size="46" font-weight={self.weight} fill={p.ink.as_str()} fill-opacity={0.7 * sub} text-anchor="middle">
                        {p.subtitle.as_str()}
                    </text>
                </g>
            </svg>
        )
    }
}
