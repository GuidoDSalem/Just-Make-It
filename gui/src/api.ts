// Cliente de la API de jmi-server. Los tipos reflejan jmi-core (fields.rs, render.rs).

export type FieldKind =
  | { type: 'text'; multiline: boolean }
  | { type: 'number'; min: number; max: number; step: number }
  | { type: 'color' }
  | { type: 'select'; options: { value: string; label: string }[] };

export type Field = { key: string; label: string; help?: string } & FieldKind;

export type Params = Record<string, unknown>;

export interface TemplateInfo {
  id: string;
  name: string;
  description: string;
  fields: Field[];
  defaults: Params;
}

export interface Project {
  template: string;
  params: Params;
}

export interface VideoInfo {
  width: number;
  height: number;
  fps: number;
  frames: number;
  seconds: number;
}

export interface Diagnostic {
  severity: string;
  message: string;
}

export interface PreviewRes {
  png: string;
  frame: number;
  seconds: number;
  diagnostics: Diagnostic[];
}

export type JobStatus =
  | { status: 'running' }
  | { status: 'done'; seconds: number }
  | { status: 'failed'; error: string }
  | { status: 'cancelled' };

export type Job = JobStatus & { id: number; progress: number; frames: number; total: number };

async function call<T>(path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const r = await fetch(`/api${path}`, {
    method: body === undefined ? 'GET' : 'POST',
    headers: body === undefined ? undefined : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal,
  });
  if (!r.ok) {
    const e = await r.json().catch(() => ({ error: r.statusText }));
    throw new Error(e.error ?? r.statusText);
  }
  return r.status === 204 ? (undefined as T) : r.json();
}

export const api = {
  templates: () => call<TemplateInfo[]>('/templates'),
  info: (p: Project, signal?: AbortSignal) => call<VideoInfo>('/info', p, signal),
  preview: (p: Project, at: string, scale: number, signal?: AbortSignal) =>
    call<PreviewRes>('/preview', { ...p, at, scale }, signal),
  render: (p: Project, scale: number) => call<{ id: number }>('/render', { ...p, scale }),
  job: (id: number) => call<Job>(`/jobs/${id}`),
  cancel: (id: number) => call<void>(`/jobs/${id}/cancel`, {}),
  fileUrl: (id: number) => `/api/jobs/${id}/file`,
};
