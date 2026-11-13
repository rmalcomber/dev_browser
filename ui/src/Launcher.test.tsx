import { render } from 'preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Launcher } from './Launcher';
import { allWorkspaces, handleMessage, opened, query, results, scope, selected } from './launcherState';

const WS = [
  { id: 'ws-a', name: 'Work', colour: 0x2c5fa8 },
  { id: 'ws-b', name: 'Personal', colour: 0xa78bfa },
];
const res = (id: string, title: string, ws: (typeof WS)[number] | null, kind = 'tab') => ({
  id,
  kind,
  title,
  subtitle: 'https://x.example/',
  workspace: ws,
  score: 10,
});
const payload = (r: object[], kind = 'contextual', workspaces = ['ws-a']) =>
  JSON.stringify({ scope: { kind, workspaces }, all: WS, results: r });

let root: HTMLElement;
const tick = () => new Promise((r) => setTimeout(r, 0));
const sent = () => (window.m0!.send as ReturnType<typeof vi.fn>).mock.calls;

beforeEach(() => {
  root = document.createElement('div');
  document.body.appendChild(root);
  window.m0 = { send: vi.fn(), onmessage: null };
  results.value = [];
  allWorkspaces.value = [];
  query.value = '';
  selected.value = 0;
  scope.value = { kind: 'contextual', workspaces: [] };
});
afterEach(() => {
  render(null, root);
  root.remove();
  delete window.m0;
});

describe('the launcher page', () => {
  it('shows what the app sent, with the workspace of each tab', async () => {
    handleMessage(
      'launcher.results',
      payload([res('tab/ws-a/t1', 'Quarterly report', WS[0]), res('cmd/tab.new', 'New tab', null, 'command')]),
    );
    render(<Launcher />, root);
    await tick();
    const rows = Array.from(root.querySelectorAll('.result'));
    expect(rows.map((r) => r.querySelector('.ltitle')!.textContent)).toEqual(['Quarterly report', 'New tab']);
    expect(rows[0].querySelector('.ldot')!.getAttribute('title')).toBe('Work');
    expect(root.querySelector('.lscope')!.textContent).toBe('Searching this workspace');
  });

  it('sends only the query when typing, never a scope', async () => {
    render(<Launcher />, root);
    const box = root.querySelector('input')!;
    box.value = 'quar';
    box.dispatchEvent(new Event('input', { bubbles: true }));
    expect(sent()).toContainEqual(['launcher.search', JSON.stringify({ query: 'quar' })]);
  });

  it('moves the selection with the arrow keys, runs the selected result on Enter and closes on Escape', async () => {
    handleMessage('launcher.results', payload([res('tab/ws-a/t1', 'One', WS[0]), res('tab/ws-a/t2', 'Two', WS[0])]));
    render(<Launcher />, root);
    await tick();
    const box = root.querySelector('input')!;
    box.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    box.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    box.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(sent()).toContainEqual(['launcher.run', JSON.stringify({ id: 'tab/ws-a/t2' })]);
    box.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(sent()).toContainEqual(['launcher.close', '{}']);
  });

  it('asks for a wider scope, and says so on screen only once the app agrees', async () => {
    render(<Launcher />, root);
    await tick();
    const all = Array.from(root.querySelectorAll('.lscopes button')).find((b) => b.textContent?.includes('All workspaces'));
    (all as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['launcher.scope', JSON.stringify({ kind: 'all', workspaces: [] })]);
    // the page does not change its own scope: it waits for the app
    expect(root.querySelector('.lscope')!.textContent).toBe('Searching this workspace');
    handleMessage('launcher.results', payload([], 'all', ['ws-a', 'ws-b']));
    await tick();
    expect(root.querySelector('.lscope')!.textContent).toBe('Searching every workspace');
  });

  it('chooses workspaces one by one', async () => {
    handleMessage('launcher.results', payload([]));
    render(<Launcher />, root);
    await tick();
    const personal = Array.from(root.querySelectorAll('.lws')).find((b) => b.textContent?.includes('Personal'));
    (personal as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['launcher.scope', JSON.stringify({ kind: 'selected', workspaces: ['ws-b'] })]);
  });

  it('starts afresh when the app opens it', () => {
    query.value = 'old';
    selected.value = 3;
    handleMessage('launcher.opened', '{}');
    expect(query.value).toBe('');
    expect(selected.value).toBe(0);
    expect(opened.value).toBeGreaterThan(0);
  });

  it('shows text, never markup, and ignores malformed results', async () => {
    handleMessage('launcher.results', payload([res('tab/ws-a/t1', '<img src=x onerror=alert(1)>', WS[0])]));
    render(<Launcher />, root);
    await tick();
    expect(root.querySelector('.result img')).toBeNull();
    expect(root.querySelector('.ltitle')!.textContent).toBe('<img src=x onerror=alert(1)>');
    const bad = [
      'not json',
      '{}',
      JSON.stringify({ scope: { kind: 'everything', workspaces: [] }, all: [], results: [] }),
      payload([{ id: 5 }]),
    ];
    for (const b of bad) handleMessage('launcher.results', b);
    expect(results.value.length).toBe(1);
  });

  it('drags by the title bar, sending only the movement', async () => {
    render(<Launcher />, root);
    const bar = root.querySelector('.lbar') as HTMLElement;
    const ev = (type: string, x: number, y: number) => {
      const e = new MouseEvent(type, { bubbles: true, screenX: x, screenY: y });
      (type === 'mousedown' ? bar : window).dispatchEvent(e);
    };
    ev('mousedown', 100, 100);
    ev('mousemove', 130, 90);
    ev('mouseup', 130, 90);
    ev('mousemove', 500, 500);
    expect(sent().filter((c) => c[0] === 'launcher.drag')).toEqual([['launcher.drag', JSON.stringify({ dx: 30, dy: -10 })]]);
  });
});
