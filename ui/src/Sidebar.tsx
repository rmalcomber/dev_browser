import { useEffect, useRef, useState } from 'preact/hooks';
import {
  PALETTE,
  activateWorkspace,
  addressFocus,
  go,
  nav,
  openIn,
  activeWorkspace,
  closeTab,
  collapseGroup,
  createWorkspace,
  css,
  deleteWorkspace,
  expandWorkspace,
  initial,
  isExpanded,
  lastError,
  newWorkspaceRequest,
  openTab,
  pageFocus,
  recolourWorkspace,
  renameGroup,
  renameWorkspace,
  runCommand,
  sectionOf,
  selectTab,
  sidebarFocus,
  tabLabel,
  workspaces,
} from './state';
import type { Group, Section, Tab, Workspace } from './state';

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

const arrow = (open: boolean) => (open ? '▾' : '▸');

/** Up and Down move between the workspace, group and tab buttons; Home and End jump to the first and last; Right and Left expand and
 * collapse the focused workspace or group (without switching to it). */
function navigate(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  if (!target.matches?.('[data-nav]')) return; // typing in a field keeps its own arrow keys
  if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
    const open = e.key === 'ArrowRight';
    const id = target.dataset.id ?? '';
    if (target.dataset.kind === 'ws' && (target.dataset.expanded === 'true') !== open) {
      expandWorkspace(id, open);
      e.preventDefault();
    } else if (target.dataset.kind === 'group' && (target.dataset.collapsed === 'true') === open) {
      collapseGroup(id, !open);
      e.preventDefault();
    }
    return;
  }
  if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(e.key)) return;
  const items = Array.from(document.querySelectorAll<HTMLElement>('[data-nav]'));
  const i = items.indexOf(target);
  if (i < 0) return;
  const next = e.key === 'Home' ? 0 : e.key === 'End' ? items.length - 1 : e.key === 'ArrowDown' ? Math.min(i + 1, items.length - 1) : Math.max(i - 1, 0);
  items[next].focus();
  e.preventDefault();
}

function TabRow(props: { tab: Tab; workspace: string; active: boolean; selected: boolean }) {
  const { tab, workspace, active, selected } = props;
  return (
    <li class="tabrow">
      <button
        type="button"
        data-nav
        data-kind="tab"
        class={'tab' + (selected ? ' active' : '')}
        aria-current={selected ? 'true' : undefined}
        title={tab.url}
        onClick={() => {
          if (!active) openTab(workspace, tab.id);
          else if (!selected) selectTab(tab.id);
        }}
      >
        {tab.icon ? <img class="fav" src={tab.icon} alt="" width="16" height="16" /> : <span class="fav blank" aria-hidden="true" />}
        <span class="name">{tabLabel(tab)}</span>
      </button>
      {active && (
        <button type="button" class="x" aria-label={`Close tab ${tabLabel(tab)}`} onClick={() => closeTab(tab.id)}>
          {'×'}
        </button>
      )}
    </li>
  );
}

function GroupBlock(props: { group: Group; tabs: Tab[]; section: Section; active: boolean; renaming: boolean; setRenaming: (id: string | null) => void }) {
  const { group, tabs, section, active, renaming, setRenaming } = props;
  return (
    <li class="group">
      <div class="grouprow">
        <button
          type="button"
          class="chev"
          tabIndex={-1}
          aria-label={`${group.collapsed ? 'Expand' : 'Collapse'} group ${group.name}`}
          aria-expanded={!group.collapsed}
          disabled={!active}
          onClick={() => collapseGroup(group.id, !group.collapsed)}
        >
          {arrow(!group.collapsed)}
        </button>
        {renaming ? (
          <NameForm
            initial={group.name}
            label="Group name"
            onSave={(name) => {
              renameGroup(group.id, name);
              setRenaming(null);
            }}
            onCancel={() => setRenaming(null)}
          />
        ) : active ? (
          <button
            type="button"
            data-nav
            data-kind="group"
            data-id={group.id}
            data-collapsed={group.collapsed}
            class="groupname"
            title="Click or press F2 to rename"
            onClick={() => setRenaming(group.id)}
            onKeyDown={(e) => e.key === 'F2' && setRenaming(group.id)}
          >
            {group.name}
          </button>
        ) : (
          <span class="groupname">{group.name}</span>
        )}
      </div>
      {!group.collapsed && (
        <ul class="tabs nested">
          {tabs.map((t) => (
            <TabRow tab={t} workspace={section.workspace} active={active} selected={t.id === section.selected} />
          ))}
        </ul>
      )}
    </li>
  );
}

