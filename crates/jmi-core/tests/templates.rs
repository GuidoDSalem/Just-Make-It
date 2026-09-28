//! Toda plantilla, con sus parámetros por defecto, tiene que renderizar sin problemas: sin
//! tipografías faltantes, sin texto cortado y sin pánicos, al principio, en el medio y al final.
use serde_json::json;

#[test]
fn every_template_renders_cleanly() {
    for t in jmi_core::templates() {
        let info = t.info();
        assert!(!info.fields.is_empty(), "{}: sin campos", info.id);
        for f in &info.fields {
            assert!(info.defaults.get(f.key).is_some(), "{}: el campo '{}' no tiene valor por defecto", info.id, f.key);
        }
        let v = t.video_info(&info.defaults).unwrap();
        assert!(v.seconds > 0.5, "{}: dura {}s", info.id, v.seconds);
        for at in ["0", "25%", "50%", "75%", "end"] {
            let p = t.preview(&info.defaults, at, 0.25).unwrap_or_else(|e| panic!("{} @ {at}: {e:#}", info.id));
            assert!(!p.png.is_empty());
            let bad: Vec<_> = p.diagnostics.iter().filter(|d| d.severity != "info").collect();
            assert!(bad.is_empty(), "{} @ {at}: {bad:?}", info.id);
        }
    }
}

#[test]
fn missing_params_fall_back_to_defaults() {
    for t in jmi_core::templates() {
        t.video_info(&json!({})).unwrap_or_else(|e| panic!("{}: {e:#}", t.info().id));
    }
}

#[test]
fn texto_long_lines_shrink_to_fit() {
    let t = jmi_core::template("texto").unwrap();
    let params = json!({ "text": "una línea larguísima que de ninguna manera entra en el ancho de la pantalla a este tamaño", "size": 200 });
    let p = t.preview(&params, "end", 0.25).unwrap();
    let bad: Vec<_> = p.diagnostics.iter().filter(|d| d.severity != "info").collect();
    assert!(bad.is_empty(), "{bad:?}");
}

/// WAV mono de 16 bits.
fn wav(samples: &[f32], sr: u32) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes()).collect();
    let mut v = Vec::new();
    v.extend(b"RIFF");
    v.extend((36 + data.len() as u32).to_le_bytes());
    v.extend(b"WAVEfmt ");
    v.extend(16u32.to_le_bytes());
    v.extend(1u16.to_le_bytes());
    v.extend(1u16.to_le_bytes());
    v.extend(sr.to_le_bytes());
    v.extend((sr * 2).to_le_bytes());
    v.extend(2u16.to_le_bytes());
    v.extend(16u16.to_le_bytes());
    v.extend(b"data");
    v.extend((data.len() as u32).to_le_bytes());
    v.extend(data);
    v
}

/// Bombo en cada tiempo (más fuerte en el 1), a `bpm`, empezando en `offset`.
fn kicks(bpm: f32, seconds: f32, offset: f32, sr: u32) -> Vec<f32> {
    let n = (seconds * sr as f32) as usize;
    let mut x = vec![0.0f32; n];
    let beat = 60.0 / bpm;
    let mut k = 0;
    while offset + k as f32 * beat < seconds {
        let i0 = ((offset + k as f32 * beat) * sr as f32) as usize;
        let amp = if k % 4 == 0 { 0.9 } else { 0.5 };
        for j in 0..(sr as usize / 4).min(n - i0) {
            let t = j as f32 / sr as f32;
            x[i0 + j] += amp * (2.0 * std::f32::consts::PI * (55.0 + 90.0 * (-t * 35.0).exp()) * t).sin() * (-t * 10.0).exp();
        }
        k += 1;
    }
    x
}

#[test]
fn promo_with_an_uploaded_song() {
    let dir = std::env::temp_dir().join(format!("jmi-media-test-{}", std::process::id()));
    jmi_core::media_store::set_root(&dir);
    let sr = 44100;
    let song = wav(&kicks(124.0, 20.0, 0.4, sr), sr);

    // sin licencia no se sube
    let no_license = jmi_core::media_store::add_audio("loop.wav", &song, Default::default());
    assert!(no_license.is_err());

    let license = jmi_core::media_store::License { license: "Propia".into(), source: String::new(), author: "Test".into() };
    let info = jmi_core::media_store::add_audio("Mi Loop.wav", &song, license).unwrap();
    let tempo = info.tempo.clone().unwrap();
    assert!((tempo.bpm - 124.0).abs() < 0.5, "bpm {}", tempo.bpm);
    assert_eq!(jmi_core::media_store::list().len(), 1);

    let t = jmi_core::template("promo").unwrap();
    let params = json!({ "audio": info.id, "phrases": "uno\ndos\ntres", "beats_per_phrase": "4", "start_bar": 1 });
    let v = t.video_info(&params).unwrap();
    // compás 1 + 3 frases de 4 tiempos + 1 compás de cola = 20 tiempos desde el primer tiempo fuerte
    let expected = tempo.first_downbeat + 20.0 * tempo.period;
    assert!((v.seconds - expected).abs() < 0.1, "dura {} (esperado {expected})", v.seconds);
    for at in ["0", "50%", "end"] {
        let p = t.preview(&params, at, 0.25).unwrap();
        let bad: Vec<_> = p.diagnostics.iter().filter(|d| d.severity != "info").collect();
        assert!(bad.is_empty(), "@ {at}: {bad:?}");
    }

    // el video sale con audio y con sus créditos al lado
    let out = dir.join("promo.mp4");
    let mut job = jmi_core::RenderJob::new(&out);
    job.scale = 0.25;
    t.render(&params, &job).unwrap();
    assert!(std::fs::metadata(&out).unwrap().len() > 1000);
    let credits = std::fs::read_to_string(dir.join("promo.mp4.creditos.txt")).unwrap();
    assert!(credits.contains("Mi Loop.wav — Test · Propia"), "{credits}");
    let _ = std::fs::remove_dir_all(&dir);
}
