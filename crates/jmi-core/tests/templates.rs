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
