import { signal } from '@preact/signals';
import { connect, send } from './ipc';

export interface WorkspaceRef {
  id: string;
  name: string;
  /** 0xRRGGBB */
  colour: number;
}

export interface Result {
  /** Opaque: only the app can say what it means. */
  id: string;
  kind: 'tab' | 'command' | 'bookmark' | 'history' | 'entry' | 'tool';
  title: string;
  subtitle: string;
  workspace: WorkspaceRef | null;
  score: number;
  /** A badge: the environment of an entry (prod is drawn red). */
  tag?: 'dev' | 'stage' | 'prod' | 'selection' | null;
}

export type ScopeKind = 'contextual' | 'selected' | 'all';

/** What the app is searching right now. The app decides this; the launcher only asks (a request, not authority). */
export const scope = signal<{ kind: ScopeKind; workspaces: string[] }>({ kind: 'contextual', workspaces: [] });
/** Every workspace's name and colour, so the scope can be changed. */
export const allWorkspaces = signal<WorkspaceRef[]>([]);
export const results = signal<Result[]>([]);
export const query = signal('');
export const selected = signal(0);
/** What is selected on the page, as the app saw it when the launcher opened: its length and a short start. Null when nothing is. */
export const selection = signal<{ length: number; preview: string } | null>(null);

/** A tool open inside the launcher, with what to start it with. Its input is held here in memory only, and gone when the launcher closes. */
export interface OpenTool {
  tool: string;
  name: string;
  description: string;
  takes_input: boolean;
  sensitive: boolean;
  input: string;
  from_selection: boolean;
}
export interface Field {
  label: string;
  value: string;
  block: boolean;
}
export const tool = signal<OpenTool | null>(null);
export const toolInput = signal('');
export const toolOutput = signal<{ fields: Field[]; notes: string[] } | null>(null);
export const toolError = signal<string | null>(null);

/** Bumped whenever the app opens the launcher: the page clears its box and takes the focus. */
export const opened = signal(0);

const isRef = (v: unknown): v is WorkspaceRef => {
  const w = v as WorkspaceRef;
  return !!w && typeof w.id === 'string' && typeof w.name === 'string' && typeof w.colour === 'number';
};

const isResult = (v: unknown): v is Result => {
  const r = v as Result;
  return (
    !!r &&
    typeof r.id === 'string' &&
    ['tab', 'command', 'bookmark', 'history', 'entry', 'tool'].includes(r.kind) &&
    typeof r.title === 'string' &&
    typeof r.subtitle === 'string' &&
    (r.workspace === null || isRef(r.workspace)) &&
    typeof r.score === 'number' &&
    (r.tag === undefined || r.tag === null || ['dev', 'stage', 'prod', 'selection'].includes(r.tag))
  );
};

/** Handle one message from the app. Malformed messages are ignored, never half-applied. */
export function handleMessage(name: string, payload: string): void {
  try {
    if (name === 'launcher.opened') {
      query.value = '';
      selected.value = 0;
      opened.value++;
      tool.value = null;
      toolInput.value = '';
      toolOutput.value = null;
      toolError.value = null;
      selection.value = null;
    } else if (name === 'launcher.selection') {
      const s = JSON.parse(payload) as { length?: unknown; preview?: unknown } | null;
      selection.value = s && typeof s.length === 'number' && typeof s.preview === 'string' ? { length: s.length, preview: s.preview } : null;
      // a tool that reads input and has none yet starts with the selection
      if (tool.value?.takes_input && toolInput.value === '' && selection.value) send('launcher.run', { id: 'tool/' + tool.value.tool });
      // the order of the tools may change now that the app knows what is selected
      else if (!tool.value) send('launcher.search', { query: query.value });
    } else if (name === 'launcher.tool.opened') {
      const t = JSON.parse(payload) as OpenTool;
      if (typeof t.tool !== 'string' || typeof t.name !== 'string' || typeof t.input !== 'string' || typeof t.takes_input !== 'boolean') return;
      tool.value = t;
      toolInput.value = t.input;
      toolOutput.value = null;
      toolError.value = null;
    } else if (name === 'launcher.tool.output') {
      const o = JSON.parse(payload) as { tool: string; output: { fields: unknown; notes: unknown } | null; error: unknown };
      if (!tool.value || o.tool !== tool.value.tool) return;
      if (o.output && Array.isArray(o.output.fields) && Array.isArray(o.output.notes)) {
        toolOutput.value = { fields: o.output.fields as Field[], notes: o.output.notes.map(String) };
        toolError.value = null;
      } else {
        toolOutput.value = null;
        toolError.value = typeof o.error === 'string' ? o.error : 'that did not work';
      }
    } else if (name === 'launcher.results') {
      const r = JSON.parse(payload) as { scope: { kind: string; workspaces: unknown }; all: unknown; results: unknown };
      if (
        !r.scope ||
        !['contextual', 'selected', 'all'].includes(r.scope.kind) ||
        !Array.isArray(r.scope.workspaces) ||
        !Array.isArray(r.all) ||
        !r.all.every(isRef) ||
        !Array.isArray(r.results) ||
        !r.results.every(isResult)
      )
        return;
      scope.value = { kind: r.scope.kind as ScopeKind, workspaces: r.scope.workspaces.map(String) };
      allWorkspaces.value = r.all;
      results.value = r.results;
      const sel = (r as { selection?: { length?: unknown; preview?: unknown } | null }).selection;
      selection.value = sel && typeof sel.length === 'number' && typeof sel.preview === 'string' ? { length: sel.length, preview: sel.preview } : null;
      selected.value = 0;
    }
  } catch {
    // not valid JSON: a bug in the app, nothing for the user to act on
  }
}

export function start(): void {
  connect(handleMessage);
}

export const search = (q: string) => {
  query.value = q;
  send('launcher.search', { query: q });
};
/** Ask for a scope. The app narrows it to what exists and keeps it until the launcher closes. */
export const requestScope = (kind: ScopeKind, workspaces: string[] = []) => send('launcher.scope', { kind, workspaces });
export const run = (id: string) => send('launcher.run', { id });
export const close = () => send('launcher.close', {});
export const drag = (dx: number, dy: number) => send('launcher.drag', { dx: Math.round(dx), dy: Math.round(dy) });

/** Run the open tool on this text. Only text goes out; the app answers with labelled values. */
export const runTool = (id: string, input: string) => {
  toolInput.value = input;
  send('launcher.tool', { tool: id, input });
};
/** Leave the tool and go back to searching. The tool's input is dropped. */
export const leaveTool = () => {
  tool.value = null;
  toolInput.value = '';
  toolOutput.value = null;
  toolError.value = null;
  send('launcher.search', { query: query.value });
};
/** Copy one chosen value (the tool cannot read the clipboard; the page writes it). */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // fall back to a hidden box and the browser's own copy command
    const box = document.createElement('textarea');
    box.value = text;
    box.style.position = 'fixed';
    box.style.opacity = '0';
    document.body.appendChild(box);
    box.select();
    let ok = false;
    try {
      ok = document.execCommand('copy');
    } catch {
      ok = false;
    }
    box.remove();
    return ok;
  }
}
