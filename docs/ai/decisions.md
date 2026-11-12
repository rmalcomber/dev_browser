# Decisions

Newest first. One entry per non-obvious decision: what, why, what was rejected.

## 2026-11-12: Shell integration: direct CEF through cef-rs (decided after the M0 spike)
- **Decision:** The browser shell is a Rust host that embeds CEF directly through the `cef` bindings from [tauri-apps/cef-rs](https://github.com/tauri-apps/cef-rs), using CEF's Views framework with Alloy-style browser views (see the entry below). Tauri with CEF is not used.
- **Why:** The M0 spike (`spike/`, results in `planning/m0-feasibility-spike.md`) ran all five tests on Windows with this stack: embedding, a browser-owned draggable overlay, `Ctrl+P` interception from a page, an input and an iframe, one request context and profile per workspace that survives restarts without leaking, and two panes with a saved divider, outlines and flash-free workspace swaps. It also showed the pieces M1 to M3 need work: per-view destroy and recreate, app-owned dialogs and permission prompts, a per-workspace camera and microphone decision store, and a render-process wrapper that stops live capture. Direct CEF gives the control this needs (request contexts, pre-key-event handler, frame and render-process handlers).
- **Rejected:** Tauri with CEF: never tried. It would give a faster shell but the spec flags overlay and split-view as unproven there, and the spike made direct CEF work, so there was no reason to take on a second integration layer. Revisit only if direct CEF is dropped for another reason (maintenance, packaging, a platform it cannot reach).
- **Cost in bytes/deps:** CEF runtime about 426 MB extracted for the minimal Windows x64 build (installed size of our own binary plus runtime still to be measured and published). Dependencies: `cef` and `cef-dll-sys` (cef-rs, pinned to a git revision because crates.io 156.4.0 could not build: its CEF download step fails on the CDN index size), CMake and Ninja as build tools on Windows, and a one-time CEF export (`export-cef-dir`) with a local patch to the download limit. None of this ships in our own code.
- **Known caveats carried forward:** validated on Windows only (Linux and macOS untested); the GPU process crashed at start-up with several views until DirectComposition was disabled (root cause and cost unknown); IME and real OS-level DPI scaling were not checked; a page can defeat the capture-stop wrapper, so a browser-side reload fallback is required; Alloy-style views mean downloads, permission prompts, dialogs and error pages are ours to build.

## 2026-11-12: Alloy-style browser views inside the Chrome bootstrap (confirmed by M0)
- **Decision:** Build panes and the overlay as Alloy-style CEF BrowserViews. Confirmed in the M0 spike: tests 2, 3 and 5 were run with real input and screenshots and pass.
- **Why:** Chrome style hosts one browser per window; split view and a browser-owned overlay need several. CEF kept Alloy style on purpose for this (cef issue 3681) after removing the Alloy bootstrap in M128.
- **Rejected:** Chrome style for everything (one browser per window); one native window per pane (not tried, awkward for a single-window design).
- **Cost in bytes/deps:** None. Chrome features (downloads UI, permission prompts, error pages) become ours to implement. Keep the style choice in one place.

## 2026-11-12: Chromium is a deliberate exception to the Floppy Rule
- **Decision:** Accept CEF/Chromium as the rendering runtime. Hold everything we write (Rust code, shell UI, docs) to the Floppy Rule, and measure and publish the installed size of the runtime honestly.
- **Why:** Writing a rendering engine is out of scope, and web compatibility is the point of a daily-driver browser.
- **Rejected:** A platform webview (weaker per-workspace session isolation); a new engine (non-goal).
- **Cost in bytes/deps:** Large (the CEF runtime). Exact figure recorded after M0.

## 2026-11-12: Shell integration is undecided (superseded by the decision above)
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
