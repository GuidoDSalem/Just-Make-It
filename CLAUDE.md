# Just Make It

Videos desde plantillas + JSON. Motor en Rust sobre fframes; GUI en React + TypeScript. Textos de la
interfaz y documentación en español rioplatense.

- Antes de commitear: `cargo build --release`, `cargo test --release -p jmi-core` y
  `cd gui && npm run build` tienen que pasar. Usar siempre `--release`: fframes en debug es muy lento.
- La primera compilación compila ffmpeg (~10 min). Hacen falta las librerías de sistema del README.
- Una plantilla = `VideoTemplate` en `crates/jmi-core/src/templates/<id>.rs`, registrada en
  `templates()`. Sus parámetros llevan `#[serde(default)]` y un `Default` completo: los tests
  genéricos renderizan cada plantilla con esos valores y fallan ante texto cortado, tipografías
  faltantes o pánicos.
- `render_frame` corre en varios hilos: nada de pánicos, lecturas de archivos ni trabajo pesado
  adentro; preparar datos en `build` o en un `OnceLock`.
- Tipografías en `crates/jmi-core/media/` (sólo medios ahí; licencias en `FONTS.md`). Usar la
  familia y el peso que existen (`FONTS` en `lib.rs`).
- Para mirar resultados sin GPU: `jmi frame proyecto.json --at 50% -o f.png --scale 0.5` y abrir el
  PNG. La GUI se prueba con `jmi-server` + Playwright (Chromium en `/opt/pw-browsers` en la nube).
- La API y los tipos de la GUI (`gui/src/api.ts`) reflejan `jmi-core` (`fields.rs`, `render.rs`):
  cambiarlos juntos.
- Guía de fframes para agentes: `npx skills add dmtrKovalenko/fframes`, o
  <https://github.com/dmtrKovalenko/fframes/tree/main/skills/fframes-video>.
