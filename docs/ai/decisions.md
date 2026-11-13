# Decisions

Newest first. One entry per non-obvious decision: what, why, what was rejected.

## 2026-11-13: Workspaces and storage over IPC (M1.2)
- **Dependencies:** `serde` (derive) and `serde_json`, in `core` only, for the IPC messages. Why: the shell and the app now exchange structured messages (workspace list, create, rename, colour, delete, activate); hand-written JSON parsing at a trust boundary is more code to get wrong than the standard crates. Both were already compiled in the build tree (as build dependencies of `cef`). Approved by Roy, 2026-11-13. **Rejected:** a hand-rolled parser, and a tab-separated wire format (fragile for arbitrary names).
- **Decision:** `core::backend::Backend` owns the data folder and the global database and decides every message; the host only executes what it returns. Replies are always the **full** `workspaces` state (list plus active id), so the shell never patches its own copy. Effects are `Activate(id)` and `Delete(id)`. Protocol violations (unknown name, bad or oversized JSON, unknown fields) are refused and logged, never answered; user errors (bad name, deleting the only workspace) get an `error` reply. A refusal never echoes the payload (serde's own messages quote offending values, so only the error class and position are logged). Ids from the shell go through `Id::parse` and must name an existing workspace. There is always at least one workspace and exactly one is active; the active id is saved in the global `settings` table and restored.
- **Data removal:** deleting a workspace removes the record at once and closes its view. **Finding:** Chromium keeps a deleted workspace's profile open for the rest of the run (a full Chrome profile: `Account Web Data`, `DIPS`, ...), so its folders cannot be removed while the app runs (retrying for 20 s did not help). The host therefore sweeps orphaned `ws-` folders again right after CEF shuts down (retrying a few seconds for child processes), and `Backend::open` sweeps before CEF starts, so the data is gone when the app exits, or at the latest at the next start. **Known gap:** until then the deleted workspace's cookies and storage are on disk; erasing them at delete time (clearing storage through CEF) is not done. The sweep only ever removes folders named like a workspace id (kind `ws`); CEF's own folders in `chromium-root` are never touched.
- **Isolation:** one request context per workspace, created on first show, with its cache folder a direct child of `chromium-root` (checked). `tools\m12-workspaces.ps1` shows two workspaces on the same site have separate cookies and localStorage, and that each keeps its own after a restart.
- **Runs on the UI thread:** the backend's SQLite calls run on the CEF UI thread, not on a storage thread as `planning/m1-plan.md` section 4 describes. Why: each message is one tiny local write today. The call site is a single place (`host/src/ipc.rs`), so moving it behind a channel later changes nothing else. Revisit when tabs and history add volume (M1.3 and later).
- **Not done in M1.2:** the colour frame around the page and workspace colour in the title (M1.4), restoring each workspace's tabs (M1.3: every workspace opens the start URL), keyboard shortcuts for workspace switching (M1.3's command registry; the sidebar is operable with Tab, Enter and Escape meanwhile).

## 2026-11-13: How the shell is served and how IPC v0 is guarded (M1.1)
- **Decision:** The shell is served from the custom scheme `app://shell/`, registered `STANDARD | SECURE | CORS_ENABLED | FETCH_ENABLED | DISPLAY_ISOLATED`, from a `ui` folder next to the executable (a copy of `ui/dist` made by `tools/build.ps1`). The shell view uses CEF's global request context, never a workspace context, so it shares no cookies or storage with a page. `window.m0` is installed in the renderer only for a main frame whose URL starts with `app://shell/`. The browser process checks again on every message: the sending view must be the shell view and its frame must still be an `app://shell/` page; anything else is dropped and logged. What a message means is decided in `dev_browser_core::ipc` (v0: `ping` answers `pong`, unknown names and payloads over 64 KiB are refused). Replies reach the page as a call to `m0.onmessage(name, payload)` with both strings escaped as JavaScript literals.
- **Why:** two independent checks (renderer binding, browser gate) mean a page that somehow gets the binding, or a compromised renderer that skips JavaScript, still cannot talk to the app. `DISPLAY_ISOLATED` stops other origins embedding or linking to the shell. Serving from a folder keeps the first slice small; the path resolver (`shell::resolve`) refuses `..`, backslashes, colons and drive letters and is unit-tested.
- **Rejected:** embedding `ui/dist` in the binary now (needs a dependency or a build script; the folder can be replaced by embedding later without touching callers); a V8 accessor for `onmessage` (calling a plain property through `execute_java_script` is simpler and the payload is escaped); trusting the frame URL seen by the renderer alone.
- **Checked (2026-11-13), `tools/s6-hostile-page.ps1`:** a hostile http page sees no `window.m0` (own or `top`), and its fetch, XHR and iframe to `app://shell` all fail. **Finding:** a page view cannot load `app://shell` at all (`ERR_UNKNOWN_URL_SCHEME`), because the scheme handler is registered only on the global request context and workspace contexts do not inherit it. That is a stronger first line than planned, but it depends on the factory staying global-only, so the script checks it (part B). The browser-side gate (`ipc::trusted`) is unit-tested but not exercised end to end, since no page can reach it; a compromised-renderer test would need a test hook and is left for M1.7. The script was also shown to fail when the renderer installs `window.m0` for every page.

