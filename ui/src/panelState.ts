import { signal } from '@preact/signals';
import { connect, send } from './ipc';
import type { Field } from './launcherState';

export interface PinnedTool {
  id: number;
  tool: string;
  name: string;
  description: string;
  takes_input: boolean;
  sensitive: boolean;
  collapsed: boolean;
}
export interface Section {
  workspace: { id: string; name: string; colour: number };
  widgets: PinnedTool[];
}
/** What was typed into a widget and what it answered: memory only, never sent anywhere but to the app to run the tool. */
export interface WidgetState {
  input: string;
  output: { fields: Field[]; notes: string[] } | null;
  error: string | null;
}

/** The pinned tools of the workspaces in view, as the app sent them. The app owns this; the panel never edits its own copy. */
export const sections = signal<Section[]>([]);
export const open = signal(false);
export const width = signal(320);
export const states = signal<Record<number, WidgetState>>({});
/** Workspace sections the user folded (this run only). */
export const folded = signal<string[]>([]);
/** Bumped when the app wants the keyboard focus inside the panel. */
export const focusRequest = signal(0);

const empty = (): WidgetState => ({ input: '', output: null, error: null });

function isTool(v: unknown): v is PinnedTool {
  const t = v as PinnedTool;
  return (
    !!t &&
    Number.isInteger(t.id) &&
    typeof t.tool === 'string' &&
    typeof t.name === 'string' &&
    typeof t.description === 'string' &&
    typeof t.takes_input === 'boolean' &&
    typeof t.sensitive === 'boolean' &&
    typeof t.collapsed === 'boolean'
  );
}

function isSection(v: unknown): v is Section {
  const s = v as Section;
  return (
    !!s &&
    !!s.workspace &&
    typeof s.workspace.id === 'string' &&
    typeof s.workspace.name === 'string' &&
    typeof s.workspace.colour === 'number' &&
    Array.isArray(s.widgets) &&
    s.widgets.every(isTool)
  );
}

/** Forget what was typed into tools that handle secrets (a token), keeping the rest. */
function clearSecrets() {
  const next: Record<number, WidgetState> = {};
  const sensitive = new Set(sections.value.flatMap((s) => s.widgets.filter((w) => w.sensitive).map((w) => w.id)));
  for (const [id, st] of Object.entries(states.value)) if (!sensitive.has(Number(id))) next[Number(id)] = st;
  states.value = next;
}

/** Handle one message from the app. Unknown or malformed messages are ignored, never half-applied. */
export function handleMessage(name: string, payload: string): void {
  try {
    if (name === 'widgets') {
      const p = JSON.parse(payload) as { open: unknown; width: unknown; sections: unknown };
      if (typeof p.open !== 'boolean' || typeof p.width !== 'number' || !Array.isArray(p.sections) || !p.sections.every(isSection)) return;
      const inView = new Set(p.sections.flatMap((s) => s.widgets.map((w) => w.id)));
      // a widget that left the view (unpinned, or its workspace is no longer shown) loses what was typed into it; the ones still in view keep it
      const next: Record<number, WidgetState> = {};
      for (const [id, st] of Object.entries(states.value)) if (inView.has(Number(id))) next[Number(id)] = st;
      states.value = next;
      sections.value = p.sections;
      open.value = p.open;
      width.value = p.width;
    } else if (name === 'widget.output') {
      const o = JSON.parse(payload) as { id: number; output: { fields: unknown; notes: unknown } | null; error: unknown };
      if (!Number.isInteger(o.id)) return;
      const cur = states.value[o.id] ?? empty();
      const next: WidgetState =
        o.output && Array.isArray(o.output.fields) && Array.isArray(o.output.notes)
          ? { ...cur, output: { fields: o.output.fields as Field[], notes: o.output.notes.map(String) }, error: null }
          : { ...cur, output: null, error: typeof o.error === 'string' ? o.error : 'that did not work' };
      states.value = { ...states.value, [o.id]: next };
    } else if (name === 'panel.focus') {
      focusRequest.value++;
    } else if (name === 'panel.closed') {
      clearSecrets();
    }
  } catch {
    // not valid JSON: a bug in the app, nothing for the user to act on
  }
}

export function start(): void {
  connect(handleMessage);
  send('widgets.list');
}

export const runWidget = (id: number, input: string) => {
  states.value = { ...states.value, [id]: { ...(states.value[id] ?? empty()), input } };
  send('widget.run', { id, input });
};
export const unpin = (id: number) => send('widgets.unpin', { id });
export const collapse = (id: number, collapsed: boolean) => send('widgets.collapse', { id, collapsed });
export const resize = (dx: number, done: boolean) => send('panel.resize', { dx: Math.round(dx), done });
export const toggleSection = (workspace: string) => {
  folded.value = folded.value.includes(workspace) ? folded.value.filter((w) => w !== workspace) : [...folded.value, workspace];
};
export const widgetState = (id: number): WidgetState => states.value[id] ?? empty();
