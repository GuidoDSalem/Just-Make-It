# Arquitectura

## La idea

Una plantilla de video es **código** (Rust, compilado); lo que cambia entre un video y otro son
**datos** (un JSON). La GUI, la CLI y la API sólo editan y mandan ese JSON: cambiar un texto o un
color es instantáneo, y sólo cambiar el diseño de una plantilla requiere recompilar.

```
             ┌────────── GUI (React) ──────────┐
             │ formulario ← fields de la plantilla
             │ vista previa ← /api/preview       │
             └───────────────┬──────────────────┘
                             │ HTTP (JSON)
 script / agente ──► jmi-server (axum) ──┐
 CSV / terminal ──► jmi (CLI) ───────────┤
                                         ▼
                                  jmi-core
                     Template (JSON) ──► VideoTemplate (tipado)
                                         │ build(params) → fframes::Video
                                         ▼
                             fframes: SVG → píxeles → ffmpeg
```

## jmi-core

- `templates/mod.rs`: dos traits.
  - `VideoTemplate`: se implementa en cada plantilla. Declara una struct de parámetros (`serde`,
    con `#[serde(default)]`), los campos del formulario (`fields()`) y cómo construir el
    `fframes::Video` (`build`).
  - `Template`: la misma plantilla sin tipos, con parámetros en JSON. Es lo que usan la CLI y el
    servidor. Se implementa sola para todo `VideoTemplate`.
- `fields.rs`: tipos de campo (texto, número, color, opciones). La GUI arma el formulario con esto.
- `render.rs`: render genérico de un video (`render_video`, con progreso y cancelación) y de un
  frame a PNG con los problemas que encuentre fframes (`render_preview`).
- `project.rs`: el formato de proyecto `{ "template", "params" }`.
- `media_store.rs`: canciones subidas. Una carpeta por canción (`<raíz>/<id>/`, el id sale del
  contenido) con el archivo y un `meta.json` con nombre, licencia, fuente, autor y tempo. Sin
  licencia no se guarda.
- `beats.rs`: detección de tempo (tempo constante). Envolvente de ataques sin graves →
  autocorrelación con preferencia por ~120 BPM → pico fraccionario → fase y período fino por
  búsqueda en grilla → el "1" del compás es la posición con más graves. Probado con loops sintéticos
  de 96 a 140 BPM.
- `media/`: tipografías embebidas en el binario (`include_media_dir!`). En esa carpeta sólo puede
  haber medios.

### Agregar una plantilla

1. `crates/jmi-core/src/templates/<id>.rs` con `Params` (+ `Default` con un ejemplo completo), el
   video (`impl fframes::Video`) y `impl VideoTemplate`. Si usa una canción, `media()` devuelve su
   ficha: el motor carga esa carpeta como medios de fframes y escribe los créditos al renderizar.
2. Registrarla en `templates()` de `templates/mod.rs`.
3. `cargo test -p jmi-core --release`: los tests genéricos renderizan todas las plantillas con sus
   valores por defecto y fallan si hay texto cortado, tipografías faltantes o pánicos.

La GUI y la CLI la toman solas.

## jmi-server

| Método | Ruta | |
|---|---|---|
| GET | `/api/templates` | plantillas: `id`, `name`, `description`, `fields`, `defaults` |
| POST | `/api/info` | `{template, params}` → `{width, height, fps, frames, seconds}` |
| POST | `/api/preview` | `{template, params, at, scale}` → `{png (base64), frame, seconds, diagnostics}` |
| POST | `/api/render` | `{template, params, scale}` → `{id}` |
| GET | `/api/jobs/{id}` | `{status: running/done/failed/cancelled, progress, frames, total}` |
| POST | `/api/jobs/{id}/cancel` | cancela |
| GET | `/api/jobs/{id}/file` | el `.mp4` (el estado del trabajo trae `credits`) |
| GET | `/api/media` | canciones subidas, con licencia y tempo |
| POST | `/api/media?name=&license=&source=&author=` | sube una canción (el cuerpo es el archivo, hasta 200 MB) |
| GET | `/api/media/{id}/file` | el audio |
| POST | `/api/media/{id}/license` | `{license, source, author}` |

Todo lo demás sirve la GUI compilada (`gui/dist`). Opciones: `--addr`, `--gui`, `--out`, `--media`
(o `JMI_ADDR`, `JMI_GUI_DIR`, `JMI_OUT_DIR`, `JMI_MEDIA_DIR`).

Los renders corren en hilos aparte (`spawn_blocking`); los trabajos viven en memoria (se pierden al
reiniciar el servidor).

## Hacia "cualquier entorno"

La GUI es web y el motor es una librería, así que el mismo código se empaqueta de varias formas:

- **Navegador + servidor** (hoy): `jmi-server` local o en la nube.
- **Escritorio** (Tauri): la GUI dentro de una app nativa que llama a `jmi-core` directo y puede
  usar la GPU de la máquina.
- **Contenedor** (Docker, hecho): `Dockerfile` en tres etapas (GUI con Node, motor con Rust sobre
  Ubuntu 24.04, imagen final con los binarios, la GUI y `libx264`). Sirve para renders automáticos en
  la nube.
- **Vista previa en el navegador**: fframes compila a WebAssembly (su editor ya lo hace).
