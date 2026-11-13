import { signal } from '@preact/signals';
import { connect, send } from './ipc';

export interface Workspace {
  id: string;
  name: string;
  /** 0xRRGGBB */
  colour: number;
  position: number;
}

export interface Tab {
  id: string;
  title: string;
  url: string;
  position: number;
}

/** The workspace list in order, as the app last sent it. The app owns this; the shell never edits its own copy. */
export const workspaces = signal<Workspace[]>([]);
/** The id of the workspace being shown. Set only from the app's `workspaces` message. */
export const activeWorkspace = signal<string | null>(null);
/** The active workspace's tabs in order, and which one is selected. Set only from the app's `tabs` message. */
export const tabs = signal<Tab[]>([]);
export const selectedTab = signal<string | null>(null);
/** The last user-facing error from the app (a bad name, deleting the only workspace), or null. */
export const lastError = signal<string | null>(null);
/** Counters bumped when the app moves focus: into the sidebar (so it focuses a control) or out to the page (so open forms close). */
export const sidebarFocus = signal(0);
export const pageFocus = signal(0);

/** Colours offered when making or recolouring a workspace. */
export const PALETTE = [0x2c5fa8, 0xa78bfa, 0xf4a261, 0x2ecc71, 0xe76f51, 0x8fdde8, 0xffd166, 0xef476f];

export const css = (colour: number): string => '#' + (colour & 0xffffff).toString(16).padStart(6, '0');

/** What to show for a tab: its title, else its host name, else the address, else a placeholder. */
export function tabLabel(t: Pick<Tab, 'title' | 'url'>): string {
  if (t.title.trim()) return t.title.trim();
  try {
    const host = new URL(t.url).host;
    if (host) return host;
  } catch {
    // not a parseable address: fall through
  }
  return t.url || 'New tab';
}

function isWorkspace(v: unknown): v is Workspace {
  const w = v as Workspace;
  return (
    !!w &&
    typeof w.id === 'string' &&
    typeof w.name === 'string' &&
    typeof w.colour === 'number' &&
    typeof w.position === 'number'
  );
}

function isTab(v: unknown): v is Tab {
  const t = v as Tab;
  return (
    !!t &&
    typeof t.id === 'string' &&
    typeof t.title === 'string' &&
    typeof t.url === 'string' &&
    typeof t.position === 'number'
  );
}

/** Handle one message from the app. Unknown or malformed messages are ignored, never half-applied. */
export function handleMessage(name: string, payload: string): void {
  try {
    if (name === 'workspaces') {
      const s = JSON.parse(payload) as { workspaces: unknown; active: unknown };
      if (!Array.isArray(s.workspaces) || !s.workspaces.every(isWorkspace) || typeof s.active !== 'string') return;
      workspaces.value = s.workspaces;
      activeWorkspace.value = s.active;
      lastError.value = null;
    } else if (name === 'tabs') {
      const s = JSON.parse(payload) as { tabs: unknown; selected: unknown };
      if (!Array.isArray(s.tabs) || !s.tabs.every(isTab)) return;
      if (s.selected !== null && typeof s.selected !== 'string') return;
      tabs.value = s.tabs;
      selectedTab.value = s.selected;
    } else if (name === 'focus') {
      const f = JSON.parse(payload) as { target?: unknown };
      if (f.target === 'sidebar') sidebarFocus.value++;
      else if (f.target === 'page') pageFocus.value++;
    } else if (name === 'error') {
      const e = JSON.parse(payload) as { message?: unknown };
      lastError.value = typeof e.message === 'string' ? e.message : 'something went wrong';
    }
  } catch {
    // a message that is not valid JSON is a bug in the app, not something the user can act on
  }
}

/** Start listening and ask for the current state. */
export function start(): void {
  connect(handleMessage);
  send('workspaces.list');
}

export const createWorkspace = (name: string, colour: number) => send('workspaces.create', { name, colour });
export const renameWorkspace = (id: string, name: string) => send('workspaces.rename', { id, name });
export const recolourWorkspace = (id: string, colour: number) => send('workspaces.colour', { id, colour });
export const deleteWorkspace = (id: string) => send('workspaces.delete', { id });
export const activateWorkspace = (id: string) => send('workspaces.activate', { id });

export const newTab = () => send('tabs.new', {});
export const closeTab = (id: string) => send('tabs.close', { id });
export const selectTab = (id: string) => send('tabs.select', { id });
export const moveTab = (id: string, position: number) => send('tabs.move', { id, position });
/** Run a registered command by id: the same path a keyboard shortcut takes. */
export const runCommand = (id: string) => send('command', { id });
