import { render } from 'preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Sidebar } from './Sidebar';
import { activeWorkspace, addressFocus, handleMessage, nav, sections, workspaces } from './state';

const WS = [
  { id: 'ws-a', name: 'Work', colour: 0x2c5fa8, position: 0 },
  { id: 'ws-b', name: 'Personal', colour: 0xa78bfa, position: 1 },
];
const PNG = 'data:image/png;base64,iVBORw0KGgo=';
const tab = (id: string, title: string, url: string, icon?: string | null) => ({ id, title, url, position: 0, group: null, icon });

let root: HTMLElement;
const tick = () => new Promise((r) => setTimeout(r, 0));
const sent = () => (window.m0!.send as ReturnType<typeof vi.fn>).mock.calls;
const input = () => root.querySelector('input.address') as HTMLInputElement;
const btn = (label: string) => root.querySelector(`button[aria-label="${label}"]`) as HTMLButtonElement;

function show(n: object | null = null, secs: object[] = []) {
  workspaces.value = WS;
  activeWorkspace.value = 'ws-a';
  sections.value = secs as never;
  nav.value = n as never;
  render(<Sidebar />, root);
}
const state = (o: Partial<{ url: string; title: string; loading: boolean; canBack: boolean; canForward: boolean }> = {}) => ({
  tab: 'tab-1',
  url: 'https://example.com/page',
  title: 'Example',
  loading: false,
  canBack: false,
  canForward: false,
  ...o,
});

beforeEach(() => {
  root = document.createElement('div');
  document.body.appendChild(root);
  window.m0 = { send: vi.fn(), onmessage: null };
});
afterEach(() => {
  render(null, root);
  root.remove();
  delete window.m0;
  workspaces.value = [];
  activeWorkspace.value = null;
  sections.value = [];
  nav.value = null;
  addressFocus.value = 0;
});

describe('the nav message', () => {
  it('takes the shown tab\'s state', () => {
    handleMessage('nav', JSON.stringify(state({ loading: true, canBack: true })));
    expect(nav.value).toMatchObject({ tab: 'tab-1', url: 'https://example.com/page', loading: true, canBack: true, canForward: false });
  });

  it('ignores malformed nav messages', () => {
    handleMessage('nav', JSON.stringify(state()));
    for (const bad of ['not json', 'null', '{}', JSON.stringify({ ...state(), loading: 'yes' }), JSON.stringify({ ...state(), url: 5 }), JSON.stringify({ tab: 'x' })]) {
      expect(() => handleMessage('nav', bad)).not.toThrow();
    }
    expect(nav.value?.title).toBe('Example');
  });

  it('only accepts PNG data addresses as tab icons', () => {
    const msg = (icon: unknown) => JSON.stringify({ sections: [{ workspace: 'ws-a', selected: null, groups: [], tabs: [{ id: 't', title: '', url: 'https://x.example/', position: 0, group: null, icon }] }] });
    handleMessage('tabs', msg(PNG));
    expect(sections.value[0].tabs[0].icon).toBe(PNG);
    handleMessage('tabs', msg('https://evil.example/icon.png'));
    handleMessage('tabs', msg('javascript:alert(1)'));
    handleMessage('tabs', msg('data:text/html;base64,PGI+'));
    handleMessage('tabs', msg(5));
    expect(sections.value[0].tabs[0].icon).toBe(PNG);
    handleMessage('tabs', msg(null));
    expect(sections.value[0].tabs[0].icon).toBeNull();
  });
});

