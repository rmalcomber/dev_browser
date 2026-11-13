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

/** True when running inside the app (the binding exists). In a plain browser, for example under `vite dev`, it does not. */
export function inApp(): boolean {
  return typeof window !== 'undefined' && typeof window.m0?.send === 'function';
}

/**
 * Send a message to the app. An object payload goes as JSON; a string goes as it is (`ping` takes any text).
 * Returns false when not running inside the app.
 */
export function send(name: string, payload: unknown = {}): boolean {
  if (!inApp()) return false;
  window.m0!.send(name, typeof payload === 'string' ? payload : JSON.stringify(payload));
  return true;
}

/** Start listening for the app's messages. Call once at start-up. */
export function connect(handler: (name: string, payload: string) => void): void {
  if (!window.m0) return;
  window.m0.onmessage = handler;
}
