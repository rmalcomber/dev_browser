import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  activateWorkspace,
  activeWorkspace,
  closeTab,
  createWorkspace,
  css,
  deleteWorkspace,
  handleMessage,
  lastError,
  moveTab,
  newTab,
  pageFocus,
  recolourWorkspace,
  renameWorkspace,
  runCommand,
  selectTab,
  selectedTab,
  sidebarFocus,
  start,
  tabLabel,
  tabs,
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
  tabs.value = [];
  selectedTab.value = null;
  sidebarFocus.value = 0;
  pageFocus.value = 0;
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

const tabsMsg = (selected: string | null, items: Array<[string, string, string]>) =>
  JSON.stringify({
    workspace: 'ws-0',
    selected,
    tabs: items.map(([id, title, url], i) => ({ id, title, url, position: i })),
  });

describe('tabs', () => {
  it('takes the tab list and the selection from a tabs message', () => {
    handleMessage('tabs', tabsMsg('tab-b', [['tab-a', 'A', 'https://a.example/'], ['tab-b', 'B', 'https://b.example/']]));
    expect(tabs.value.map((t) => t.title)).toEqual(['A', 'B']);
    expect(selectedTab.value).toBe('tab-b');
    handleMessage('tabs', tabsMsg(null, []));
    expect(tabs.value).toEqual([]);
    expect(selectedTab.value).toBeNull();
  });

  it('ignores malformed tabs messages without changing anything', () => {
    handleMessage('tabs', tabsMsg('tab-a', [['tab-a', 'A', 'https://a.example/']]));
    for (const bad of [
      'not json',
      'null',
      '{"tabs":[{"id":1}],"selected":null}',
      '{"tabs":[],"selected":5}',
      '{"tabs":"x","selected":null}',
    ]) {
      expect(() => handleMessage('tabs', bad)).not.toThrow();
    }
    expect(tabs.value.map((t) => t.title)).toEqual(['A']);
    expect(selectedTab.value).toBe('tab-a');
  });

  it('labels a tab by title, then host name, then address', () => {
    expect(tabLabel({ title: '  Hello  ', url: 'https://a.example/x' })).toBe('Hello');
    expect(tabLabel({ title: '', url: 'https://a.example:8080/x' })).toBe('a.example:8080');
    expect(tabLabel({ title: '', url: 'about:blank' })).toBe('about:blank');
    expect(tabLabel({ title: '', url: '' })).toBe('New tab');
    expect(tabLabel({ title: ' ', url: 'not a url' })).toBe('not a url');
  });

  it('counts the focus moves the app asks for', () => {
    handleMessage('focus', '{"target":"sidebar"}');
    handleMessage('focus', '{"target":"sidebar"}');
    handleMessage('focus', '{"target":"page"}');
    handleMessage('focus', '{"target":"elsewhere"}');
    handleMessage('focus', 'not json');
    expect(sidebarFocus.value).toBe(2);
    expect(pageFocus.value).toBe(1);
  });

  it('tab actions and commands send the right messages', () => {
    const send = vi.fn();
    window.m0 = { send, onmessage: null };
    newTab();
    selectTab('tab-1');
    closeTab('tab-2');
    moveTab('tab-3', 0);
    runCommand('tab.next');
    expect(send.mock.calls).toEqual([
      ['tabs.new', '{}'],
      ['tabs.select', '{"id":"tab-1"}'],
      ['tabs.close', '{"id":"tab-2"}'],
      ['tabs.move', '{"id":"tab-3","position":0}'],
      ['command', '{"id":"tab.next"}'],
    ]);
  });
});
