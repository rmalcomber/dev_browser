import { render } from 'preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Sidebar } from './Sidebar';
import { activeWorkspace, handleMessage, lastError, sections, workspaces } from './state';

const WS = [
  { id: 'ws-a', name: 'Work', colour: 0x2c5fa8, position: 0 },
  { id: 'ws-b', name: 'Personal', colour: 0xa78bfa, position: 1 },
  { id: 'ws-c', name: 'Client X', colour: 0x2ecc71, position: 2 },
];

// Titles and addresses that must never be visible for a collapsed workspace.
const SECRET_TITLES = ['Quarterly salary review', 'Divorce lawyer - contact', 'Acme layoffs plan'];
const SECRET_URLS = ['hr.work.example', 'lawyer.private.example', 'acme.client.example'];

const tab = (id: string, title: string, url: string, group: string | null = null) => ({ id, title, url, position: 0, group });
const secret = (workspace: string, i: number, extra: object = {}) => ({
  workspace,
  selected: `tab-${i}`,
  groups: [],
  tabs: [tab(`tab-${i}`, SECRET_TITLES[i], `https://${SECRET_URLS[i]}/page`)],
  ...extra,
});

let root: HTMLElement;
const tick = () => new Promise((r) => setTimeout(r, 0));

function show(secs: object[]) {
  workspaces.value = WS;
  activeWorkspace.value = 'ws-a';
  sections.value = secs as never;
  render(<Sidebar />, root);
}
const row = (name: string) => [...root.querySelectorAll('.wsitem')].find((li) => li.querySelector('.ws .name')?.textContent === name) as HTMLElement;

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
  lastError.value = null;
});

describe('S11: a collapsed workspace is only a name and an icon', () => {
  it('shows nothing but the icon letter and the name for collapsed workspaces', () => {
    show([secret('ws-a', 0)]); // only Work is expanded
    for (const [name, letter] of [['Personal', 'P'], ['Client X', 'C']]) {
      const li = row(name);
      expect(li, name).toBeTruthy();
      expect(li.textContent?.replace(/[\u25B8\u25BE\s]/g, '')).toBe(letter + name.replace(/\s/g, ''));
      expect(li.querySelector('.tabs, ul, li li'), 'no nested list').toBeNull();
      // no tooltips, counts, badges or thumbnails: the only elements are the chevron, the name button, its icon and its name
      expect(li.querySelectorAll('[title]').length).toBe(0);
      expect(li.querySelectorAll('img, svg, canvas, .badge, .count').length).toBe(0);
      expect([...li.querySelectorAll('*')].map((e) => e.tagName + '.' + e.className)).toEqual(['DIV.wsrow', 'BUTTON.chev', 'BUTTON.ws', 'SPAN.icon', 'SPAN.name']);
    }
  });

  it('the page as a whole contains none of the collapsed workspaces\' titles or addresses, in text or in attributes', () => {
    show([secret('ws-a', 0)]);
    const html = root.innerHTML;
    for (const s of [SECRET_TITLES[1], SECRET_TITLES[2], SECRET_URLS[1], SECRET_URLS[2]]) expect(html).not.toContain(s);
    // the expanded one does show its own
    expect(html).toContain(SECRET_TITLES[0]);
  });

  it('even if the app wrongly sent a section for a collapsed workspace, only expanded workspaces are drawn from sections', () => {
    // the sections are the expanded set: a section present means expanded. Data for a workspace without a section cannot be shown.
    show([secret('ws-a', 0)]);
    const html = root.innerHTML;
    expect(html).not.toContain(SECRET_TITLES[1]);
    expect(row('Personal').querySelector('.chev')?.getAttribute('aria-expanded')).toBe('false');
    expect(row('Work').querySelector('.chev')?.getAttribute('aria-expanded')).toBe('true');
  });

  it('collapsing the active workspace hides its tabs too', async () => {
    show([secret('ws-a', 0)]);
    expect(root.innerHTML).toContain(SECRET_TITLES[0]);
    sections.value = [];
    await tick();
    expect(root.innerHTML).not.toContain(SECRET_TITLES[0]);
    expect(row('Work').textContent?.replace(/[\u25B8\u25BE\s]/g, '')).toBe('WWork');
  });

  it('expanding another workspace reveals its names and only its own', async () => {
    show([secret('ws-a', 0)]);
    sections.value = [secret('ws-a', 0), secret('ws-b', 1)] as never;
    await tick();
    expect(root.innerHTML).toContain(SECRET_TITLES[1]);
    expect(root.innerHTML).not.toContain(SECRET_TITLES[2]);
  });

  it('aria labels carry only the workspace name', () => {
    show([secret('ws-a', 0)]);
    const labels = [...row('Client X').querySelectorAll('[aria-label]')].map((e) => e.getAttribute('aria-label'));
    expect(labels).toEqual(['Expand Client X']);
  });
});

