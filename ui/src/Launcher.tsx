import { useEffect, useRef } from 'preact/hooks';
import { css } from './state';
import {
  allWorkspaces,
  close,
  drag,
  opened,
  query,
  requestScope,
  results,
  run,
  scope,
  search,
  selected,
} from './launcherState';

/** The launcher: a search box over what the app says is in scope, and the way to widen that scope on purpose. */
export function Launcher() {
  const box = useRef<HTMLInputElement>(null);

  // the app opened the launcher: take the focus with an empty box
  useEffect(() => {
    if (opened.value > 0) box.current?.focus();
  }, [opened.value]);

  // keep the selected result in view
  useEffect(() => {
    document.querySelector('.result[aria-selected="true"]')?.scrollIntoView?.({ block: 'nearest' });
  }, [selected.value, results.value]);

  const list = results.value;
  const names = allWorkspaces.value;

  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selected.value = Math.min(selected.value + 1, Math.max(list.length - 1, 0));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selected.value = Math.max(selected.value - 1, 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const r = list[selected.value];
      if (r) run(r.id);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }

  // dragging the title bar moves the overlay; the app clamps it to the window
  const last = useRef<{ x: number; y: number } | null>(null);
  const onMove = (e: MouseEvent) => {
    if (!last.current) return;
    const dx = e.screenX - last.current.x;
    const dy = e.screenY - last.current.y;
    if (dx || dy) drag(dx, dy);
    last.current = { x: e.screenX, y: e.screenY };
  };
  const onUp = () => {
    last.current = null;
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onUp);
  };
  const onDown = (e: MouseEvent) => {
    last.current = { x: e.screenX, y: e.screenY };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  };

  const s = scope.value;
  const chosen = (id: string) => s.workspaces.includes(id);
  const toggle = (id: string) => {
    const now = s.kind === 'selected' ? s.workspaces : [];
    requestScope('selected', now.includes(id) ? now.filter((w) => w !== id) : [...now, id]);
  };
  const label =
    s.kind === 'all'
      ? 'Searching every workspace'
      : s.kind === 'selected'
        ? `Searching ${s.workspaces.length} chosen workspace${s.workspaces.length === 1 ? '' : 's'}`
        : 'Searching this workspace';

  return (
    <div class="launcher" role="dialog" aria-label="Launcher">
      <div class="lbar" onMouseDown={onDown} title="Drag to move">
        <span class="lgrip" aria-hidden="true">
          ⠿
        </span>
        <span class="lscope" role="status">
          {label}
        </span>
      </div>
      <input
        ref={box}
        class="lquery"
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls="results"
        aria-label="Search tabs and commands"
        placeholder="Search tabs and commands"
        autocomplete="off"
        spellcheck={false}
        value={query.value}
        onInput={(e) => search((e.currentTarget as HTMLInputElement).value)}
        onKeyDown={onKey}
      />
      <ul id="results" class="lresults" role="listbox">
        {list.map((r, i) => (
          <li
            class="result"
            role="option"
            aria-selected={i === selected.value}
            onMouseMove={() => (selected.value = i)}
            onClick={() => run(r.id)}
          >
            {r.workspace ? (
              <span class="ldot" style={{ background: css(r.workspace.colour) }} title={r.workspace.name} aria-label={r.workspace.name} />
            ) : (
              <span class="ldot cmd" aria-hidden="true">
                &gt;
              </span>
            )}
            <span class="ltitle">{r.title}</span>
            <span class="lsub">{r.subtitle}</span>
          </li>
        ))}
        {list.length === 0 && <li class="lempty">Nothing matches in this scope.</li>}
      </ul>
      <div class="lscopes" role="group" aria-label="Search scope">
        <button type="button" aria-pressed={s.kind === 'contextual'} onClick={() => requestScope('contextual')}>
          This workspace
        </button>
        <button type="button" aria-pressed={s.kind === 'all'} onClick={() => requestScope('all')}>
          All workspaces
        </button>
        {names.map((w) => (
          <button type="button" class="lws" aria-pressed={s.kind === 'selected' && chosen(w.id)} onClick={() => toggle(w.id)}>
            <span class="ldot" style={{ background: css(w.colour) }} aria-hidden="true" /> {w.name}
          </button>
        ))}
      </div>
    </div>
  );
}
