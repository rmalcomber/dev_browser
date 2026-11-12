# M0: Chromium feasibility spike

**Goal:** answer, with evidence, whether a Rust host can embed Chromium with per-workspace sessions, a browser-owned
overlay, reliable shortcut interception and two panes. Throwaway code is fine; the output is a decision, not a product.

**Status:** in progress (see Results at the end). Needs Roy's approval before any dependency is added or CEF is downloaded (AGENTS.md).

## Decisions needed before starting

| Question | Proposed default | Who decides |
| --- | --- | --- |
| First platform | Windows 11 (the daily driver). Linux and macOS are later, separate commitments. | Roy |
| Candidates | A: direct CEF via Rust bindings. B: Tauri with CEF. Time-box each. | Roy |
| Spike location | `spike/` inside this repo, clearly marked disposable, not part of the docs site. | Roy |

## Candidates to evaluate

For each, record: binding crate and version, maintenance activity, CEF version it tracks, how windowing is owned, how many
lines of glue were needed, and what broke.

- **A. Direct CEF bindings.** Most control (request contexts, pre-key-event handler, child windows). Most glue.
- **B. Tauri with CEF.** Faster shell and UI. Risk: the spec flags that overlay and split-view combinations may not work out of the box.

## Tests (all must have a written result)

| # | Test | Pass means | Evidence |
| --- | --- | --- | --- |
| 1 | Embed and navigate | A page loads, back/forward work, event loop is stable on resize and close. | Screenshot, notes |
| 2 | Overlay | **Pass for behaviour, not yet seen by eye.** An overlay (`add_overlay_view`, custom docking) hosting its own browser view is created, shown and hidden over the live pages. A real mouse drag (`SendInput`) moved it by the expected steps (+10,+5 each, 12 steps) and the position is clamped to the window. Not checked: how it looks, drag after a window resize, 150% DPI, IME. |
| 3 | `Ctrl+P` | **Pass in the cases run** (real key input via `SendInput`, read from the log): intercepted with focus in a page text input, in the iframe test area, and in the other pane; the page never saw it (`on_pre_key_event` returned handled). `Escape` closes the overlay and focus returns to the pane that had it (A or B). Not checked: that no print dialog flashes (the key is consumed before the page, so it should not), pages with their own `Ctrl+P` handler, a focused PDF viewer, non-US keyboard layouts. |
| 4 | Two sessions | Two request contexts with distinct `cache_path` under one `root_cache_path`. Sign in to GitHub as two identities; both survive an app restart; cookies are not shared. | Both account names visible after restart |
| 5 | Two panes | **Mostly pass.** In Alloy style both panes and the overlay are created and laid out side by side (588x681 each). Mouse clicks and `Ctrl+1` / `Ctrl+2` move keyboard focus to the intended pane (confirmed by focus events), and the overlay returns focus to the right one. In Chrome style only the first browser view gets a browser. Not done: a draggable divider (Views has no splitter), the coloured outline, flash-free pane/workspace swaps (needs eyes). |

Use throwaway test accounts or a non-sensitive login where possible; never record credentials in this repo.

## Also measure

- Installed size of the app plus CEF runtime, and idle memory with two views (the Floppy Rule: be honest about the cost).
- Time to first paint and to restore a session.
- Build time and toolchain requirements on a clean Windows machine.
- How Chromium security updates would be consumed (feeds the M4 update procedure).

## Outcome template (copy into `docs/ai/decisions.md`)

```text
## YYYY-MM-DD: Browser shell integration
- Decision: A direct CEF | B Tauri with CEF | neither (reason)
- Why: results of tests 1-5, with links to evidence
- Rejected: the other candidate, and why
- Cost in bytes/deps: installed size, crates added
```

## Exit criteria

All five pass on the chosen candidate on Windows, or the spike ends with a written recommendation to change approach
(for example a different shell, or a platform webview with weaker isolation, which would require revisiting the spec's
privacy model).

## Results so far (2026-11-12, Windows 11, candidate A: cef-rs 156.5.0 / CEF 156.0.28, Views framework)