describe('the sidebar', () => {
  it('the chevron expands without switching; the name switches', () => {
    show([secret('ws-a', 0)]);
    const send = window.m0!.send as ReturnType<typeof vi.fn>;
    (row('Personal').querySelector('.chev') as HTMLElement).click();
    expect(send).toHaveBeenLastCalledWith('workspaces.expand', '{"id":"ws-b","expanded":true}');
    (row('Personal').querySelector('.ws') as HTMLElement).click();
    expect(send).toHaveBeenLastCalledWith('workspaces.activate', '{"id":"ws-b"}');
    (row('Work').querySelector('.chev') as HTMLElement).click();
    expect(send).toHaveBeenLastCalledWith('workspaces.expand', '{"id":"ws-a","expanded":false}');
  });

  it('a tab of the active workspace selects; a tab of another expanded workspace opens it there', async () => {
    show([secret('ws-a', 0, { tabs: [tab('tab-x', 'One', 'https://one.example/'), tab('tab-y', 'Two', 'https://two.example/')], selected: 'tab-x' }), secret('ws-b', 1)]);
    await tick();
    const send = window.m0!.send as ReturnType<typeof vi.fn>;
    const btns = [...row('Work').querySelectorAll('.tab')] as HTMLElement[];
    expect(btns.map((b) => b.textContent)).toEqual(['One', 'Two']);
    btns[1].click();
    expect(send).toHaveBeenLastCalledWith('tabs.select', '{"id":"tab-y"}');
    btns[0].click(); // already selected: nothing sent
    expect(send).toHaveBeenCalledTimes(1);
    (row('Personal').querySelector('.tab') as HTMLElement).click();
    expect(send).toHaveBeenLastCalledWith('tabs.open', '{"workspace":"ws-b","id":"tab-1"}');
    // close buttons exist only on the active workspace's tabs
    expect(row('Work').querySelectorAll('.x').length).toBe(2);
    expect(row('Personal').querySelectorAll('.x').length).toBe(0);
  });

  it('draws groups after the ungrouped tabs, with a collapsed group hiding its tabs', () => {
    show([
      {
        workspace: 'ws-a',
        selected: 't1',
        groups: [{ id: 'g1', name: 'Reading', position: 0, collapsed: false }, { id: 'g2', name: 'Later', position: 1, collapsed: true }],
        tabs: [tab('t1', 'Loose', 'https://l.example/'), tab('t2', 'In reading', 'https://r.example/', 'g1'), tab('t3', 'In later', 'https://x.example/', 'g2')],
      },
    ]);
    const text = row('Work').textContent!;
    expect(text.indexOf('Loose')).toBeLessThan(text.indexOf('Reading'));
    expect(text.indexOf('Reading')).toBeLessThan(text.indexOf('In reading'));
    expect(text).toContain('Later');
    expect(text).not.toContain('In later');
  });

  it('only the active workspace\'s groups can be renamed or collapsed from the sidebar', () => {
    show([
      { workspace: 'ws-a', selected: null, groups: [{ id: 'g1', name: 'Mine', position: 0, collapsed: false }], tabs: [tab('t1', 'A', 'https://a.example/', 'g1')] },
      { workspace: 'ws-b', selected: null, groups: [{ id: 'g2', name: 'Theirs', position: 0, collapsed: false }], tabs: [tab('t2', 'B', 'https://b.example/', 'g2')] },
    ]);
    expect(row('Work').querySelector('button.groupname')).toBeTruthy();
    expect(row('Personal').querySelector('button.groupname')).toBeNull();
    expect((row('Personal').querySelector('.grouprow .chev') as HTMLButtonElement).disabled).toBe(true);
  });

  it('shows the app\'s error message', async () => {
    show([secret('ws-a', 0)]);
    handleMessage('error', JSON.stringify({ request: 'workspaces.delete', message: 'you cannot delete your only workspace' }));
    await tick();
    expect(root.querySelector('.error')?.textContent).toBe('you cannot delete your only workspace');
  });
});
