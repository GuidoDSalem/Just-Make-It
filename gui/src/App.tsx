import { useEffect, useMemo, useRef, useState } from 'react';
import { api, type Job, type PreviewRes, type Project, type TemplateInfo, type VideoInfo } from './api';
import { Form } from './Form';

const STORE = 'jmi.project';
const SCALES = [
  { value: 0.5, label: 'Borrador (960×540)' },
  { value: 1, label: '1080p' },
  { value: 2, label: '4K' },
];

function load(): Project | null {
  try {
    return JSON.parse(localStorage.getItem(STORE) ?? 'null');
  } catch {
    return null;
  }
}

export function App() {
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [project, setProject] = useState<Project | null>(null);
  const [info, setInfo] = useState<VideoInfo | null>(null);
  const [t, setT] = useState(1);
  const [preview, setPreview] = useState<PreviewRes | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scale, setScale] = useState(1);
  const [job, setJob] = useState<Job | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  // plantillas y el último proyecto
  useEffect(() => {
    api.templates().then((ts) => {
      setTemplates(ts);
      const saved = load();
      const tpl = ts.find((x) => x.id === saved?.template) ?? ts[0];
      if (tpl) setProject({ template: tpl.id, params: { ...tpl.defaults, ...(saved?.template === tpl.id ? saved.params : {}) } });
    }, (e) => setError(String(e.message ?? e)));
  }, []);

  useEffect(() => {
    if (project) try { localStorage.setItem(STORE, JSON.stringify(project)); } catch { /* sin almacenamiento */ }
  }, [project]);

  const tpl = useMemo(() => templates.find((x) => x.id === project?.template), [templates, project?.template]);

  // duración del video (cambia con los parámetros)
  useEffect(() => {
    if (!project) return;
    const ctl = new AbortController();
    const h = setTimeout(() => api.info(project, ctl.signal).then(setInfo, () => {}), 200);
    return () => { clearTimeout(h); ctl.abort(); };
  }, [project]);

  // vista previa: se pide 250 ms después del último cambio y se cancela la anterior
  useEffect(() => {
    if (!project) return;
    const ctl = new AbortController();
    const at = `${Math.min(t, Math.max(0, (info?.seconds ?? t) - 0.04)).toFixed(3)}s`;
    const h = setTimeout(() => {
      setLoading(true);
      api.preview(project, at, 0.5, ctl.signal).then(
        (p) => { setPreview(p); setError(null); setLoading(false); },
        (e) => { if (!ctl.signal.aborted) { setError(String(e.message ?? e)); setLoading(false); } },
      );
    }, 250);
    return () => { clearTimeout(h); ctl.abort(); };
  }, [project, t, info?.seconds]);

  // seguimiento del render
  useEffect(() => {
    if (!job || job.status !== 'running') return;
    const h = setInterval(() => api.job(job.id).then(setJob, () => {}), 400);
    return () => clearInterval(h);
  }, [job]);

  if (!project || !tpl) return <div className="empty">{error ? `No se pudo conectar con el servidor: ${error}` : 'Cargando…'}</div>;

  const pickTemplate = (id: string) => {
    const next = templates.find((x) => x.id === id)!;
    setProject({ template: id, params: { ...next.defaults } });
    setPreview(null);
    setJob(null);
  };

  const exportJson = () => {
    const blob = new Blob([JSON.stringify(project, null, 2) + '\n'], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `${project.template}.json`;
    a.click();
    URL.revokeObjectURL(a.href);
  };

  const importJson = async (f: File) => {
    try {
      const p = JSON.parse(await f.text()) as Project;
      const next = templates.find((x) => x.id === p.template);
      if (!next) throw new Error(`no existe la plantilla '${p.template}'`);
      setProject({ template: p.template, params: { ...next.defaults, ...p.params } });
    } catch (e) {
      setError(`No se pudo importar: ${(e as Error).message}`);
    }
  };

  const startRender = async () => {
    try {
      const { id } = await api.render(project, scale);
      setJob({ id, status: 'running', progress: 0, frames: 0, total: 0 });
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const duration = info?.seconds ?? 5;
  const problems = preview?.diagnostics.filter((d) => d.severity !== 'info') ?? [];

  return (
    <div className="app">
      <aside className="side">
        <header>
          <h1>Just Make It</h1>
          <select className="tpl" value={project.template} onChange={(e) => pickTemplate(e.target.value)}>
            {templates.map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}
          </select>
          <p className="desc">{tpl.description}</p>
        </header>
        <Form fields={tpl.fields} params={project.params} onChange={(params) => setProject({ ...project, params })} />
        <div className="actions">
          <button onClick={() => setProject({ ...project, params: { ...tpl.defaults } })}>Restablecer</button>
          <button onClick={exportJson}>Exportar JSON</button>
          <button onClick={() => fileInput.current?.click()}>Importar JSON</button>
          <input ref={fileInput} type="file" accept="application/json" hidden onChange={(e) => e.target.files?.[0] && importJson(e.target.files[0])} />
        </div>
      </aside>

      <main className="main">
        <div className="stage">
          {preview ? <img src={`data:image/png;base64,${preview.png}`} alt="vista previa" /> : <div className="placeholder" />}
          {loading && <div className="spinner" />}
        </div>

        <div className="scrub">
          <input type="range" min={0} max={duration} step={1 / 30} value={Math.min(t, duration)} onChange={(e) => setT(Number(e.target.value))} />
          <span className="time">{Math.min(t, duration).toFixed(2)} / {duration.toFixed(2)} s</span>
        </div>

        {info && <p className="meta">{info.width}×{info.height} · {info.fps} fps · {info.frames} frames</p>}
        {error && <p className="error">{error}</p>}
        {problems.length > 0 && (
          <ul className="problems">
            {problems.map((d, i) => <li key={i} className={d.severity}>{d.message}</li>)}
          </ul>
        )}

        <section className="render">
          <select value={scale} onChange={(e) => setScale(Number(e.target.value))}>
            {SCALES.map((s) => <option key={s.value} value={s.value}>{s.label}</option>)}
          </select>
          {job?.status === 'running' ? (
            <>
              <progress value={job.progress} max={1} />
              <span>{job.frames}/{job.total || '…'} frames</span>
              <button onClick={() => api.cancel(job.id)}>Cancelar</button>
            </>
          ) : (
            <button className="primary" onClick={startRender}>Renderizar video</button>
          )}
          {job?.status === 'failed' && <span className="error">Falló: {job.error}</span>}
          {job?.status === 'cancelled' && <span>Cancelado</span>}
        </section>

        {job?.status === 'done' && (
          <section className="result">
            <video src={api.fileUrl(job.id)} controls />
            <a href={api.fileUrl(job.id)} download>Descargar ({job.seconds.toFixed(1)} s de render)</a>
            {job.credits && job.credits.length > 0 && (
              <p className="credits">Música: {job.credits.join(' · ')}<br />Incluí estos créditos al publicar si la licencia pide atribución.</p>
            )}
          </section>
        )}

        <details className="cli">
          <summary>Automatizar este video</summary>
          <p>Exportá el JSON y renderizalo desde la terminal, un script o un agente:</p>
          <pre>jmi render {project.template}.json -o video.mp4</pre>
          <p>O muchos a la vez, uno por fila de un CSV (cada columna reemplaza un parámetro):</p>
          <pre>jmi batch {project.template}.json filas.csv -o videos/</pre>
        </details>
      </main>
    </div>
  );
}