Code is in `spike/` (see its README). Checked from `[m0]` log lines and the files on disk only: I cannot see or drive the
native window from the tooling used, so anything visual or typed is marked **not yet verified by eye**.

| # | Test | Result |
| --- | --- | --- |
| 1 | Embed and navigate | **Pass.** Window opens, `https://example.com/` loads (HTTP 200). |
| 2 | Overlay | **Partial.** An overlay (`add_overlay_view`, custom docking) hosting its own browser view is created and loads its page. Dragging (a title-channel hack to move it) is coded, **not yet verified by eye**. |
| 3 | `Ctrl+P` | **Coded, not yet run.** `on_pre_key_event` toggles the overlay and consumes the key; needs real key input into a focused window. |
| 4 | Two sessions | **Pass (Alloy style).** Each workspace gets its own profile directory under the common root (`ws-a/`, `ws-b/`, each with its own `Local Storage`). A value written in A is still there after a restart and pane B never sees it, both runs. Authentication cookies not tried yet; localStorage on the same origin is the same storage partition test. |
| 5 | Two panes | **Partial.** In Alloy style both panes and the overlay are created and laid out side by side (588x681 each). In Chrome style only the first browser view ever gets a browser. No divider drag yet (Views has no splitter). Focus switching and `Ctrl+1/2` are coded, **not yet verified by eye**. |

### Why test 4 looked blocked, and what fixed it

1. **Path separators.** CEF compares the request-context `cache_path` with its user data dir as strings. A forward-slash
   path gave `Cannot create profile at path` and a silent fall-back to an in-memory profile on the global `Default/`
   store (so data seemed to vanish). Use exactly the same (backslash) form as `root_cache_path`, and a directory directly under it.
2. **Asynchronous profile creation.** With a valid path CEF creates the profile asynchronously, and `browser_view_create`
   returns `None` until `RequestContextHandler::on_request_context_initialized` has fired. Create the browser views from
   that callback (the spike builds its UI once every context is ready).

### Findings worth keeping

- **Chrome vs Alloy style (support horizon checked 2026-11-12).** Chrome style gave a browser only for the first
  `BrowserView` in a window; Alloy style did two panes and the overlay. CEF removed the Alloy *bootstrap* in M128 (Chrome
  bootstrap is the only one now), but kept Alloy *style* on purpose: [cef issue 3681](https://github.com/chromiumembedded/cef/issues/3681)
  adds lightweight Alloy-style windows and browsers, explicitly "multiple browsers in a single Chrome window ... with different
  CefRequestContext", and was closed on 2025-12-06 with a test for several Alloy-style BrowserViews in one Window. So the
  spike's setup (Chrome bootstrap, Alloy-style views) is the supported route for this use. Trade-off: Alloy style lacks the
  Chrome UI and some Chrome features, so downloads, permission prompts, popups and error pages are ours to build.
- `cef` 156.4.0 from crates.io cannot build as is: its CEF download fails because the CDN `index.json` is over the 10 MB
  response limit in `download-cef`; cef-rs master has the same limit. Worked around with a local patch when exporting CEF.
- Windows needs CMake and Ninja as well as Visual Studio. The minimal CEF runtime is about 426 MB extracted.
- Closing: Chrome style exits cleanly; in Alloy style the process did not exit after closing every browser (window
  teardown still to sort out).

### Next steps for M0

1. Look at the window once by eye (overlay position and look, no print dialog, no flashes) and try 150% DPI and a resize.
   Add a draggable divider. Input automation lives in `spike/drive.ps1` (it takes over the mouse and keyboard while it runs).
2. Fix Alloy shutdown; try a real login cookie flow (not just localStorage) for test 4.
3. List which Chrome-only features we lose with Alloy-style views (downloads, permissions, printing, extensions) and
   keep the style choice behind one function so a CEF change is a small edit.
4. Time-box candidate B (Tauri with CEF) against the same five tests only if A cannot reach a pass.