/** The tabs and groups of one expanded workspace. */
function TabTree(props: { section: Section; active: boolean; renamingGroup: string | null; setRenamingGroup: (id: string | null) => void }) {
  const { section, active } = props;
  const loose = section.tabs.filter((t) => t.group === null);
  return (
    <ul class="tabs" role="group">
      {loose.map((t) => (
        <TabRow tab={t} workspace={section.workspace} active={active} selected={t.id === section.selected} />
      ))}
      {section.groups.map((g) => (
        <GroupBlock
          group={g}
          tabs={section.tabs.filter((t) => t.group === g.id)}
          section={section}
          active={active}
          renaming={props.renamingGroup === g.id}
          setRenaming={props.setRenamingGroup}
        />
      ))}
    </ul>
  );
}

/**
 * One workspace in the tree. Collapsed, it is a chevron, an icon and the name, and nothing else: no count, no title, no address (S11).
 * The collapsed row is built only from the workspace record; tab data is read only when the app has sent a section for it.
 */
export function WorkspaceItem(props: { w: Workspace; renamingGroup: string | null; setRenamingGroup: (id: string | null) => void }) {
  const { w } = props;
  const expanded = isExpanded(w.id);
  const active = w.id === activeWorkspace.value;
  const section = sectionOf(w.id);
  return (
    <li class="wsitem">
      <div class="wsrow">
        <button
          type="button"
          class="chev"
          tabIndex={-1}
          aria-label={`${expanded ? 'Collapse' : 'Expand'} ${w.name}`}
          aria-expanded={expanded}
          onClick={() => expandWorkspace(w.id, !expanded)}
        >
          {arrow(expanded)}
        </button>
        <button
          type="button"
          data-nav
          data-kind="ws"
          data-id={w.id}
          data-expanded={expanded}
          class={'ws' + (active ? ' active' : '')}
          aria-current={active ? 'true' : undefined}
          onClick={() => !active && activateWorkspace(w.id)}
        >
          <span class="icon" style={{ background: css(w.colour) }} aria-hidden="true">
            {initial(w.name)}
          </span>
          <span class="name">{w.name}</span>
        </button>
      </div>
      {expanded && section && <TabTree section={section} active={active} renamingGroup={props.renamingGroup} setRenamingGroup={props.setRenamingGroup} />}
    </li>
  );
}

/** Back, forward, reload or stop, and the address bar (search or address, decided by the app). */
function NavBar() {
  const n = nav.value;
  const field = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useState<string | null>(null);
  // Ctrl+L: the app asked for the address bar
  useEffect(() => {
    if (addressFocus.value === 0) return;
    field.current?.focus();
    field.current?.select();
  }, [addressFocus.value]);
  const shown = draft ?? (n && n.url !== 'about:blank' ? n.url : '');
  const loading = !!n?.loading;
  return (
    <div class="navbar">
      <div class="navbtns" role="group" aria-label="Navigation">
        <button type="button" aria-label="Back" title="Back (Alt+Left)" disabled={!n?.canBack} onClick={() => runCommand('nav.back')}>
          {'\u25C2'}
        </button>
        <button type="button" aria-label="Forward" title="Forward (Alt+Right)" disabled={!n?.canForward} onClick={() => runCommand('nav.forward')}>
          {'\u25B8'}
        </button>
        <button
          type="button"
          aria-label={loading ? 'Stop' : 'Reload'}
          title={loading ? 'Stop (Ctrl+.)' : 'Reload (F5)'}
          disabled={!n}
          onClick={() => runCommand(loading ? 'nav.stop' : 'nav.reload')}
        >
          {loading ? '\u2715' : '\u27F3'}
        </button>
      </div>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          const text = draft ?? shown;
          if (text.trim()) go(text);
          setDraft(null);
        }}
      >
        <input
          ref={field}
          class="address"
          value={shown}
          placeholder="Search or type an address"
          aria-label="Address"
          spellcheck={false}
          autocomplete="off"
          onInput={(e) => setDraft((e.target as HTMLInputElement).value)}
          onFocus={(e) => (e.target as HTMLInputElement).select()}
          onBlur={() => setDraft(null)}
        />
      </form>
      {loading && <div class="loadbar" role="progressbar" aria-label="Loading" />}
    </div>
  );
}

