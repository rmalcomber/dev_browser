import { afterEach, describe, expect, it, vi } from 'vitest';
import { connect, inApp, lastReply, ping } from './ipc';

afterEach(() => {
  delete window.m0;
  lastReply.value = null;
});

describe('ipc v0', () => {
  it('does nothing outside the app', () => {
    expect(inApp()).toBe(false);
    expect(ping('x')).toBe(false);
    expect(() => connect()).not.toThrow();
  });

  it('sends a ping through window.m0', () => {
    const send = vi.fn();
    window.m0 = { send, onmessage: null };
    expect(ping('hello')).toBe(true);
    expect(send).toHaveBeenCalledWith('ping', 'hello');
  });

  it('shows the reply the app sends back', () => {
    window.m0 = { send: vi.fn(), onmessage: null };
    connect();
    window.m0.onmessage?.('pong', 'hello');
    expect(lastReply.value).toBe('pong: hello');
  });
});
