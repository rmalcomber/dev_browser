import { useEffect, useRef } from 'preact/hooks';
import { FieldRow } from './ToolView';
import { css } from './state';
import { collapse, focusRequest, focusWanted, fold, move, open, resize, runWidget, sections, togglePanel, unpin, width, widgetState } from './panelState';
import type { PinnedTool } from './panelState';

/** One pinned tool: a title bar, and (unless collapsed) its input and its labelled values. */
function WidgetCard({ w, first, last }: { w: PinnedTool; first: boolean; last: boolean }) {
  const st = widgetState(w.id);
  return (
    <section class="pwidget" aria-label={w.name}>
      <div class="pbar">
        <button type="button" class="pfold" aria-expanded={!w.collapsed} aria-label={`${w.collapsed ? 'Expand' : 'Collapse'} ${w.name}`} onClick={() => collapse(w.id, !w.collapsed)}>
          {w.collapsed ? '▸' : '▾'}
        </button>
        <strong class="pname">{w.name}</strong>
        {w.sensitive && <span class="tsecret">not stored</span>}
        <button type="button" class="pmove" aria-label={`Move ${w.name} up`} title="Move up" disabled={first} onClick={() => move(w.id, -1)}>
          ▲
        </button>
        <button type="button" class="pmove" aria-label={`Move ${w.name} down`} title="Move down" disabled={last} onClick={() => move(w.id, 1)}>
          ▼
        </button>
        <button type="button" class="x" aria-label={`Unpin ${w.name}`} title="Unpin" onClick={() => unpin(w.id)}>
          ×
        </button>
      </div>
      {!w.collapsed && (
        <div class="pbody">
          {w.takes_input ? (
            <textarea
              class="tinput"
              aria-label={`${w.name} input`}
              placeholder={w.hint || 'Type or paste here'}
              spellcheck={false}
              value={st.input}
              onInput={(e) => runWidget(w.id, (e.currentTarget as HTMLTextAreaElement).value)}
            />
          ) : (
            <button type="button" class="trun" onClick={() => runWidget(w.id, '')}>
              Generate
            </button>
          )}
          {st.error && (
            <p class="terror" role="alert">
              {st.error}
            </p>
          )}
          {st.output && (
            <div class="toutput">
              {st.output.notes.map((n) => (
                <p class="tnote">{n}</p>
              ))}
              {st.output.fields.map((f) => (
                <FieldRow label={f.label} value={f.value} block={f.block} />
              ))}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

/** The widget panel: pinned tools in a section for each workspace in view, coloured for it. */
export function PanelApp() {
  const root = useRef<HTMLDivElement>(null);
  // the app asked for the keyboard focus: the first thing that takes input
  // (the request can come while the panel is still a rail; it waits until the panel itself is on screen)
  useEffect(() => {
    if (focusRequest.value === 0 || !focusWanted.value || !open.value) return;
    // a widget's input first, else its Generate button; the widgets may be a moment behind the panel opening, so look a few times
    let tries = 12;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const attempt = () => {
      const el = root.current;
      const target = el?.querySelector<HTMLElement>('.pwidget textarea, .pwidget button.trun');
      if (target) {
        target.focus();
        focusWanted.value = false;
      } else if (tries-- > 0) {
        timer = setTimeout(attempt, 30);
      } else {
        // nothing pinned to type into: the first button, so the keyboard is at least in the panel
        el?.querySelector<HTMLElement>('button')?.focus();
        focusWanted.value = false;
      }
    };
    attempt();
    return () => clearTimeout(timer);
  }, [focusRequest.value, open.value]);

  // From the keyboard the same edge takes the arrow keys: left makes the panel wider, right narrower (the app clamps it and remembers it).
  const onGripKey = (e: KeyboardEvent) => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault();
    resize(e.key === 'ArrowLeft' ? -20 : 20, true);
  };

  // Dragging the left edge resizes the panel (wider when dragged left). The pointer is captured by the edge, so the drag keeps working when
  // the cursor leaves the panel's own view (the page beside it is another window); the app clamps the width and remembers where it ended.
  const grip = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = grip.current;
    if (!el) return;
    let last: number | null = null;
    const down = (e: Event) => {
      const p = e as PointerEvent;
      e.preventDefault();
      el.setPointerCapture?.(p.pointerId);
      last = p.screenX;
    };
    const move = (e: Event) => {
      if (last === null) return;
      const x = (e as PointerEvent).screenX;
      if (x !== last) resize(x - last, false);
      last = x;
    };
    const up = () => {
      if (last === null) return;
      last = null;
      resize(0, true);
    };
    el.addEventListener('pointerdown', down);
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
    return () => {
      el.removeEventListener('pointerdown', down);
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
    };
    // the edge exists only while the panel is expanded (a collapsed panel is a rail without one), so listen again whenever that changes
  }, [open.value]);

  // collapsed: only a narrow rail with the way back (the panel's own page stays loaded, so what is pinned is ready the moment it opens)
  if (!open.value) {
    const count = sections.value.reduce((n, s) => n + s.widgets.length, 0);
    return (
      <div class="prail" role="complementary" aria-label="Pinned tools (collapsed)">
        <button type="button" class="expand" aria-label="Show the widget panel" title="Show the widget panel (Ctrl+Shift+B)" onClick={togglePanel}>
          ◂
        </button>
        <span class="prailtext" aria-hidden="true">
          Pinned tools{count ? ` (${count})` : ''}
        </span>
      </div>
    );
  }

  return (
    <div class="panel" ref={root} role="complementary" aria-label="Pinned tools">
      <div
        class="pgrip"
        ref={grip}
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize the panel"
        aria-valuenow={width.value}
        tabIndex={0}
        title="Drag to resize, or use the left and right arrow keys"
        onKeyDown={onGripKey}
      />
      <div class="pmain">
        <div class="ptop">
          <strong>Pinned tools</strong>
          <button type="button" class="collapse" aria-label="Collapse the widget panel" title="Collapse the widget panel (Ctrl+Shift+B)" onClick={togglePanel}>
            ▸
          </button>
        </div>
        {sections.value.map((s) => {
          const isFolded = s.folded;
          return (
            <div class="psection" style={{ borderTopColor: css(s.workspace.colour) }}>
              <button type="button" class="phead" aria-expanded={!isFolded} onClick={() => fold(!s.folded)}>
                <span class="ldot" style={{ background: css(s.workspace.colour) }} aria-hidden="true" />
                <span class="pwsname">{s.workspace.name}</span>
                <span class="pcount">{s.widgets.length}</span>
                <span aria-hidden="true">{isFolded ? '▸' : '▾'}</span>
              </button>
              {!isFolded &&
                (s.widgets.length === 0 ? (
                  <p class="pempty">Nothing pinned here. Open a tool from the launcher (Ctrl+P) and press Pin.</p>
                ) : (
                  s.widgets.map((w, i) => <WidgetCard w={w} first={i === 0} last={i === s.widgets.length - 1} />)
                ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
