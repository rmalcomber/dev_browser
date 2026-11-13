import { render } from 'preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PanelApp } from './PanelApp';
import { focusRequest, folded, handleMessage, sections, states } from './panelState';

let root: HTMLElement;
const tick = () => new Promise((r) => setTimeout(r, 0));
const sent = () => (window.m0!.send as ReturnType<typeof vi.fn>).mock.calls;

const tool = (id: number, over: object = {}) => ({
  id,
  tool: 'jwt',
  name: 'JWT decoder',
  description: 'Decode a token.',
  takes_input: true,
  sensitive: true,
  collapsed: false,
  ...over,
});
const widgets = (list: object[], ws = { id: 'ws-a', name: 'Work', colour: 0x2c5fa8 }) =>
  JSON.stringify({ open: true, width: 320, sections: [{ workspace: ws, widgets: list }] });

beforeEach(() => {
  root = document.createElement('div');
  document.body.appendChild(root);
  window.m0 = { send: vi.fn(), onmessage: null };
  sections.value = [];
  states.value = {};
  folded.value = [];
});
afterEach(() => {
  render(null, root);
  root.remove();
  delete window.m0;
});

describe('the widget panel', () => {
  it('shows a section for the workspace in view, coloured for it, with its pinned tools', async () => {
    handleMessage('widgets', widgets([tool(1), tool(2, { tool: 'uuid', name: 'UUID generator', takes_input: false, sensitive: false })]));
    render(<PanelApp />, root);
    await tick();
    expect(root.querySelector('.pwsname')!.textContent).toBe('Work');
    expect((root.querySelector('.psection') as HTMLElement).style.borderTopColor).not.toBe('');
    expect(Array.from(root.querySelectorAll('.pname')).map((e) => e.textContent)).toEqual(['JWT decoder', 'UUID generator']);
    expect(root.querySelector('.trun')).not.toBeNull();
  });

  it('says what to do when nothing is pinned', async () => {
    handleMessage('widgets', widgets([]));
    render(<PanelApp />, root);
    await tick();
    expect(root.querySelector('.pempty')!.textContent).toContain('press Pin');
  });

  it('sends only the widget id and the text when typing, and shows the answer', async () => {
    handleMessage('widgets', widgets([tool(7)]));
    render(<PanelApp />, root);
    await tick();
    const box = root.querySelector('textarea')!;
    box.value = 'a.b.c';
    box.dispatchEvent(new Event('input', { bubbles: true }));
    expect(sent()).toContainEqual(['widget.run', JSON.stringify({ id: 7, input: 'a.b.c' })]);
    handleMessage('widget.output', JSON.stringify({ id: 7, output: { fields: [{ label: 'sub', value: 'alice', block: false }], notes: ['Not verified.'] }, error: null }));
    await tick();
    expect(root.querySelector('.tlabel')!.textContent).toBe('sub');
    expect(root.querySelector('.tnote')!.textContent).toBe('Not verified.');
    handleMessage('widget.output', JSON.stringify({ id: 7, output: null, error: 'the payload is not JSON' }));
    await tick();
    expect(root.querySelector('.terror')!.textContent).toBe('the payload is not JSON');
  });

  it('collapses, expands and unpins by asking the app, and changes nothing itself', async () => {
    handleMessage('widgets', widgets([tool(3)]));
    render(<PanelApp />, root);
    await tick();
    (root.querySelector('.pfold') as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['widgets.collapse', JSON.stringify({ id: 3, collapsed: true })]);
    expect(root.querySelector('textarea')).not.toBeNull();
    (root.querySelector('button[aria-label="Unpin JWT decoder"]') as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['widgets.unpin', JSON.stringify({ id: 3 })]);
    expect(root.querySelector('.pname')).not.toBeNull();
    handleMessage('widgets', widgets([tool(3, { collapsed: true })]));
    await tick();
    expect(root.querySelector('textarea')).toBeNull();
  });

  it('folds a workspace section on its own, in the page', async () => {
    handleMessage('widgets', widgets([tool(1)]));
    render(<PanelApp />, root);
    await tick();
    (root.querySelector('.phead') as HTMLButtonElement).click();
    await tick();
    expect(root.querySelector('.pwidget')).toBeNull();
    (root.querySelector('.phead') as HTMLButtonElement).click();
    await tick();
    expect(root.querySelector('.pwidget')).not.toBeNull();
  });

  it('forgets what was typed into a secret tool when the panel closes, and keeps the rest', () => {
    handleMessage('widgets', widgets([tool(1), tool(2, { tool: 'json', name: 'JSON formatter', sensitive: false })]));
    states.value = { 1: { input: 'secret-token', output: null, error: null }, 2: { input: '{"a":1}', output: null, error: null } };
    handleMessage('panel.closed', '{}');
    expect(states.value[1]).toBeUndefined();
    expect(states.value[2].input).toBe('{"a":1}');
  });

  it('forgets a widget\'s text when it leaves the view (unpinned, or another workspace is shown)', () => {
    handleMessage('widgets', widgets([tool(1), tool(2, { tool: 'json', sensitive: false })]));
    states.value = { 1: { input: 'secret-token', output: null, error: null }, 2: { input: 'x', output: null, error: null } };
    handleMessage('widgets', widgets([tool(2, { tool: 'json', sensitive: false })], { id: 'ws-b', name: 'Personal', colour: 0xa78bfa }));
    expect(Object.keys(states.value)).toEqual(['2']);
    handleMessage('widgets', widgets([]));
    expect(states.value).toEqual({});
  });

  it('takes the focus when the app asks', async () => {
    handleMessage('widgets', widgets([tool(1)]));
    render(<PanelApp />, root);
    await tick();
    handleMessage('panel.focus', '{}');
    await tick();
    expect(focusRequest.value).toBeGreaterThan(0);
    expect(document.activeElement?.tagName).toBe('TEXTAREA');
  });

  it('resizes by the left edge, sending only the movement, and says when the drag ends', async () => {
    handleMessage('widgets', widgets([]));
    render(<PanelApp />, root);
    await tick();
    await new Promise((r) => setTimeout(r, 50)); // effects run after the paint
    const grip = root.querySelector('.pgrip') as HTMLElement;
    const ev = (type: string, x: number) => {
      const e = new Event(type, { bubbles: true }) as Event & { screenX: number; pointerId: number };
      Object.assign(e, { screenX: x, pointerId: 1 });
      grip.dispatchEvent(e);
    };
    ev('pointerdown', 500);
    ev('pointermove', 460);
    ev('pointerup', 460);
    ev('pointermove', 100);
    expect(sent().filter((c) => c[0] === 'panel.resize')).toEqual([
      ['panel.resize', JSON.stringify({ dx: -40, done: false })],
      ['panel.resize', JSON.stringify({ dx: 0, done: true })],
    ]);
  });

  it('shows names and values as text, never markup, and ignores malformed messages', async () => {
    handleMessage('widgets', widgets([tool(1, { name: '<img src=x onerror=alert(1)>' })]));
    render(<PanelApp />, root);
    await tick();
    expect(root.querySelector('.pwidget img')).toBeNull();
    for (const bad of ['not json', '{}', JSON.stringify({ open: 'yes', width: 1, sections: [] }), JSON.stringify({ open: true, width: 1, sections: [{ workspace: {}, widgets: [] }] })]) handleMessage('widgets', bad);
    expect(sections.value.length).toBe(1);
    handleMessage('widget.output', 'nope');
    handleMessage('widget.output', JSON.stringify({ id: 'x' }));
  });
});
