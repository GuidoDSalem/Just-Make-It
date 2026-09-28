//! Detección de tempo y beats de una canción (tempo constante, que es lo habitual en música
//! producida con un secuenciador).
//!
//! 1. Envolvente de ataques: aumento de energía (en escala logarítmica) de toda la señal y de los
//!    agudos cada ~23 ms, y aparte la de los graves (bombo).
//! 2. Tempo: autocorrelación de la envolvente entre 70 y 180 BPM, con preferencia suave por ~120.
//! 3. Fase: la grilla de beats que más ataques junta; después se ajusta el período fino.
//! 4. Compás: de las 4 posiciones posibles del primer tiempo, la que tiene más graves.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tempo {
    pub bpm: f32,
    /// Segundos entre beats.
    pub period: f32,
    /// Primer beat (s).
    pub first_beat: f32,
    /// Primer tiempo fuerte (s): el "1" del compás.
    pub first_downbeat: f32,
    pub duration: f32,
    /// 0..1: qué tan marcado es el pulso (bajo = conviene revisar el BPM a mano).
    pub confidence: f32,
}

impl Tempo {
    /// Un pulso fijo para cuando no hay canción.
    pub fn fixed(bpm: f32, duration: f32) -> Self {
        let period = 60.0 / bpm;
        Self { bpm, period, first_beat: 0.0, first_downbeat: 0.0, duration, confidence: 1.0 }
    }

    /// Tiempo (s) del beat `i` contado desde el primer tiempo fuerte (puede ser fraccionario o negativo).
    pub fn beat_time(&self, i: f32) -> f32 {
        self.first_downbeat + i * self.period
    }

    /// Beat (continuo) en el tiempo `t`, contado desde el primer tiempo fuerte.
    pub fn beat_at(&self, t: f32) -> f32 {
        (t - self.first_downbeat) / self.period
    }
}

const HOP: usize = 512;

/// Analiza una señal mono.
pub fn analyze(samples: &[f32], sample_rate: u32) -> Tempo {
    let sr = sample_rate as f32;
    let duration = samples.len() as f32 / sr;
    let fps = sr / HOP as f32;
    // tempo y fase con la envolvente sin graves (bombo y caja pesan igual: evita confundir el
    // tempo con la mitad); los graves sólo deciden dónde está el "1" del compás
    let (onset, low) = onset_envelope(samples, sr);
    if onset.len() < (fps * 4.0) as usize {
        return Tempo { confidence: 0.0, ..Tempo::fixed(120.0, duration) };
    }

    // tempo: autocorrelación con preferencia log-normal centrada en 120 BPM
    let lag_of = |bpm: f32| 60.0 * fps / bpm;
    let (min_lag, max_lag) = (lag_of(180.0).floor() as usize, lag_of(70.0).ceil() as usize);
    // los picos duran un frame y el período casi nunca es un número entero de frames: se suaviza
    // la envolvente para que la autocorrelación en retardos enteros no castigue a los tempos rápidos
    let smooth: Vec<f32> = (0..onset.len())
        .map(|i| {
            let k = [0.15, 0.5, 1.0, 0.5, 0.15];
            (0..5).map(|j| k[j] * onset.get((i + j).wrapping_sub(2)).copied().unwrap_or(0.0)).sum()
        })
        .collect();
    let ac = |lag: usize| -> f32 { smooth.iter().zip(&smooth[lag..]).map(|(a, b)| a * b).sum::<f32>() / (smooth.len() - lag) as f32 };
    let ac0 = ac(0).max(1e-9);
    let mut best = (0.0f32, min_lag);
    for lag in min_lag..=max_lag.min(onset.len() / 2) {
        let bpm = 60.0 * fps / lag as f32;
        let prior = (-0.5 * ((bpm / 120.0).log2() / 0.9).powi(2)).exp();
        let score = ac(lag) * prior;
        if score > best.0 {
            best = (score, lag);
        }
    }
    let confidence = (ac(best.1) / ac0).clamp(0.0, 1.0);
    // pico fraccionario (interpolación parabólica alrededor del mejor retardo entero)
    let l = best.1;
    let (a, b, c) = (ac(l - 1), ac(l), ac(l + 1));
    let den = a - 2.0 * b + c;
    let mut period = l as f32 + if den.abs() > 1e-12 { (0.5 * (a - c) / den).clamp(-0.5, 0.5) } else { 0.0 };

    // fase y período fino: la grilla que más ataques junta
    let grid_score = |env: &[f32], phase: f32, period: f32| -> f32 {
        let mut s = 0.0;
        let mut x = phase;
        while (x as usize) + 1 < env.len() {
            let i = x as usize;
            let f = x - i as f32;
            s += env[i] * (1.0 - f) + env[i + 1] * f;
            x += period;
        }
        s
    };
    let mut phase = 0.0;
    for _ in 0..2 {
        let mut b = (f32::MIN, 0.0, period);
        for k in -30..=30 {
            let p = period * (1.0 + k as f32 * 0.0005);
            let mut ph = 0.0;
            while ph < p {
                let s = grid_score(&onset, ph, p);
                if s > b.0 {
                    b = (s, ph, p);
                }
                ph += 0.5;
            }
        }
        phase = b.1;
        period = b.2;
    }

    // compás: el primer tiempo es el de más graves de los 4
    let bar = (0..4)
        .max_by(|&a, &b| {
            let sa = grid_score(&low, phase + a as f32 * period, period * 4.0);
            let sb = grid_score(&low, phase + b as f32 * period, period * 4.0);
            sa.total_cmp(&sb)
        })
        .unwrap_or(0);

    let hop_s = HOP as f32 / sr;
    let bpm = 60.0 / (period * hop_s);
    Tempo {
        bpm: (bpm * 100.0).round() / 100.0,
        period: period * hop_s,
        first_beat: phase * hop_s,
        first_downbeat: (phase + bar as f32 * period) * hop_s,
        duration,
        confidence,
    }
}

