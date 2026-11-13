import { render } from 'preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PanelApp } from './PanelApp';
import { Sidebar } from './Sidebar';
import { handleMessage as panelMessage, open, sections } from './panelState';
import { activeWorkspace, handleMessage, sidebarCollapsed, workspaces } from './state';

let root: HTMLElement;
const tick = () => new Promise((r) => setTimeout(r, 0));
const sent = () => (window.m0!.send as ReturnType<typeof vi.fn>).mock.calls;

beforeEach(() => {
  root = document.createElement('div');
  document.body.appendChild(root);
  window.m0 = { send: vi.fn(), onmessage: null };
  workspaces.value = [
    { id: 'ws-a', name: 'Work', colour: 0x2c5fa8, position: 0 },
    { id: 'ws-b', name: 'Personal', colour: 0xa78bfa, position: 1 },
  ];
  activeWorkspace.value = 'ws-a';
  sidebarCollapsed.value = false;
  sections.value = [];
  open.value = true;
});
afterEach(() => {
  render(null, root);
  root.remove();
  delete window.m0;
});

describe('the collapsible sidebar', () => {
  it('has a collapse button that runs the same command as the shortcut', async () => {
    render(<Sidebar />, root);
    await tick();
    (root.querySelector('button[aria-label="Collapse the sidebar"]') as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['command', JSON.stringify({ id: 'sidebar.toggle' })]);
  });

  it('is a rail when the app says so: workspace icons only, no names, tabs or titles', async () => {
    handleMessage('workspaces', JSON.stringify({ workspaces: workspaces.value, active: 'ws-a', sidebar_collapsed: true }));
    render(<Sidebar />, root);
    await tick();
    expect(root.querySelector('nav.rail')).not.toBeNull();
    expect(root.querySelector('nav.sidebar')).toBeNull();
    // the names are only labels of the buttons (for a screen reader and a tooltip), never text on the page
    expect(root.textContent).not.toContain('Work');
    expect(root.textContent).not.toContain('Personal');
    expect(Array.from(root.querySelectorAll('.railws')).map((b) => b.getAttribute('aria-label'))).toEqual(['Work', 'Personal']);
    expect(root.querySelector('.railws.active')!.getAttribute('aria-label')).toBe('Work');
  });

  it('can expand again, switch workspace and make a tab from the rail', async () => {
    handleMessage('workspaces', JSON.stringify({ workspaces: workspaces.value, active: 'ws-a', sidebar_collapsed: true }));
    render(<Sidebar />, root);
    await tick();
    (root.querySelector('button[aria-label="Expand the sidebar"]') as HTMLButtonElement).click();
    (root.querySelector('button[aria-label="Personal"]') as HTMLButtonElement).click();
    (root.querySelector('button[aria-label="New tab"]') as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['command', JSON.stringify({ id: 'sidebar.toggle' })]);
    expect(sent()).toContainEqual(['workspaces.activate', JSON.stringify({ id: 'ws-b' })]);
    expect(sent()).toContainEqual(['command', JSON.stringify({ id: 'tab.new' })]);
  });

  it('follows the app: collapsed state comes only from its message', () => {
    handleMessage('workspaces', JSON.stringify({ workspaces: workspaces.value, active: 'ws-a', sidebar_collapsed: true }));
    expect(sidebarCollapsed.value).toBe(true);
    handleMessage('workspaces', JSON.stringify({ workspaces: workspaces.value, active: 'ws-a' }));
    expect(sidebarCollapsed.value).toBe(false);
  });
});

describe('the collapsible widget panel', () => {
  const panelState = (isOpen: boolean) =>
    JSON.stringify({
      open: isOpen,
      width: 320,
      sections: [{ workspace: { id: 'ws-a', name: 'Work', colour: 0x2c5fa8 }, folded: false, widgets: [{ id: 1, tool: 'jwt', name: 'JWT decoder', description: '', takes_input: true, sensitive: true, collapsed: false }] }],
    });

  it('can be resized by its edge even when it started out collapsed and was expanded later', async () => {
    panelMessage('widgets', panelState(false));
    render(<PanelApp />, root);
    await tick();
    panelMessage('widgets', panelState(true));
    await new Promise((r) => setTimeout(r, 50));
    const grip = root.querySelector('.pgrip') as HTMLElement;
    const ev = (type: string, x: number) => {
      const e = new Event(type, { bubbles: true }) as Event & { screenX: number; pointerId: number };
      Object.assign(e, { screenX: x, pointerId: 1 });
      grip.dispatchEvent(e);
    };
    ev('pointerdown', 500);
    ev('pointermove', 470);
    ev('pointerup', 470);
    expect(sent().filter((c) => c[0] === 'panel.resize')).toEqual([
      ['panel.resize', JSON.stringify({ dx: -30, done: false })],
      ['panel.resize', JSON.stringify({ dx: 0, done: true })],
    ]);
  });

  it('keeps a request for the focus until it has expanded, then takes it', async () => {
    panelMessage('widgets', panelState(false));
    render(<PanelApp />, root);
    await tick();
    // the app asks for the focus while the panel is still a rail, and says it is open a moment later
    panelMessage('panel.focus', '{}');
    await new Promise((r) => setTimeout(r, 30));
    expect(document.activeElement?.tagName).not.toBe('TEXTAREA');
    panelMessage('widgets', panelState(true));
    await new Promise((r) => setTimeout(r, 60));
    expect(document.activeElement?.tagName).toBe('TEXTAREA');
  });

  it('has a collapse button in its header', async () => {
    panelMessage('widgets', panelState(true));
    render(<PanelApp />, root);
    await tick();
    (root.querySelector('button[aria-label="Collapse the widget panel"]') as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['command', JSON.stringify({ id: 'panel.toggle' })]);
  });

  it('is a narrow rail when collapsed, with the way back and a count, and no tool content', async () => {
    panelMessage('widgets', panelState(false));
    render(<PanelApp />, root);
    await tick();
    expect(root.querySelector('.prail')).not.toBeNull();
    expect(root.querySelector('.panel')).toBeNull();
    expect(root.querySelector('textarea')).toBeNull();
    expect(root.querySelector('.prailtext')!.textContent).toContain('(1)');
    (root.querySelector('button[aria-label="Show the widget panel"]') as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['command', JSON.stringify({ id: 'panel.toggle' })]);
  });
});