## 2026-11-12: Dependencies for the M1 foundation
- **Decision:** Rust `core`: `rusqlite` with the bundled SQLite (the same crate and feature `home_screen` uses; the spec's storage is SQLite). UI: `preact` and `@preact/signals` (the `home_screen` stack, so its components drop in at M2), `vite` and `@preact/preset-vite` (build), `vitest` and `jsdom` (tests). Nothing else yet; `cef` and `cef-dll-sys` arrive with the `host` crate (see the shell decision).
- **Why:** each has one job and no cheaper standard-library or platform route: the standard library has no SQLite, and a sidebar with groups needs a view layer and a build step.
- **Rejected:** a CSS framework in the shell for now (`home_screen` uses Tailwind and daisyUI; revisit when the sidebar needs it); an ORM or migration crate (a small hand-written migration runner is enough and easier to test).
- **Cost in bytes/deps:** the UI bundle is small (Preact is about 4 KB gzipped); SQLite adds roughly 1 MB to the binary. Space is not a constraint here (Roy, 2026-11-12), so these are recorded for review, not as a budget.

## 2026-11-12: Data location, `%LOCALAPPDATA%\dev_browser\` (D6)
- **Decision:** All application data lives under `%LOCALAPPDATA%\dev_browser\`: global `browser.db`, `chromium-root\` (CEF's root cache path, one direct child per workspace), and `workspaces\<id>\` with each workspace's `browser.db` and `homescreen.db`. Workspace folders use generated IDs, never names. A `--data-dir` flag overrides it for tests.
- **Why:** it matches the spec's layout and keeps browser data out of roaming profiles. The override keeps tests and the spike-style tools away from real data.
- **Rejected:** `%APPDATA%` (roams, and would sync browser data between machines); a folder next to the executable (breaks on read-only installs).
- **Cost in bytes/deps:** none.

## 2026-11-12: What CI covers (D5)
- **Decision:** CI builds the Rust crates that do not need CEF, runs `cargo test` and `cargo clippy`, runs the UI unit tests and the docs build. The GUI checks (real input, window capture, frame analysis) stay local scripts in `tools/`. Adding the CI workflow itself needs Roy's approval first (it is deploy-adjacent configuration).
- **Why:** a hosted Windows runner has no interactive desktop, and the M0 checks need one. They found defects logs and unit tests missed, so they stay part of "done", run by hand before a slice is called finished.
- **Rejected:** running the GUI checks in CI (unreliable without a desktop); skipping them (they paid for themselves in M0).
- **Cost in bytes/deps:** none.

## 2026-11-12: cef-rs and CEF version policy (D4)
- **Decision:** Pin cef-rs to an exact git revision and CEF to the version it targets. Upgrade deliberately, on a schedule that follows Chromium security releases, and re-run the `tools/` checks (and the M0 wrapper and destroy scenarios) on every upgrade. Record each bump here.
- **Why:** crates.io 156.4.0 did not build (the CEF download step fails on the CDN index size), so a git pin is needed anyway; a pin also makes builds reproducible and makes picking up a Chromium security fix a conscious step rather than an accident. The spec makes a security-update strategy mandatory before important accounts are used.
- **Rejected:** floating on crates.io or `main` (builds change under us); vendoring cef-rs (more to maintain).
- **Cost in bytes/deps:** one pinned git dependency; a CEF export step per machine (`tools/`).

## 2026-11-12: First supported OS is Windows (D3, confirmed by Roy)
- **Decision:** Windows is the first supported platform. Platform-specific code stays inside the `host` crate. Linux and macOS are separate, later commitments, each with its own test and packaging work.
- **Why:** it is the only platform the M0 spike validated and the one in daily use.
- **Rejected:** building for several platforms from the start (each multiplies the test and packaging work before there is a working product).
- **Cost in bytes/deps:** none.

## 2026-11-12: How the shell talks to Rust (D2)
- **Decision:** The shell view and the Rust host talk over CEF process messages with a small V8 binding (`window.m0`) that the render process installs **only** for documents served from the private `app://shell` origin. Web pages get no binding. Commands and events are typed: one Rust enum, one TypeScript type generated from it. The Rust side resolves the caller from the frame that sent the message, never from a field in the message.
- **Why:** it replaces the spike's page-title hack, keeps privileged IPC out of web pages by construction (the first S6 check), and makes caller identity something the browser knows rather than something a page claims.
- **Rejected:** a localhost HTTP server (reachable from any page and any local process: fine for a personal start page like `home_screen`, not for a browser); `cefQuery` (works, but is a generic string channel we would have to type ourselves).
- **Cost in bytes/deps:** none beyond the cef crate.

## 2026-11-12: Shell UI is Preact in trusted browser views (D1, confirmed by Roy)
- **Decision:** The sidebar, toolbar and later the launcher and widget panel are Preact (with signals) apps built with Vite and loaded in trusted browser views from `app://shell`. They are not CEF Views widgets.
- **Why:** the launcher and tools in M2 and M3 are `home_screen` code, which is Preact; one stack means they drop in. The spike's overlay and divider already prove trusted views work in a Views window. Views widgets cannot do a sidebar with groups and drag in a reasonable amount of code.
- **Rejected:** CEF Views widgets for the chrome (too limited); a different framework (loses the `home_screen` reuse).
- **Cost in bytes/deps:** see the dependencies entry above.

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
