# M0: Chromium feasibility spike

**Goal:** answer, with evidence, whether a Rust host can embed Chromium with per-workspace sessions, a browser-owned
overlay, reliable shortcut interception and two panes. Throwaway code is fine; the output is a decision, not a product.

**Status:** not started. Needs Roy's approval before any dependency is added or CEF is downloaded (AGENTS.md).

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
| 2 | Overlay | A browser-owned draggable panel renders above a live page; dragging is smooth; works at 100% and 150% DPI and after resize. IME and focus noted. | Screen capture |
| 3 | `Ctrl+P` | Opens our overlay, never the print dialog, while the page, an input, and an iframe have focus. `Escape` returns focus to the prior element. | Pre-key-event handler notes |
| 4 | Two sessions | Two request contexts with distinct `cache_path` under one `root_cache_path`. Sign in to GitHub as two identities; both survive an app restart; cookies are not shared. | Both account names visible after restart |
| 5 | Two panes | Two views side by side with a draggable divider; click and keyboard focus go to the right view; `Ctrl+1/2` focus switch works; no flash of the wrong view when swapping. | Screen capture |

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
