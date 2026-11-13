import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  activateWorkspace,
  activeWorkspace,
  createWorkspace,
  css,
  deleteWorkspace,
  handleMessage,
  lastError,
  recolourWorkspace,
  renameWorkspace,
  start,
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
