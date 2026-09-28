# Just Make It

Videos generados por código a partir de **plantillas** y un **JSON**. El mismo JSON sale de la GUI,
de un script, de un CSV o de un agente, y el motor lo convierte en un `.mp4`.

El render usa [fframes](https://github.com/dmtrKovalenko/fframes) (Rust): cada frame es un SVG que
se rasteriza y se codifica con ffmpeg.

```
proyecto.json ──► plantilla (fframes) ──► video.mp4
{ "template": "titulo", "params": { "title": "Hola", ... } }
```

## Qué hay

| | |
|---|---|
| `crates/jmi-core` | El motor: plantillas, render de videos y de frames para la vista previa. |
| `crates/jmi-cli` | `jmi`: la línea de comandos (render, vista previa, lotes desde un CSV). |
| `crates/jmi-server` | `jmi-server`: la API HTTP y el servidor de la GUI. |
| `gui/` | La GUI web (React + TypeScript + Vite). |
| `examples/` | Proyectos y un CSV de ejemplo. |

Plantillas disponibles (`jmi templates`):

- **`promo`**: promo con música. Subís una canción, se detecta el BPM y las frases entran en los
  tiempos fuertes, con todo latiendo al pulso.
- **`titulo`**: placa de título (título, subtítulo, colores, tipografía, duración).
- **`texto`**: texto palabra por palabra a ritmo de lectura, pantalla por pantalla.

## Música

Cada canción se sube **con su licencia** (es obligatoria): queda guardada junto al archivo y cada
video que la usa sale con un `<video>.mp4.creditos.txt` al lado. Al subirla se detecta el tempo
(BPM, primer tiempo fuerte); si la detección falla, el BPM se puede fijar a mano en la plantilla.

De dónde sacar música que se puede usar (revisá siempre los términos vigentes de cada sitio):

- Gratis: Pixabay Music (uso comercial, sin atribución), YouTube Audio Library, Incompetech (CC BY:
  hay que nombrar al autor), Free Music Archive y ccMixter (licencia por canción).
- Por suscripción: Epidemic Sound, Artlist, Uppbeat.
- Generada con IA (Suno, Udio): en general sólo los planes pagos dan uso comercial.
- Evitá canciones bajadas de Spotify o YouTube: las plataformas las detectan y bajan o silencian el video.

`examples/musica/demo-118bpm.mp3` es una canción de prueba hecha por código
(`scripts/demo_song.py`), de dominio público (CC0):

```sh
jmi media add examples/musica/demo-118bpm.mp3 --license CC0 --source scripts/demo_song.py
jmi render examples/promo.json -o promo.mp4
```

Las canciones se guardan en `media/` (o en `JMI_MEDIA_DIR`), una carpeta por canción.

## Requisitos

- [Rust](https://rustup.rs) (1.85 o más nuevo).
- Las librerías con las que se compila ffmpeg:
  - macOS: `brew install pkg-config ffmpeg x264 x265 opus nasm ninja`
  - Debian/Ubuntu: `sudo apt-get install -y yasm nasm ffmpeg libx264-dev libx265-dev libopus-dev libclang-dev clang ninja-build libvpx-dev libasound2-dev`
- [Node.js](https://nodejs.org) 20+ para la GUI.

La primera compilación compila ffmpeg desde el código fuente y tarda unos 10 minutos; las siguientes,
segundos.

## Uso

### GUI

```sh
cargo build --release
(cd gui && npm install && npm run build)
./target/release/jmi-server          # http://127.0.0.1:8787
```

Elegís una plantilla, completás el formulario, mirás la vista previa (con la barra de tiempo) y
renderizás. "Exportar JSON" baja el proyecto para automatizarlo.

Para desarrollar la GUI con recarga en vivo: `jmi-server` en una terminal y `cd gui && npm run dev`
en otra (Vite le pasa `/api` al servidor).

### Docker

Sin instalar Rust ni Node: la imagen trae la GUI, la API y la CLI.

```sh
docker compose up --build            # http://localhost:8787; videos en ./out, canciones en ./media
```

O a mano:

```sh
docker build -t just-make-it .
docker run --rm -p 8787:8787 -v "$PWD/out:/data/out" -v "$PWD/media:/data/media" just-make-it
docker run --rm --user "$(id -u)" -v "$PWD:/work" -w /work just-make-it jmi render examples/titulo.json -o out/titulo.mp4
```

La primera construcción compila ffmpeg (~10 min); las siguientes reusan el caché de cargo. La imagen
final pesa ~230 MB y renderiza en CPU (el render en GPU necesita otra imagen, con drivers).

### Línea de comandos

```sh
J=./target/release/jmi
$J templates                                   # plantillas
$J schema texto                                # campos y valores por defecto (JSON)
$J new titulo -o proyecto.json                 # proyecto nuevo
$J frame proyecto.json --at 2s -o frame.png    # un frame (y los problemas que encuentre)
$J render proyecto.json -o video.mp4           # el video (--scale 0.5 borrador, 2 = 4K)
$J batch examples/titulo.json examples/charlas.csv -o videos/ --name title
$J media add cancion.mp3 --license "Pixabay Content License" --source https://…   # → id
$J media list                                  # canciones, BPM y licencias
```

`batch` hace un video por fila: cada columna del CSV reemplaza un parámetro del proyecto (también
acepta un JSON con una lista de objetos).

### API

`jmi-server` expone lo mismo por HTTP (ver `docs/ARCHITECTURE.md`):

```sh
curl -X POST localhost:8787/api/render -H 'content-type: application/json' \
  -d '{"template":"titulo","params":{"title":"Hola"}}'       # → {"id":1}
curl localhost:8787/api/jobs/1                               # progreso
curl -o video.mp4 localhost:8787/api/jobs/1/file
```

## Estado y próximos pasos

- [x] Motor con plantillas que leen un JSON, CLI, lotes, API y GUI web.
- [x] Música con licencia, detección de tempo y plantilla `promo` en el ritmo.
- [ ] Más plantillas (vertical para redes, audio con subtítulos, explicador con datos).
- [ ] Render en GPU (backend Skia de fframes: Metal en Mac, Vulkan en Linux/Windows).
- [x] Contenedor (Docker) con la GUI, la API y la CLI.
- [ ] App de escritorio (Tauri) con la misma GUI.
- [ ] Vista previa en el navegador con WebAssembly.

## Créditos

- [fframes](https://github.com/dmtrKovalenko/fframes), MIT, de Dmitriy Kovalenko.
- Tipografías bajo SIL Open Font License: ver `crates/jmi-core/FONTS.md`.
