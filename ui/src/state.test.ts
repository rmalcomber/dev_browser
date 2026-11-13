import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  activateWorkspace,
  activeWorkspace,
  closeTab,
  collapseGroup,
  createWorkspace,
  css,
  deleteWorkspace,
  expandWorkspace,
  groupTab,
  handleMessage,
  initial,
  isExpanded,
  lastError,
  moveTab,
  newGroup,
  newTab,
  newWorkspaceRequest,
  openTab,
  pageFocus,
  recolourWorkspace,
  renameGroup,
  renameWorkspace,
  runCommand,
  sectionOf,
  sections,
  selectTab,
  sidebarFocus,
  start,
  tabLabel,
  workspaces,
} from './state';

const state = (active: string, names: string[]) =>
  JSON.stringify({
    workspaces: names.map((n, i) => ({ id: `ws-${i}`, name: n, colour: 0x2c5fa8, position: i })),
    active,
  });

beforeEach(() => {
  workspaces.value = [];
  activeWorkspace.value = null;
  lastError.value = null;
  sections.value = [];
  sidebarFocus.value = 0;
  pageFocus.value = 0;
  newWorkspaceRequest.value = 0;
});
afterEach(() => {
  delete window.m0;
});

describe('shell state', () => {
  it('starts with no workspaces and none active', () => {
    expect(workspaces.value).toEqual([]);
    expect(activeWorkspace.value).toBeNull();
  });

  it('takes the list and the active id from a workspaces message and clears the error', () => {
    lastError.value = 'old';
    handleMessage('workspaces', state('ws-1', ['A', 'B']));
    expect(workspaces.value.map((w) => w.name)).toEqual(['A', 'B']);
    expect(activeWorkspace.value).toBe('ws-1');
    expect(lastError.value).toBeNull();
  });

  it('shows an error message from the app', () => {
    handleMessage('error', JSON.stringify({ request: 'workspaces.delete', message: 'you cannot delete your only workspace' }));
    expect(lastError.value).toBe('you cannot delete your only workspace');
  });

  it('ignores malformed or unknown messages without changing anything', () => {
    handleMessage('workspaces', state('ws-0', ['A']));
    for (const [name, payload] of [
      ['workspaces', 'not json'],
      ['workspaces', '{"workspaces":[{"id":1}],"active":"x"}'],
      ['workspaces', '{"workspaces":[],"active":5}'],
      ['workspaces', 'null'],
      ['mystery', '{}'],
    ] as const) {
      expect(() => handleMessage(name, payload)).not.toThrow();
    }
    expect(workspaces.value.map((w) => w.name)).toEqual(['A']);
    expect(activeWorkspace.value).toBe('ws-0');
  });

  it('formats colours for CSS', () => {
    expect(css(0x2c5fa8)).toBe('#2c5fa8');
    expect(css(0xff)).toBe('#0000ff');
  });
});

describe('actions send the right messages', () => {
  it('asks for the list on start and listens for replies', () => {
    const send = vi.fn();
    window.m0 = { send, onmessage: null };
    start();
    expect(send).toHaveBeenCalledWith('workspaces.list', '{}');
    window.m0.onmessage?.('workspaces', state('ws-0', ['Only']));
    expect(workspaces.value.map((w) => w.name)).toEqual(['Only']);
  });

  it('create, rename, recolour, delete and activate', () => {
    const send = vi.fn();
    window.m0 = { send, onmessage: null };
    createWorkspace('Work', 5);
    renameWorkspace('ws-1', 'Home');
    recolourWorkspace('ws-1', 7);
    deleteWorkspace('ws-1');
    activateWorkspace('ws-2');
    expect(send.mock.calls).toEqual([
      ['workspaces.create', '{"name":"Work","colour":5}'],
      ['workspaces.rename', '{"id":"ws-1","name":"Home"}'],
      ['workspaces.colour', '{"id":"ws-1","colour":7}'],
      ['workspaces.delete', '{"id":"ws-1"}'],
      ['workspaces.activate', '{"id":"ws-2"}'],
    ]);
  });
});

const section = (workspace: string, selected: string | null, items: Array<[string, string, string, string | null]>, groups: Array<[string, string, boolean]> = []) => ({
  workspace,
  selected,
  groups: groups.map(([id, name, collapsed], i) => ({ id, name, position: i, collapsed })),
  tabs: items.map(([id, title, url, group], i) => ({ id, title, url, position: i, group })),
});
const tabsMsg = (...s: ReturnType<typeof section>[]) => JSON.stringify({ sections: s });

