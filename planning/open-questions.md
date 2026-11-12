# Open questions

From spec section 12 and gaps found while planning. Close an item by recording the answer in `docs/ai/decisions.md` and
linking it here.

| # | Question | Why it matters | Next step | Decider |
| --- | --- | --- | --- | --- |
| 1 | Direct CEF bindings or Tauri with CEF? | Shapes the whole codebase; maturity, windowing, packaging, GPU. | M0 spike | Roy, after evidence |
| 2 | First supported OS? | Each OS is a separate test and packaging commitment. | Proposed: Windows. Confirm. | Roy |
| 3 | Does a browser-owned overlay survive two live Chromium views (layering, focus, drag, IME, DPI)? | The launcher is the headline feature. | M0 test 2, 3, 5 | Evidence |
| 4 | Do per-workspace `cache_path` values persist logins across restarts without leaking? | Core promise of workspaces. | M0 test 4 | Evidence |
| 5 | How is selected text read across frames, iframes and internal pages, with consent? | Context-aware tools depend on it. | Design in M3 | Roy |
| 6 | How will Chromium security updates ship? | Mandatory before important accounts. | Plan in M4; note findings from M0 | Roy |
| 7 | Tool sandbox for non-bundled tools? | The database split is organisation, not a sandbox. | Defer; bundled audited tools only in v1 | Roy |
| 8 | Retention and deletion for history and the search index; FTS index ownership. | Privacy and leak prevention. | Design in M1/M2 | Roy |
| 9 | Accessibility of the launcher and draggable overlay (keyboard, screen readers, contrast, fallback). | Required, not polish. | Design in M2 | Roy |
| 10 | Single trusted Rust host vs separate workers. | Prefer single host plus Chromium's model for v1. | Revisit with third-party tools | Roy |
| 11 | UI framework for the shell: reuse Home Screen's stack as is? | Floppy Rule; see [home-screen-reuse.md](home-screen-reuse.md). | Confirm after M0 | Roy |
| 12 | Licensing and redistribution terms for the CEF/Chromium binaries. | Affects packaging and the public repo. | Check before the first release | Roy |
| 13 | Binary size budget. Chromium is far larger than the Floppy Rule's spirit. | Be explicit about the exception and what stays small (our code, docs, UI). | Record in decisions.md | Roy |
