import { signal } from '@preact/signals';

/** The binding the app installs as `window.m0`, only for pages served from app://shell. */
export interface M0 {
  send(name: string, payload: string): void;
  onmessage: ((name: string, payload: string) => void) | null;
}

declare global {
  interface Window {
    m0?: M0;
  }
}

/** The last reply from the app, as "name: payload" (null until one arrives). */
export const lastReply = signal<string | null>(null);

/** True when running inside the app (the binding exists). In a plain browser, for example under `vite dev`, it does not. */
export function inApp(): boolean {
  return typeof window !== 'undefined' && typeof window.m0?.send === 'function';
}

/** Start listening for replies. Call once at start-up. */
export function connect(): void {
  if (!window.m0) return;
  window.m0.onmessage = (name, payload) => {
    lastReply.value = `${name}: ${payload}`;
  };
}

/** Send a ping; the app answers `pong` with the same payload. Returns false when not running inside the app. */
export function ping(payload: string): boolean {
  if (!inApp()) return false;
  window.m0!.send('ping', payload);
  return true;
}
