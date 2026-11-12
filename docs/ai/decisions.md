# Decisions

Newest first. One entry per non-obvious decision: what, why, what was rejected.

## 2026-11-12: Alloy-style browser views inside the Chrome bootstrap (provisional)
- **Decision:** Build panes and the overlay as Alloy-style CEF BrowserViews. Provisional until M0 tests 2, 3 and 5 are checked by hand.
- **Why:** Chrome style hosts one browser per window; split view and a browser-owned overlay need several. CEF kept Alloy style on purpose for this (cef issue 3681) after removing the Alloy bootstrap in M128.
- **Rejected:** Chrome style for everything (one browser per window); one native window per pane (not tried, awkward for a single-window design).
- **Cost in bytes/deps:** None. Chrome features (downloads UI, permission prompts, error pages) become ours to implement. Keep the style choice in one place.

## 2026-11-12: Chromium is a deliberate exception to the Floppy Rule
- **Decision:** Accept CEF/Chromium as the rendering runtime. Hold everything we write (Rust code, shell UI, docs) to the Floppy Rule, and measure and publish the installed size of the runtime honestly.
- **Why:** Writing a rendering engine is out of scope, and web compatibility is the point of a daily-driver browser.
- **Rejected:** A platform webview (weaker per-workspace session isolation); a new engine (non-goal).
- **Cost in bytes/deps:** Large (the CEF runtime). Exact figure recorded after M0.

## 2026-11-12: Shell integration (direct CEF vs Tauri with CEF) is undecided
- **Decision:** Do not choose before the M0 spike. Both candidates are evaluated against the same five tests (`planning/m0-feasibility-spike.md`).
- **Why:** Binding maturity, overlay layering and multi-view behaviour are unproven; the spec marks this "to validate".
- **Rejected:** Picking a framework first and building around it.
- **Cost in bytes/deps:** None yet.

## 2026-11-12: Two SQLite databases per workspace, plus one global
- **Decision:** Global `browser.db` holds only workspace metadata and preferences. Each workspace has `browser.db` (history, bookmarks, tabs, groups, search) and `homescreen.db` (launcher, widgets, tool instances). Chromium keeps cookies and site storage in its own per-workspace context.
- **Why:** Isolation and independent migrations. This is data organisation, not a security sandbox; cross-database atomic transactions are not assumed.
- **Rejected:** One shared database with a workspace column (easy to leak across workspaces); copying bookmarks into Home Screen storage (duplication).
- **Cost in bytes/deps:** SQLite, already used by `home_screen`.

## 2026-11-12: Rust core with Chromium rendering; reuse Home Screen UI and tools
- **Decision:** Rust application core, embedded Chromium, launcher and bundled tools taken from `home_screen` behind a typed IPC and a capability broker.
- **Why:** Agreed in spec v0.1 (see the decision log in `planning/home-screen-browser-spec.md`).
- **Rejected:** Chrome extension (cannot own the launcher or isolate sessions this way); Electron-style shells as the default.
- **Cost in bytes/deps:** To be measured.
