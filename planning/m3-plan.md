# M3 plan: restricted tools and the widget panel

Source: `planning/home-screen-browser-spec.md` sections 3.3, 3.4 and 7, and the M3 entry in `roadmap-and-milestones.md`.
**Tests for this milestone:** S5 (tool code cannot reach browser databases or session files), S7 (page selection binds to the focused pane), S9 (token inputs are
not logged or persisted). All three are in `security-acceptance-tests.md`.

## Design decisions

| # | Question | Decision | Why |
| --- | --- | --- | --- |
| T1 | What is a "tool" in v1? | A **bundled Rust function** from text to a structured result (`core::tools`): `run(tool, input, now) -> Output`. No JavaScript, no process, no access to anything but its arguments. The five to start with are the JWT decoder, JSON formatter, Base64, URL and UUID. | The spec says bundled tools should be audited code behind broker APIs, and that a JavaScript realm shared with privileged UI is *not* isolation. A pure function has no ambient authority to isolate, so it needs no sandbox. Third-party tools are a different problem and stay out of scope until a real sandbox exists. |
| T2 | Who gives a tool its input, and who takes its output? | The **broker** (core). Input is text the user typed or pasted into the tool, or the text selected on the shown page, captured on a user action. Output is a list of labelled, copyable values plus notes. A tool never sees tabs, workspaces, cookies, files or the clipboard. | "A tool receives only the data required." |
| T3 | Reading the page selection (`page.selection.read`) | Only the **shown tab's main frame**, only when the launcher opens (a user action), through a request with a one-time id: the host asks the page's renderer for `window.getSelection()`, the renderer answers with the text, the host accepts the answer only for that id from that tab. Held in core memory until the launcher closes. Text selected inside an iframe is not read (noted limitation). | One pane today, so "the focused pane" is the shown tab (S7); split view (M4) must bind it to the focused pane. A page can say what it likes about its own selection, which is its own content. |
| T4 | Sensitive input (S9) | Tool input and the captured selection are **never written to a database, a file or a log**, only held in memory (core, and the launcher page) and dropped when the launcher closes. Tool outputs are not stored either. A test scans the data folder and the process log for a token after it was decoded. | The spec: tokens are in memory only, by default. |
| T5 | Copy out (`clipboard.write`) | The launcher page copies a chosen output value (the page is trusted; the tool is not). There is no way for a tool to read the clipboard. | Matches the spec's capability list; read access does not exist. |
| T6 | Widgets (M3.2) | A pinned tool is an **instance** in the workspace's `homescreen.db` (`widgets`: tool id, position, collapsed). Only non-sensitive configuration is stored, never input text. The panel is a third trusted view on the right, resizable and collapsible, with one collapsible section per workspace in view. | Matches 3.3 and 3.4: a tool's definition is global; its instances are workspace-specific. |

## Slices

- **M3.1** Tool registry, broker, the five tools, the launcher's tool mode with selection capture and copy. S5, S7 (single pane), S9. Real-window check.
- **M3.2** The right-hand widget panel: pin from the launcher, per-workspace sections, collapse, persistence.
- **M3.3** Resize, reorder, widget state rules (hidden widgets keep state, secrets cleared), the remaining exit tests.

**M3.1 status: done 2026-11-13** (see the M3.1 entry in `docs/ai/decisions.md`). `tools\m31-tools.ps1` passes on the real window; S5, S9 and the single-pane part of S7 pass.

## Not in M3

Third-party tools and any sandbox for them, split-view panes (M4: until then the focused pane is the shown tab), clipboard read, network tools (curl, http) which need a network capability of their own.
