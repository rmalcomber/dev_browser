import { render } from 'preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Launcher } from './Launcher';
import { handleMessage, results, scope, selection, tool, toolError, toolInput, toolOutput } from './launcherState';

let root: HTMLElement;
const tick = () => new Promise((r) => setTimeout(r, 0));
const sent = () => (window.m0!.send as ReturnType<typeof vi.fn>).mock.calls;

const opened = (o: object = {}) =>
  JSON.stringify({
    tool: 'jwt',
    name: 'JWT decoder',
    description: 'Decode a token.',
    takes_input: true,
    sensitive: true,
    input: '',
    from_selection: false,
    ...o,
  });
const output = (fields: object[], notes: string[] = [], t = 'jwt') => JSON.stringify({ tool: t, output: { fields, notes }, error: null });
const field = (label: string, value: string, block = false) => ({ label, value, block });

beforeEach(() => {
  root = document.createElement('div');
  document.body.appendChild(root);
  window.m0 = { send: vi.fn(), onmessage: null };
  results.value = [];
  scope.value = { kind: 'contextual', workspaces: [] };
  tool.value = null;
  toolInput.value = '';
  toolOutput.value = null;
  toolError.value = null;
  selection.value = null;
});
afterEach(() => {
  render(null, root);
  root.remove();
  delete window.m0;
});

describe('a tool inside the launcher', () => {
  it('opens with the selected text as its input and shows the labelled values', async () => {
    handleMessage('launcher.selection', JSON.stringify({ length: 9, preview: 'eyJhbGci' }));
    handleMessage('launcher.tool.opened', opened({ input: 'eyJhbGci.x.y', from_selection: true }));
    handleMessage('launcher.tool.output', output([field('sub', 'alice'), field('Payload', '{\n  "sub": "alice"\n}', true)], ['The signature is not verified.']));
    render(<Launcher />, root);
    await tick();
    expect((root.querySelector('textarea') as HTMLTextAreaElement).value).toBe('eyJhbGci.x.y');
    expect(root.querySelector('.tsource')!.textContent).toContain('selected on the page');
    expect(root.querySelector('.tsecret')!.textContent).toContain('not stored');
    expect(Array.from(root.querySelectorAll('.tlabel')).map((e) => e.textContent)).toEqual(['sub', 'Payload']);
    expect(root.querySelector('.tnote')!.textContent).toContain('not verified');
    expect(root.querySelector('input.lquery')).toBeNull();
  });

  it('sends only the text when the input changes', async () => {
    handleMessage('launcher.tool.opened', opened());
    render(<Launcher />, root);
    await tick();
    const box = root.querySelector('textarea')!;
    box.value = 'a.b.c';
    box.dispatchEvent(new Event('input', { bubbles: true }));
    expect(sent()).toContainEqual(['launcher.tool', JSON.stringify({ tool: 'jwt', input: 'a.b.c' })]);
  });

  it('copies exactly the value of the field whose button was pressed', async () => {
    const write = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: write }, configurable: true });
    handleMessage('launcher.tool.opened', opened());
    handleMessage('launcher.tool.output', output([field('sub', 'alice'), field('iss', 'https://issuer.example')]));
    render(<Launcher />, root);
    await tick();
    (root.querySelector('button[aria-label="Copy iss"]') as HTMLButtonElement).click();
    await tick();
    expect(write).toHaveBeenCalledTimes(1);
    expect(write).toHaveBeenCalledWith('https://issuer.example');
    expect(root.querySelector('button[aria-label="Copy iss"]')!.textContent).toBe('Copied');
  });

  it('shows values and messages as text, never as markup', async () => {
    handleMessage('launcher.tool.opened', opened());
    handleMessage('launcher.tool.output', output([field('name', '<img src=x onerror=alert(1)>')], ['<b>bold</b>']));
    render(<Launcher />, root);
    await tick();
    expect(root.querySelector('.toutput img, .toutput b')).toBeNull();
    expect(root.querySelector('.tvalue')!.textContent).toBe('<img src=x onerror=alert(1)>');
  });

  it('shows an error from the app, and ignores output for a tool that is not open', async () => {
    handleMessage('launcher.tool.opened', opened());
    handleMessage('launcher.tool.output', JSON.stringify({ tool: 'jwt', output: null, error: 'the payload is not JSON' }));
    render(<Launcher />, root);
    await tick();
    expect(root.querySelector('.terror')!.textContent).toBe('the payload is not JSON');
    handleMessage('launcher.tool.output', output([field('x', 'y')], [], 'json'));
    await tick();
    expect(root.querySelector('.toutput')).toBeNull();
  });

  it('has a generate button for a tool that reads no input', async () => {
    handleMessage('launcher.tool.opened', opened({ tool: 'uuid', name: 'UUID generator', takes_input: false, sensitive: false }));
    render(<Launcher />, root);
    await tick();
    expect(root.querySelector('textarea')).toBeNull();
    (root.querySelector('.trun') as HTMLButtonElement).click();
    expect(sent()).toContainEqual(['launcher.tool', JSON.stringify({ tool: 'uuid', input: '' })]);
  });

  it('goes back to searching and drops the input', async () => {
    handleMessage('launcher.tool.opened', opened({ input: 'secret-token' }));
    render(<Launcher />, root);
    await tick();
    (root.querySelector('button[aria-label="Back to search"]') as HTMLButtonElement).click();
    await tick();
    expect(tool.value).toBeNull();
    expect(toolInput.value).toBe('');
    expect(toolOutput.value).toBeNull();
    expect(root.querySelector('input.lquery')).not.toBeNull();
    expect(root.textContent).not.toContain('secret-token');
    expect(sent().some((c) => c[0] === 'launcher.search')).toBe(true);
  });

  it('forgets everything when the app opens the launcher again', () => {
    handleMessage('launcher.tool.opened', opened({ input: 'secret-token' }));
    handleMessage('launcher.selection', JSON.stringify({ length: 3, preview: 'abc' }));
    handleMessage('launcher.opened', '{}');
    expect(tool.value).toBeNull();
    expect(toolInput.value).toBe('');
    expect(selection.value).toBeNull();
  });

  it('shows what is selected on the page, and nothing when nothing is', async () => {
    handleMessage('launcher.results', JSON.stringify({ scope: { kind: 'contextual', workspaces: [] }, all: [], results: [], selection: { length: 120, preview: 'Bearer eyJhbGci' } }));
    render(<Launcher />, root);
    await tick();
    expect(root.querySelector('.lselection')!.textContent).toContain('120 characters');
    handleMessage('launcher.selection', 'null');
    await tick();
    expect(root.querySelector('.lselection')).toBeNull();
  });

  it('marks tools in the list, and a tool that suits the selection', async () => {
    handleMessage(
      'launcher.results',
      JSON.stringify({
        scope: { kind: 'contextual', workspaces: [] },
        all: [],
        results: [{ id: 'tool/jwt', kind: 'tool', title: 'JWT decoder', subtitle: 'Decode a token.', workspace: null, score: 50, tag: 'selection' }],
      }),
    );
    render(<Launcher />, root);
    await tick();
    expect(root.querySelector('.lkind[title="tool"]')).not.toBeNull();
    expect(root.querySelector('.ltag.selection')).not.toBeNull();
  });
});