export function Sidebar() {
  const [mode, setMode] = useState<Mode>('none');
  const [newColour, setNewColour] = useState(PALETTE[0]);
  const [renamingGroup, setRenamingGroup] = useState<string | null>(null);
  const list = workspaces.value;
  const active = list.find((w) => w.id === activeWorkspace.value);
  const close = () => setMode('none');

  // the app moved focus out to the page (Esc, F6): open forms are abandoned, not left half-typed
  useEffect(() => {
    if (pageFocus.value > 0) {
      setMode('none');
      setRenamingGroup(null);
    }
  }, [pageFocus.value]);
  // the app moved focus into the sidebar (F6): land on the selected tab, else the active workspace
  useEffect(() => {
    if (sidebarFocus.value === 0) return;
    const el = document.querySelector<HTMLElement>('.tab[aria-current="true"]') ?? document.querySelector<HTMLElement>('.ws[aria-current="true"]');
    el?.focus();
  }, [sidebarFocus.value]);
  // the "new workspace" command (Alt+N): open the name field
  useEffect(() => {
    if (newWorkspaceRequest.value === 0) return;
    setNewColour(PALETTE[list.length % PALETTE.length]);
    setMode('new');
  }, [newWorkspaceRequest.value]);

  return (
    <nav class="sidebar" aria-label="Workspaces and tabs" onKeyDown={navigate}>
      <h1>
        The Dev Browser <small>alpha</small>
      </h1>

      <NavBar />

      <ul class="tree" role="tree">
        {list.map((w) => (
          <WorkspaceItem w={w} renamingGroup={renamingGroup} setRenamingGroup={setRenamingGroup} />
        ))}
      </ul>

      <div class="footer">
        {lastError.value && (
          <p class="error" role="alert">
            {lastError.value}
          </p>
        )}

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
        ) : mode === 'rename' && active ? (
          <NameForm
            initial={active.name}
            label="Workspace name"
            onSave={(name) => {
              renameWorkspace(active.id, name);
              close();
            }}
            onCancel={close}
          />
        ) : mode === 'colour' && active ? (
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
        ) : mode === 'delete' && active ? (
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
          <>
            <div class="row">
              <button type="button" onClick={() => runCommand('tab.new')}>
                + Tab
              </button>
              <button type="button" onClick={() => runCommand('tab.group.new')}>
                Group
              </button>
              <button type="button" onClick={() => runCommand('tab.ungroup')}>
                Ungroup
              </button>
            </div>
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
            {list.length > 1 && nav.value && /^https?:/i.test(nav.value.url) && (
              <div class="row" role="group" aria-label="Open this page in">
                {list
                  .filter((w) => w.id !== activeWorkspace.value)
                  .map((w) => (
                    <button type="button" class="openin" title={'Open this page in ' + w.name + ' (its own logins)'} onClick={() => openIn(w.id, nav.value!.url)}>
                      {'\u2192 ' + w.name}
                    </button>
                  ))}
              </div>
            )}
            {active && (
              <div class="row" role="group" aria-label={`Manage ${active.name}`}>
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
          </>
        )}
      </div>
    </nav>
  );
}
