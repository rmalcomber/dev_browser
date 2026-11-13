import { afterEach, describe, expect, it, vi } from 'vitest';
import { connect, inApp, send } from './ipc';

afterEach(() => {
  delete window.m0;
});

describe('ipc', () => {
  it('does nothing outside the app', () => {
    expect(inApp()).toBe(false);
    expect(send('workspaces.list')).toBe(false);
    expect(() => connect(() => {})).not.toThrow();
  });

  it('sends objects as JSON and strings as they are', () => {
    const sendFn = vi.fn();
    window.m0 = { send: sendFn, onmessage: null };
    expect(send('workspaces.create', { name: 'Work', colour: 1 })).toBe(true);
    expect(sendFn).toHaveBeenLastCalledWith('workspaces.create', '{"name":"Work","colour":1}');
    send('ping', 'hello');
    expect(sendFn).toHaveBeenLastCalledWith('ping', 'hello');
    send('workspaces.list');
    expect(sendFn).toHaveBeenLastCalledWith('workspaces.list', '{}');
  });

  it('delivers the app\'s messages to the handler', () => {
    window.m0 = { send: vi.fn(), onmessage: null };
    const handler = vi.fn();
    connect(handler);
    window.m0.onmessage?.('pong', 'hi');
    expect(handler).toHaveBeenCalledWith('pong', 'hi');
  });
});