/// (envolvente de ataques de toda la señal, envolvente de ataques de los graves), una muestra por HOP.
fn onset_envelope(x: &[f32], sr: f32) -> (Vec<f32>, Vec<f32>) {
    // filtros de un polo: graves < 150 Hz, agudos > 3 kHz
    let a_low = (-2.0 * std::f32::consts::PI * 150.0 / sr).exp();
    let a_high = (-2.0 * std::f32::consts::PI * 3000.0 / sr).exp();
    let (mut lp, mut lp_h) = (0.0f32, 0.0f32);
    let n = x.len() / HOP;
    let mut e = vec![[0.0f32; 3]; n];
    for (i, frame) in x.chunks_exact(HOP).enumerate() {
        let mut acc = [0.0f32; 3];
        for &s in frame {
            lp = (1.0 - a_low) * s + a_low * lp;
            lp_h = (1.0 - a_high) * s + a_high * lp_h;
            let high = s - lp_h;
            acc[0] += lp * lp;
            acc[1] += s * s;
            acc[2] += high * high;
        }
        e[i] = acc;
    }
    let flux = |band: usize, w: f32| -> Vec<f32> {
        let mut v = vec![0.0f32; n];
        for i in 1..n {
            let d = (e[i][band] + 1e-6).ln() - (e[i - 1][band] + 1e-6).ln();
            v[i] = w * d.max(0.0);
        }
        v
    };
    let (fl, fa, fh) = (flux(0, 1.0), flux(1, 1.0), flux(2, 1.0));
    let all: Vec<f32> = (0..n).map(|i| fa[i] + fh[i]).collect();
    (normalize(&all), normalize(&fl))
}

/// Resta la media local (~1 s) y recorta a positivos: deja sólo los picos.
fn normalize(v: &[f32]) -> Vec<f32> {
    let w = 43;
    let mut out = vec![0.0; v.len()];
    let mut sum = 0.0;
    for i in 0..v.len() {
        sum += v[i];
        if i >= w {
            sum -= v[i - w];
        }
        let mean = sum / (i.min(w - 1) + 1) as f32;
        out[i] = (v[i] - mean).max(0.0);
    }
    let max = out.iter().cloned().fold(0.0, f32::max).max(1e-9);
    out.iter().map(|x| x / max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pista sintética: bombo en cada tiempo 1 y 3, caja en 2 y 4, hi-hat en corcheas.
    pub fn drums(bpm: f32, seconds: f32, offset: f32, sr: u32) -> Vec<f32> {
        let n = (seconds * sr as f32) as usize;
        let mut x = vec![0.0f32; n];
        let beat = 60.0 / bpm;
        let mut k = 0;
        loop {
            let t0 = offset + k as f32 * beat / 2.0;
            let i0 = (t0 * sr as f32) as usize;
            if i0 >= n {
                break;
            }
            let on_beat = k % 2 == 0;
            let beat_in_bar = (k / 2) % 4;
            for j in 0..(sr as usize / 5).min(n - i0) {
                let t = j as f32 / sr as f32;
                let mut s = 0.0;
                if on_beat && (beat_in_bar == 0 || beat_in_bar == 2) {
                    s += (2.0 * std::f32::consts::PI * (60.0 + 80.0 * (-t * 30.0).exp()) * t).sin() * (-t * 12.0).exp();
                }
                if on_beat && (beat_in_bar == 1 || beat_in_bar == 3) {
                    s += 0.4 * (((j * 7919) % 1000) as f32 / 500.0 - 1.0) * (-t * 25.0).exp();
                }
                s += 0.15 * (((j * 104729) % 1000) as f32 / 500.0 - 1.0) * (-t * 80.0).exp();
                x[i0 + j] += s;
            }
            k += 1;
        }
        x
    }

    #[test]
    fn finds_tempo_and_downbeat_of_a_drum_loop() {
        for (bpm, offset) in [(128.0, 0.31), (96.0, 0.05), (140.0, 0.6), (110.0, 1.2)] {
            let sr = 22050;
            let x = drums(bpm, 30.0, offset, sr);
            let t = analyze(&x, sr);
            assert!((t.bpm - bpm).abs() < 0.5, "bpm {bpm}: detectado {}", t.bpm);
            // el primer tiempo fuerte cae sobre un bombo del tiempo 1 (módulo un compás)
            let bar = 4.0 * 60.0 / bpm;
            let d = ((t.first_downbeat - offset) / bar).rem_euclid(1.0);
            let err = d.min(1.0 - d) * bar;
            assert!(err < 0.03, "bpm {bpm}: primer tiempo en {} (esperado {offset} + n compases)", t.first_downbeat);
        }
    }
}
