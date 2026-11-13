import { render } from 'preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Sidebar } from './Sidebar';
import { activeWorkspace, handleMessage, MAX_NOTICES, notices, offers, workspaces } from './state';

let root: HTMLElement;
const tick = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  notices.value = [];
  offers.value = [];
  workspaces.value = [{ id: 'ws-a', name: 'Work', colour: 0x2c5fa8, position: 0 }];
  activeWorkspace.value = 'ws-a';
  root = document.createElement('div');
  document.body.appendChild(root);
});
afterEach(() => {
  render(null, root);
  root.remove();
  vi.restoreAllMocks();
});

describe('notices (M1.6)', () => {
  it('shows a notice as plain text, never as markup', async () => {
    handleMessage('notice', JSON.stringify({ text: '<img src=x onerror=alert(1)> asked for a permission' }));
    render(<Sidebar />, root);
    await tick();
    expect(root.querySelector('.notice img')).toBeNull();
    expect(root.querySelector('.notice')!.textContent).toContain('<img src=x');
  });

  it('keeps only the newest few and can dismiss one', async () => {
    for (let i = 0; i < MAX_NOTICES + 3; i++) handleMessage('notice', JSON.stringify({ text: `n${i}` }));
    expect(notices.value.length).toBe(MAX_NOTICES);
    expect(notices.value.at(-1)).toBe(`n${MAX_NOTICES + 2}`);
    render(<Sidebar />, root);
    await tick();
    root.querySelector<HTMLButtonElement>('.notice button')!.click();
    await tick();
    expect(notices.value.length).toBe(MAX_NOTICES - 1);
  });

  it('ignores malformed notices and offers', () => {
    handleMessage('notice', '{"text":5}');
    handleMessage('notice', 'not json');
    handleMessage('download', '{"id":"7","name":"x"}');
    handleMessage('download', '{"id":1.5,"name":"x"}');
    expect(notices.value).toEqual([]);
    expect(offers.value).toEqual([]);
  });
});

describe('download offers (M1.6)', () => {
  it('lists an offer and sends only an id and save or cancel', async () => {
    const send = vi.fn();
    (window as unknown as { m0: object }).m0 = { send, onmessage: null };
    handleMessage('download', JSON.stringify({ id: 3, name: 'report.pdf' }));
    render(<Sidebar />, root);
    await tick();
    expect(root.querySelector('.offer')!.textContent).toContain('report.pdf');
    const [save, cancel] = Array.from(root.querySelectorAll<HTMLButtonElement>('.offer button'));
    save.click();
    cancel.click();
    expect(send).toHaveBeenCalledWith('download.decide', JSON.stringify({ id: 3, save: true }));
    expect(send).toHaveBeenCalledWith('download.decide', JSON.stringify({ id: 3, save: false }));
  });

  it('drops an offer when the app says it is done', () => {
    handleMessage('download', JSON.stringify({ id: 3, name: 'a' }));
    handleMessage('download', JSON.stringify({ id: 4, name: 'b' }));
    handleMessage('download.done', JSON.stringify({ id: 3 }));
    expect(offers.value.map((o) => o.id)).toEqual([4]);
  });
});
