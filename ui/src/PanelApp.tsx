import { useEffect, useRef } from 'preact/hooks';
import { FieldRow } from './ToolView';
import { css } from './state';
import { collapse, focusRequest, folded, resize, runWidget, sections, toggleSection, unpin, widgetState } from './panelState';
import type { PinnedTool } from './panelState';

/** One pinned tool: a title bar, and (unless collapsed) its input and its labelled values. */
function WidgetCard({ w }: { w: PinnedTool }) {
  const st = widgetState(w.id);
  return (
    <section class="pwidget" aria-label={w.name}>
      <div class="pbar">
        <button type="button" class="pfold" aria-expanded={!w.collapsed} aria-label={`${w.collapsed ? 'Expand' : 'Collapse'} ${w.name}`} onClick={() => collapse(w.id, !w.collapsed)}>
          {w.collapsed ? '▸' : '▾'}
        </button>
        <strong class="pname">{w.name}</strong>
        {w.sensitive && <span class="tsecret">not stored</span>}
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
              placeholder="Type or paste here"
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
  useEffect(() => {
    if (focusRequest.value === 0) return;
    // a widget's input first, else its Generate button, else the first button
    const el = root.current;
    (el?.querySelector<HTMLElement>('.pwidget textarea, .pwidget button.trun') ?? el?.querySelector<HTMLElement>('button'))?.focus();
  }, [focusRequest.value]);

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
  }, []);

  return (
    <div class="panel" ref={root} role="complementary" aria-label="Pinned tools">
      <div class="pgrip" ref={grip} title="Drag to resize" aria-hidden="true" />
      <div class="pmain">
        {sections.value.map((s) => {
          const isFolded = folded.value.includes(s.workspace.id);
          return (
            <div class="psection" style={{ borderTopColor: css(s.workspace.colour) }}>
              <button type="button" class="phead" aria-expanded={!isFolded} onClick={() => toggleSection(s.workspace.id)}>
                <span class="ldot" style={{ background: css(s.workspace.colour) }} aria-hidden="true" />
                <span class="pwsname">{s.workspace.name}</span>
                <span class="pcount">{s.widgets.length}</span>
                <span aria-hidden="true">{isFolded ? '▸' : '▾'}</span>
              </button>
              {!isFolded &&
                (s.widgets.length === 0 ? (
                  <p class="pempty">Nothing pinned here. Open a tool from the launcher (Ctrl+P) and press Pin.</p>
                ) : (
                  s.widgets.map((w) => <WidgetCard w={w} />)
                ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
