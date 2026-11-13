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
  kind: 'tab' | 'command';
  title: string;
  subtitle: string;
  workspace: WorkspaceRef | null;
  score: number;
}

export type ScopeKind = 'contextual' | 'selected' | 'all';

/** What the app is searching right now. The app decides this; the launcher only asks (a request, not authority). */
export const scope = signal<{ kind: ScopeKind; workspaces: string[] }>({ kind: 'contextual', workspaces: [] });
/** Every workspace's name and colour, so the scope can be changed. */
export const allWorkspaces = signal<WorkspaceRef[]>([]);
export const results = signal<Result[]>([]);
export const query = signal('');
export const selected = signal(0);
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
    (r.kind === 'tab' || r.kind === 'command') &&
    typeof r.title === 'string' &&
    typeof r.subtitle === 'string' &&
    (r.workspace === null || isRef(r.workspace)) &&
    typeof r.score === 'number'
  );
};

/** Handle one message from the app. Malformed messages are ignored, never half-applied. */
export function handleMessage(name: string, payload: string): void {
  try {
    if (name === 'launcher.opened') {
      query.value = '';
      selected.value = 0;
      opened.value++;
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
