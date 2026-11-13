import { useEffect, useRef, useState } from 'preact/hooks';
import {
  PALETTE,
  activateWorkspace,
  activeWorkspace,
  createWorkspace,
  css,
  deleteWorkspace,
  lastError,
  recolourWorkspace,
  renameWorkspace,
  workspaces,
} from './state';

type Mode = 'none' | 'new' | 'rename' | 'colour' | 'delete';

function Swatches(props: { value: number; onPick: (colour: number) => void }) {
  return (
    <div class="swatches" role="group" aria-label="Colour">
      {PALETTE.map((c) => (
        <button
          type="button"
          class={'swatch' + (c === props.value ? ' on' : '')}
          style={{ background: css(c) }}
          aria-label={css(c)}
          aria-pressed={c === props.value}
          onClick={() => props.onPick(c)}
        />
      ))}
    </div>
  );
}

/** A name field: Enter saves, Escape cancels. Focused when it appears so the keyboard goes straight to it. */
function NameForm(props: { initial: string; label: string; onSave: (name: string) => void; onCancel: () => void; children?: any }) {
  const input = useRef<HTMLInputElement>(null);
  const [name, setName] = useState(props.initial);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  return (
    <form
      class="form"
      onSubmit={(e) => {
        e.preventDefault();
        props.onSave(name);
      }}
      onKeyDown={(e) => {
        if (e.key === 'Escape') props.onCancel();
      }}
    >
      <input
        ref={input}
        value={name}
        maxLength={64}
        aria-label={props.label}
        onInput={(e) => setName((e.target as HTMLInputElement).value)}
      />
      {props.children}
      <div class="row">
        <button type="submit">Save</button>
        <button type="button" onClick={props.onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}

export function Sidebar() {
  const [mode, setMode] = useState<Mode>('none');
  const [newColour, setNewColour] = useState(PALETTE[0]);
  const list = workspaces.value;
  const active = list.find((w) => w.id === activeWorkspace.value);
  const close = () => setMode('none');

  return (
    <nav class="sidebar" aria-label="Workspaces">
      <h1>
        The Dev Browser <small>alpha</small>
      </h1>

      <ul class="list">
        {list.map((w) => (
          <li>
            <button
              type="button"
              class={'ws' + (w.id === activeWorkspace.value ? ' active' : '')}
              aria-current={w.id === activeWorkspace.value ? 'true' : undefined}
              onClick={() => w.id !== activeWorkspace.value && activateWorkspace(w.id)}
            >
              <span class="dot" style={{ background: css(w.colour) }} />
              <span class="name">{w.name}</span>
            </button>
          </li>
        ))}
      </ul>

      {mode === 'new' ? (
        <NameForm
          initial=""
          label="New workspace name"
          onSave={(name) => {
            createWorkspace(name, newColour);
            close();
          }}
          onCancel={close}
        >
          <Swatches value={newColour} onPick={setNewColour} />
        </NameForm>
      ) : (
        <button
          type="button"
          class="add"
          onClick={() => {
            setNewColour(PALETTE[list.length % PALETTE.length]);
            setMode('new');
          }}
        >
          + New workspace
        </button>
      )}

      {active && mode !== 'new' && (
        <section class="manage" aria-label={`Manage ${active.name}`}>
          {mode === 'rename' ? (
            <NameForm
              initial={active.name}
              label="Workspace name"
              onSave={(name) => {
                renameWorkspace(active.id, name);
                close();
              }}
              onCancel={close}
            />
          ) : mode === 'colour' ? (
            <div class="form" onKeyDown={(e) => e.key === 'Escape' && close()}>
              <Swatches
                value={active.colour}
                onPick={(c) => {
                  recolourWorkspace(active.id, c);
                  close();
                }}
              />
              <button type="button" onClick={close}>
                Cancel
              </button>
            </div>
          ) : mode === 'delete' ? (
            <div class="form" role="alertdialog" aria-label="Confirm delete" onKeyDown={(e) => e.key === 'Escape' && close()}>
              <p>
                Delete <b>{active.name}</b> and everything stored in it (logins, cookies, tabs)? This cannot be undone.
              </p>
              <div class="row">
                <button
                  type="button"
                  class="danger"
                  onClick={() => {
                    deleteWorkspace(active.id);
                    close();
                  }}
                >
                  Delete
                </button>
                <button type="button" autofocus onClick={close}>
                  Keep
                </button>
              </div>
            </div>
          ) : (
            <div class="row">
              <button type="button" onClick={() => setMode('rename')}>
                Rename
              </button>
              <button type="button" onClick={() => setMode('colour')}>
                Colour
              </button>
              <button type="button" onClick={() => setMode('delete')}>
                Delete
              </button>
            </div>
          )}
        </section>
      )}

      {lastError.value && (
        <p class="error" role="alert">
          {lastError.value}
        </p>
      )}
    </nav>
  );
}