describe('tabs and sections', () => {
  it('takes the sections from a tabs message; a workspace is expanded exactly when it has one', () => {
    handleMessage('tabs', tabsMsg(section('ws-0', 'tab-b', [['tab-a', 'A', 'https://a.example/', null], ['tab-b', 'B', 'https://b.example/', null]])));
    expect(sectionOf('ws-0')?.tabs.map((t) => t.title)).toEqual(['A', 'B']);
    expect(sectionOf('ws-0')?.selected).toBe('tab-b');
    expect(isExpanded('ws-0')).toBe(true);
    expect(isExpanded('ws-1')).toBe(false);
    handleMessage('tabs', tabsMsg());
    expect(sections.value).toEqual([]);
    expect(isExpanded('ws-0')).toBe(false);
  });

  it('carries groups and which group each tab is in', () => {
    handleMessage('tabs', tabsMsg(section('ws-0', 'tab-a', [['tab-a', 'A', 'https://a.example/', 'grp-1'], ['tab-b', 'B', 'https://b.example/', null]], [['grp-1', 'Reading', true]])));
    const s = sectionOf('ws-0')!;
    expect(s.groups).toEqual([{ id: 'grp-1', name: 'Reading', position: 0, collapsed: true }]);
    expect(s.tabs.map((t) => t.group)).toEqual(['grp-1', null]);
  });

  it('ignores malformed tabs messages without changing anything', () => {
    handleMessage('tabs', tabsMsg(section('ws-0', 'tab-a', [['tab-a', 'A', 'https://a.example/', null]])));
    for (const bad of [
      'not json',
      'null',
      '{"sections":"x"}',
      '{"sections":[{"workspace":1,"selected":null,"groups":[],"tabs":[]}]}',
      '{"sections":[{"workspace":"ws-0","selected":5,"groups":[],"tabs":[]}]}',
      '{"sections":[{"workspace":"ws-0","selected":null,"groups":[{"id":1}],"tabs":[]}]}',
      '{"sections":[{"workspace":"ws-0","selected":null,"groups":[],"tabs":[{"id":"t","title":"x","url":"u","position":0}]}]}',
    ]) {
      expect(() => handleMessage('tabs', bad)).not.toThrow();
    }
    expect(sectionOf('ws-0')?.tabs.map((t) => t.title)).toEqual(['A']);
  });

  it('labels a tab by title, then host name, then address', () => {
    expect(tabLabel({ title: '  Hello  ', url: 'https://a.example/x' })).toBe('Hello');
    expect(tabLabel({ title: '', url: 'https://a.example:8080/x' })).toBe('a.example:8080');
    expect(tabLabel({ title: '', url: 'about:blank' })).toBe('about:blank');
    expect(tabLabel({ title: '', url: '' })).toBe('New tab');
    expect(tabLabel({ title: ' ', url: 'not a url' })).toBe('not a url');
  });

  it('makes an icon letter from the name', () => {
    expect(initial('Work')).toBe('W');
    expect(initial('  personal')).toBe('P');
    expect(initial('école')).toBe('É');
    expect(initial('   ')).toBe('?');
    expect(initial('🚀 launch')).toBe('🚀');
  });

  it('counts the focus moves the app asks for', () => {
    handleMessage('focus', '{"target":"sidebar"}');
    handleMessage('focus', '{"target":"sidebar"}');
    handleMessage('focus', '{"target":"page"}');
    handleMessage('focus', '{"target":"new-workspace"}');
    handleMessage('focus', '{"target":"elsewhere"}');
    handleMessage('focus', 'not json');
    expect(sidebarFocus.value).toBe(2);
    expect(pageFocus.value).toBe(1);
    expect(newWorkspaceRequest.value).toBe(1);
  });

  it('tab, group and expand actions send the right messages', () => {
    const send = vi.fn();
    window.m0 = { send, onmessage: null };
    newTab();
    selectTab('tab-1');
    closeTab('tab-2');
    moveTab('tab-3', 0);
    openTab('ws-1', 'tab-4');
    groupTab('tab-5', 'grp-1');
    groupTab('tab-5', null);
    newGroup('tab-6');
    renameGroup('grp-1', 'Docs');
    collapseGroup('grp-1', true);
    expandWorkspace('ws-2', false);
    runCommand('tab.next');
    expect(send.mock.calls).toEqual([
      ['tabs.new', '{}'],
      ['tabs.select', '{"id":"tab-1"}'],
      ['tabs.close', '{"id":"tab-2"}'],
      ['tabs.move', '{"id":"tab-3","position":0}'],
      ['tabs.open', '{"workspace":"ws-1","id":"tab-4"}'],
      ['tabs.group', '{"id":"tab-5","group":"grp-1"}'],
      ['tabs.group', '{"id":"tab-5","group":null}'],
      ['groups.new', '{"tab":"tab-6"}'],
      ['groups.rename', '{"id":"grp-1","name":"Docs"}'],
      ['groups.collapse', '{"id":"grp-1","collapsed":true}'],
      ['workspaces.expand', '{"id":"ws-2","expanded":false}'],
      ['command', '{"id":"tab.next"}'],
    ]);
  });
});
