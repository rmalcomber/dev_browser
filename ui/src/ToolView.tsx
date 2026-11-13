import { useEffect, useRef, useState } from 'preact/hooks';
import { copyText, leaveTool, pin, runTool, tool, toolError, toolInput, toolOutput } from './launcherState';

/** One labelled value with a button that copies exactly that value. */
export function FieldRow({ label, value, block }: { label: string; value: string; block: boolean }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    if (await copyText(value)) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }
  };
  return (
    <div class="tfield">
      <div class="tlabel">{label}</div>
      {block ? <pre class="tvalue">{value}</pre> : <div class="tvalue inline">{value}</div>}
      <button type="button" class="tcopy" aria-label={`Copy ${label}`} onClick={copy}>
        {copied ? 'Copied' : 'Copy'}
      </button>
    </div>
  );
}

/** A tool, open inside the launcher: its input (if it reads one), its output and its notes. */
export function ToolView() {
  const t = tool.value;
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (t?.takes_input) input.current?.focus();
  }, [t?.tool]);
  if (!t) return null;
  const out = toolOutput.value;
  return (
    <div class="ltool">
      <div class="thead">
        <button type="button" onClick={leaveTool} aria-label="Back to search">
          ← Back
        </button>
        <button type="button" onClick={() => pin(t.tool)} aria-label="Pin to the widget panel" title="Pin to the widget panel (it stays beside the page)">
          Pin
        </button>
        <strong>{t.name}</strong>
        {t.sensitive && <span class="tsecret">not stored or logged</span>}
      </div>
      <p class="tdesc">{t.description}</p>
      {t.takes_input ? (
        <>
          <textarea
            ref={input}
            class="tinput"
            aria-label={`${t.name} input`}
            placeholder="Type or paste here"
            spellcheck={false}
            value={toolInput.value}
            onInput={(e) => runTool(t.tool, (e.currentTarget as HTMLTextAreaElement).value)}
          />
          <div class="tsource">
            {t.from_selection ? (
              <span>Started from the text selected on the page.</span>
            ) : (
              <span>Type or paste the text above.</span>
            )}
          </div>
        </>
      ) : (
        <button type="button" class="trun" onClick={() => runTool(t.tool, '')}>
          Generate another
        </button>
      )}
      {toolError.value && (
        <p class="terror" role="alert">
          {toolError.value}
        </p>
      )}
      {out && (
        <div class="toutput" role="region" aria-label="Result">
          {out.notes.map((n) => (
            <p class="tnote">{n}</p>
          ))}
          {out.fields.map((f) => (
            <FieldRow label={f.label} value={f.value} block={f.block} />
          ))}
        </div>
      )}
    </div>
  );
}