describe('the address bar', () => {
  it('shows the shown tab\'s address, and nothing for the blank page', async () => {
    show(state());
    expect(input().value).toBe('https://example.com/page');
    nav.value = state({ url: 'about:blank' }) as never;
    await tick();
    expect(input().value).toBe('');
  });

  it('sends what was typed with Enter, and leaves it to the app to decide what it means', async () => {
    show(state());
    input().value = 'rust borrow checker';
    input().dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    input().form!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(sent().at(-1)).toEqual(['nav.go', '{"input":"rust borrow checker"}']);
  });

  it('does not overwrite what is being typed when the page reports a new address', async () => {
    show(state());
    input().value = 'exam';
    input().dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    nav.value = state({ url: 'https://other.example/' }) as never;
    await tick();
    expect(input().value).toBe('exam');
    input().dispatchEvent(new Event('blur'));
    await tick();
    expect(input().value).toBe('https://other.example/');
  });

  it('takes the focus and selects its text when the app asks (Ctrl+L)', async () => {
    show(state());
    const focus = vi.spyOn(input(), 'focus');
    const select = vi.spyOn(input(), 'select');
    handleMessage('focus', '{"target":"address"}');
    await tick();
    expect(addressFocus.value).toBe(1);
    expect(focus).toHaveBeenCalled();
    expect(select).toHaveBeenCalled();
  });

  it('sends nothing for an empty entry', async () => {
    show(state({ url: 'about:blank' }));
    input().form!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(sent().filter(([n]) => n === 'nav.go')).toEqual([]);
  });
});

describe('back, forward, reload and stop', () => {
  it('are commands, enabled only when they can act', async () => {
    show(state({ canBack: true, canForward: false }));
    expect(btn('Back').disabled).toBe(false);
    expect(btn('Forward').disabled).toBe(true);
    btn('Back').click();
    expect(sent().at(-1)).toEqual(['command', '{"id":"nav.back"}']);
    nav.value = state({ canBack: true, canForward: true }) as never;
    await tick();
    btn('Forward').click();
    expect(sent().at(-1)).toEqual(['command', '{"id":"nav.forward"}']);
  });

  it('the third button reloads, and while loading it stops', async () => {
    show(state());
    btn('Reload').click();
    expect(sent().at(-1)).toEqual(['command', '{"id":"nav.reload"}']);
    expect(root.querySelector('.loadbar')).toBeNull();
    nav.value = state({ loading: true }) as never;
    await tick();
    expect(root.querySelector('.loadbar')).not.toBeNull();
    expect(btn('Reload')).toBeNull();
    btn('Stop').click();
    expect(sent().at(-1)).toEqual(['command', '{"id":"nav.stop"}']);
  });

  it('are disabled until the app has said anything about a tab', () => {
    show(null);
    expect(btn('Back').disabled && btn('Forward').disabled && btn('Reload').disabled).toBe(true);
  });
});

describe('tab icons and opening a page in another workspace', () => {
  it('a tab shows its icon when it has one, and a plain placeholder when not', () => {
    show(state(), [{ workspace: 'ws-a', selected: 't1', groups: [], tabs: [tab('t1', 'One', 'https://one.example/', PNG), tab('t2', 'Two', 'https://two.example/')] }]);
    const rows = [...root.querySelectorAll('.tab')];
    expect(rows[0].querySelector('img.fav')?.getAttribute('src')).toBe(PNG);
    expect(rows[1].querySelector('img')).toBeNull();
    expect(rows[1].querySelector('.fav.blank')).not.toBeNull();
  });

  it('offers to open the page in each other workspace, which sends the address to the app', () => {
    show(state());
    const group = root.querySelector('[aria-label="Open this page in"]')!;
    const buttons = [...group.querySelectorAll('button')];
    expect(buttons.map((b) => b.textContent)).toEqual(['→ Personal']);
    buttons[0].click();
    expect(sent().at(-1)).toEqual(['tabs.open_in', '{"workspace":"ws-b","url":"https://example.com/page"}']);
  });

  it('does not offer it for the blank page or when there is no other workspace', async () => {
    show(state({ url: 'about:blank' }));
    expect(root.querySelector('[aria-label="Open this page in"]')).toBeNull();
    workspaces.value = [WS[0]];
    nav.value = state() as never;
    await tick();
    expect(root.querySelector('[aria-label="Open this page in"]')).toBeNull();
  });
});
