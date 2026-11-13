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
  /** The id of the group the tab is in, or null. */
  group: string | null;
}

export interface Group {
  id: string;
  name: string;
  position: number;
  collapsed: boolean;
}

/** One expanded workspace's tabs and groups, in the order the sidebar shows them. */
export interface Section {
  workspace: string;
  selected: string | null;
  groups: Group[];
  tabs: Tab[];
}

/** The workspace list in order, as the app last sent it. The app owns this; the shell never edits its own copy. */
export const workspaces = signal<Workspace[]>([]);
/** The id of the workspace being shown. Set only from the app's `workspaces` message. */
export const activeWorkspace = signal<string | null>(null);
/**
 * Tabs and groups of the **expanded** workspaces only. The app sends a section for a workspace only while it is expanded, so a collapsed
 * workspace's titles and addresses never reach this page (S11). Whether a workspace is expanded is simply whether it has a section.
 */
export const sections = signal<Section[]>([]);
/** The last user-facing error from the app (a bad name, deleting the only workspace), or null. */
export const lastError = signal<string | null>(null);
/** Counters bumped when the app moves focus: into the sidebar (focus a control), out to the page (close open forms), or to the new-workspace field. */
export const sidebarFocus = signal(0);
export const pageFocus = signal(0);
export const newWorkspaceRequest = signal(0);

/** Colours offered when making or recolouring a workspace. */
export const PALETTE = [0x2c5fa8, 0xa78bfa, 0xf4a261, 0x2ecc71, 0xe76f51, 0x8fdde8, 0xffd166, 0xef476f];

export const css = (colour: number): string => '#' + (colour & 0xffffff).toString(16).padStart(6, '0');

export const sectionOf = (workspace: string): Section | undefined => sections.value.find((s) => s.workspace === workspace);
export const isExpanded = (workspace: string): boolean => sectionOf(workspace) !== undefined;

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

/** The first letter of a workspace name, for its icon. */
export function initial(name: string): string {
  const first = Array.from(name.trim())[0];
  return first ? first.toUpperCase() : '?';
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
    typeof t.position === 'number' &&
    (t.group === null || typeof t.group === 'string')
  );
}

function isGroup(v: unknown): v is Group {
  const g = v as Group;
  return (
    !!g &&
    typeof g.id === 'string' &&
    typeof g.name === 'string' &&
    typeof g.position === 'number' &&
    typeof g.collapsed === 'boolean'
  );
}

function isSection(v: unknown): v is Section {
  const s = v as Section;
  return (
    !!s &&
    typeof s.workspace === 'string' &&
    (s.selected === null || typeof s.selected === 'string') &&
    Array.isArray(s.groups) &&
    s.groups.every(isGroup) &&
    Array.isArray(s.tabs) &&
    s.tabs.every(isTab)
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
      const s = JSON.parse(payload) as { sections: unknown };
      if (!Array.isArray(s.sections) || !s.sections.every(isSection)) return;
      sections.value = s.sections;
    } else if (name === 'focus') {
      const f = JSON.parse(payload) as { target?: unknown };
      if (f.target === 'sidebar') sidebarFocus.value++;
      else if (f.target === 'page') pageFocus.value++;
      else if (f.target === 'new-workspace') newWorkspaceRequest.value++;
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
/** Expand or collapse a workspace in the sidebar without switching to it. */
export const expandWorkspace = (id: string, expanded: boolean) => send('workspaces.expand', { id, expanded });

export const newTab = () => send('tabs.new', {});
export const closeTab = (id: string) => send('tabs.close', { id });
export const selectTab = (id: string) => send('tabs.select', { id });
export const moveTab = (id: string, position: number) => send('tabs.move', { id, position });
/** Open a tab of any expanded workspace: switches to its workspace and collapses the others. */
export const openTab = (workspace: string, id: string) => send('tabs.open', { workspace, id });
export const groupTab = (id: string, group: string | null) => send('tabs.group', { id, group });
export const newGroup = (tab: string) => send('groups.new', { tab });
export const renameGroup = (id: string, name: string) => send('groups.rename', { id, name });
export const collapseGroup = (id: string, collapsed: boolean) => send('groups.collapse', { id, collapsed });
/** Run a registered command by id: the same path a keyboard shortcut takes. */
export const runCommand = (id: string) => send('command', { id });
