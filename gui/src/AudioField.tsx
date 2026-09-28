// Campo de canción: elegir una subida o subir una nueva con su licencia. Muestra el BPM detectado.
import { useEffect, useState } from 'react';
import { api, type License, type MediaInfo } from './api';

const LICENSES = [
  'Pixabay Content License',
  'YouTube Audio Library',
  'CC0',
  'CC BY 4.0',
  'Epidemic Sound (suscripción)',
  'Artlist (suscripción)',
  'Uppbeat',
  'Propia',
];

export function AudioField({ value, onChange }: { value: string; onChange: (id: string) => void }) {
  const [songs, setSongs] = useState<MediaInfo[]>([]);
  const [adding, setAdding] = useState(false);
  const [file, setFile] = useState<File | null>(null);
  const [lic, setLic] = useState<License>({ license: '', source: '', author: '' });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => api.media().then(setSongs, () => {});
  useEffect(() => { refresh(); }, []);

  const current = songs.find((s) => s.id === value);

  const upload = async () => {
    if (!file) return;
    setBusy(true);
    setError(null);
    try {
      const info = await api.upload(file, lic);
      await refresh();
      onChange(info.id);
      setAdding(false);
      setFile(null);
      setLic({ license: '', source: '', author: '' });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="audio">
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">Sin música (pulso de 120 BPM)</option>
        {songs.map((s) => (
          <option key={s.id} value={s.id}>
            {s.name}{s.tempo ? ` · ${s.tempo.bpm.toFixed(0)} BPM` : ''}
          </option>
        ))}
      </select>

      {current && (
        <div className="song">
          <audio controls preload="none" src={api.mediaUrl(current.id)} />
          {current.tempo && (
            <span className={current.tempo.confidence < 0.4 ? 'warn' : 'muted'}>
              {current.tempo.bpm.toFixed(1)} BPM · primer tiempo en {current.tempo.first_downbeat.toFixed(2)} s
              {current.tempo.confidence < 0.4 && ' · pulso poco claro: revisá el BPM a mano'}
            </span>
          )}
          <span className="muted">Licencia: {current.license}{current.author && ` · ${current.author}`}</span>
        </div>
      )}

      {!adding ? (
        <button type="button" onClick={() => setAdding(true)}>Subir canción…</button>
      ) : (
        <div className="upload">
          <input type="file" accept="audio/*,.mp3,.wav,.flac,.ogg" onChange={(e) => setFile(e.target.files?.[0] ?? null)} />
          <label>
            <span className="label">Licencia (obligatoria)</span>
            <input list="jmi-licenses" value={lic.license} placeholder="p. ej. Pixabay Content License" onChange={(e) => setLic({ ...lic, license: e.target.value })} />
            <datalist id="jmi-licenses">{LICENSES.map((l) => <option key={l} value={l} />)}</datalist>
          </label>
          <label>
            <span className="label">Fuente (link o comprobante)</span>
            <input value={lic.source} placeholder="https://…" onChange={(e) => setLic({ ...lic, source: e.target.value })} />
          </label>
          <label>
            <span className="label">Autor (si la licencia pide atribución)</span>
            <input value={lic.author} onChange={(e) => setLic({ ...lic, author: e.target.value })} />
          </label>
          <span className="help">Usá música con licencia para tu uso: bibliotecas libres, una suscripción o tu propia música. La licencia queda en los créditos de cada video.</span>
          {error && <span className="error">{error}</span>}
          <div className="row">
            <button type="button" className="primary" disabled={!file || !lic.license.trim() || busy} onClick={upload}>
              {busy ? 'Analizando…' : 'Subir y detectar BPM'}
            </button>
            <button type="button" onClick={() => setAdding(false)}>Cancelar</button>
          </div>
        </div>
      )}
    </div>
  );
}
