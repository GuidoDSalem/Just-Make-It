// Formulario armado a partir de los campos que declara la plantilla.
import type { Field, Params } from './api';

export function Form({ fields, params, onChange }: { fields: Field[]; params: Params; onChange: (p: Params) => void }) {
  const set = (key: string, value: unknown) => onChange({ ...params, [key]: value });
  return (
    <div className="form">
      {fields.map((f) => (
        <label key={f.key} className={`field field-${f.type}`}>
          <span className="label">{f.label}</span>
          <Input field={f} value={params[f.key]} onChange={(v) => set(f.key, v)} />
          {f.help && <span className="help">{f.help}</span>}
        </label>
      ))}
    </div>
  );
}

function Input({ field: f, value, onChange }: { field: Field; value: unknown; onChange: (v: unknown) => void }) {
  switch (f.type) {
    case 'text':
      return f.multiline ? (
        <textarea rows={8} value={String(value ?? '')} onChange={(e) => onChange(e.target.value)} />
      ) : (
        <input type="text" value={String(value ?? '')} onChange={(e) => onChange(e.target.value)} />
      );
    case 'number':
      return (
        <div className="row">
          <input type="range" min={f.min} max={f.max} step={f.step} value={Number(value ?? f.min)} onChange={(e) => onChange(Number(e.target.value))} />
          <input className="num" type="number" min={f.min} max={f.max} step={f.step} value={Number(value ?? f.min)} onChange={(e) => onChange(Number(e.target.value))} />
        </div>
      );
    case 'color':
      return (
        <div className="row">
          <input type="color" value={String(value ?? '#000000')} onChange={(e) => onChange(e.target.value)} />
          <input className="hex" type="text" value={String(value ?? '')} onChange={(e) => onChange(e.target.value)} />
        </div>
      );
    case 'select':
      return (
        <select value={String(value ?? '')} onChange={(e) => onChange(e.target.value)}>
          {f.options.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>
      );
  }
}
