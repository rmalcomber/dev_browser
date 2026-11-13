# M1 plan: browser and workspace foundation

Status: **draft for Roy to review** (2026-11-12). Built on the M0 spike (`m0-feasibility-spike.md`) and the shell decision in
`docs/ai/decisions.md`: a Rust host embedding CEF directly through cef-rs, Alloy-style browser views, Windows first.

## 1. What M1 delivers

M1 is the first thing you can actually use. Its exit test, from the milestones file:

> Log in to the same site as two identities in two workspaces, restart, both still signed in, nothing shared.

Around that, M1 is a working single-pane browser with workspaces:

- Create, rename, colour and delete workspaces; each has its own persistent Chromium session and its own databases.
- A left sidebar of workspaces with their tabs (and tab groups), with the agreed click rules: the name switches, the chevron only
  expands or collapses; a collapsed workspace shows only its name and icon.
- Tabs: open, close, select, restore the last tab per workspace, restore everything after a restart.
- An address bar with back, forward, reload, stop, title and favicon.
- Safe handling of the awkward cases: error pages, crashes, certificate errors (never bypassed), popups and new-window requests,
  JavaScript dialogs and permission prompts (app-owned, safe defaults; the full UI is M4).
- Workspace accent colour as a frame around the page, with no flash of the previous workspace when switching.

**Not in M1:** the launcher (M2), tools and widget panel (M3), split view and downloads UI (M4), history and bookmarks beyond the tab
metadata, extensions, themes, tear-off windows. The pane model is built so that M4 can add a second pane without a rewrite.

## 2. Decisions to settle first

Each is a short entry for `docs/ai/decisions.md`, written before the code that depends on it. My recommendation is first; Roy decides.

| # | Decision | Recommendation | Why |
| --- | --- | --- | --- |
| D1 | UI technology for the shell (sidebar, toolbar) | **Preact + signals + Vite in trusted browser views**, the same stack as `home_screen` | The launcher and tools in M2 and M3 are `home_screen` code; one stack means they drop in. The spike already shows trusted views (overlay, divider) work. CEF's native Views widgets are too limited for a sidebar with groups. |
| D2 | How the shell talks to Rust | **CEF process messages plus a small V8 binding (`window.m0`), installed only in shell views** (served from a private `app://shell` origin) | Replaces the spike's page-title hack. Web pages get no binding at all, which is what the S6 test checks. Commands are typed (one Rust enum, one TypeScript type generated from it). |
| D3 | First supported OS | **Windows** (confirm) | The only platform tested. Linux and macOS become separate, later commitments. |
| D4 | cef-rs and CEF version policy | **Pin cef-rs to a git revision and CEF to its matching version; upgrade deliberately, with the spike checks re-run** | crates.io 156.4.0 did not build (CDN index limit); a pin makes builds reproducible and makes Chromium security updates a conscious, scheduled act. |
| D5 | What CI covers | **Build, `cargo test` for the crates with no CEF, UI unit tests, docs build.** GUI checks stay as local scripts in `tools/` | A hosted Windows runner has no interactive desktop; the spike's input and capture checks need one. |
| D6 | Data location | `%LOCALAPPDATA%\dev_browser\` (global `browser.db`, `chromium-root\`, `workspaces\<id>\`) | Matches the spec's layout; keeps it out of roaming profiles. |

## 3. Repository layout

```text
Cargo.toml                 workspace
crates/
  core/                    domain and storage; no CEF. Workspaces, tabs, groups, settings, migrations,
                           and the capability-broker rules (scope resolution, caller identity). Unit-tested.
  host/                    the only crate that touches CEF: app, handlers, views, request contexts, panes,
                           render-process wrapper, process messages. Thin; logic lives in core.
  app/                     the binary: main(), command line, logging, wiring
ui/                        Preact shell (Vite). Builds to ui/dist, embedded in the binary.
tools/                     the spike's input, capture and frame-analysis scripts, promoted and tidied
spike/                     deleted when M1.1 landed (2026-11-13); last present at commit ac6960c
docs/  planning/           as now
```

Rules carried over from `AGENTS.md`: concrete structs first, traits only at real boundaries (the CEF adapter, test doubles);
standard library before dependencies; every dependency justified in `decisions.md`; docs change in the same commit as behaviour.

## 4. How it runs (the model every slice follows)

- **One UI thread.** The CEF UI thread is the main thread and the only one that calls CEF. Everything else talks to it with
  `post_task`.
- **Storage off the UI thread.** SQLite (rusqlite, as in `home_screen`) lives behind a small `Store` on its own thread, reached by
  channels. A slow disk never freezes the window.
- **Core decides, host executes.** `core` takes a command ("open tab in workspace W"), checks it against the caller and scope, writes the
  database, and returns what the host should do. `host` does the CEF work. That keeps S1, S2 and S10 testable without a browser.
- **A workspace is a profile.** Its request context is created asynchronously; **no view is created until the context reports it is
  initialised** (spike finding: otherwise `browser_view_create` returns nothing).
- **Views are cheap to discard.** A tab is a record in the database; its browser view exists only while needed and can be destroyed and
  recreated from the record (M0: destroy, recreate, mid-load and busy-page destroys all survived, with no leaks over many cycles).

## 5. Build order: slices

Each slice ends with something runnable, its acceptance checks, and updated docs. Sizes are relative (S small, M medium, L large).

### M1.0 Foundation (S)
Cargo workspace, `ui/` skeleton, `tools/`, a Windows build script that checks CMake, Ninja and the CEF export (the spike's `run.ps1`
lessons), format and lint settings, CI for the parts that can run there, and the Commands table in `AGENTS.md` filled in.
**Done when:** `cargo build`, `cargo test`, `npm test` in `ui/` and the docs build pass from a clean clone following the README.

### M1.1 One window, one tab, a trusted shell (M)
The CEF app skeleton with handlers split by concern (not one big file), one window, one page, and the shell view loaded from
`app://shell` with IPC v0 (a ping both ways) over process messages and the V8 binding. Production ports of the spike's window,
Panel-with-box-layout, dark browser background, DirectComposition switch and clean close (retry until every browser agrees).
**Done when:** the window opens, shows a shell strip and a page, one click on X closes it, and a hostile test page cannot see
`window.m0` (first S6 check). The spike is deleted.

### M1.2 Workspaces and storage (M)
`core`: IDs, workspace records, global and per-workspace databases, a migration runner with tests, and the data-location layout.
`host`: one request context per workspace with the path rule from the spike (a direct child of the root, in the same separator form as
`root_cache_path`) and the async-ready gate.
**Done when:** workspaces can be created, listed, renamed, coloured and deleted through IPC; each has its own profile folder; two
workspaces on one site keep separate logins (the M1 exit scenario, by hand); deleting a workspace removes its data.

**Status: done 2026-11-13** (see the M1.2 entry in `docs/ai/decisions.md`). Checked by 39 `core` unit tests, 10 shell tests and
`tools\m12-workspaces.ps1`, which drives the real sidebar with real input: a second workspace on the same site does not see the first's
cookie or localStorage, switching works, deleting through the UI works, one click on X closes the app, a restart keeps each workspace's
own login, and the deleted workspace's data is gone when the app exits. Two deliberate limits: deletion happens at exit (Chromium holds
the profile open until then), and the backend runs on the UI thread for now.

### M1.3 Tabs and view lifecycle (L)
Tab and group records; open, close, select, move; a pane model that holds a reference to a tab (one pane now, shaped for two);
create a tab's view on demand, destroy it on discard, recreate it from its record; focus and shortcut routing at window level so no
shortcut depends on a page view having focus (the native dialog swallowed `Ctrl+6` in M0). Includes the "visible view destroyed"
fall-back: pick another tab and focus a pane.
**Done when:** 200 open/close/discard/recreate cycles leave the process count flat, closing the visible tab never leaves a blank,
unresponsive window, and the keyboard works with focus in a page, in an iframe and in the shell.

**Status: done 2026-11-13** (see the M1.3 entry in `docs/ai/decisions.md`), apart from the pane model, which waits for split view (M4). Checked by
79 `core` and 14 `host` unit tests (the command registry, tab records, effects and the chord mapping) and `tools\m13-tabs.ps1`: shortcuts work
with the focus in a page, an iframe and the sidebar and no page ever receives one; 200 open / unload / recreate / close cycles left 7
processes at 7 and memory at +32 MB; the page area was never blank after closing the visible tab; tabs and the selected tab survive a
restart and a window close. The checks found and fixed a real leak (browser views must live in a wrapper panel).

### M1.4 Sidebar and workspace experience (M)
The Preact sidebar: workspaces, groups, tabs; name versus chevron; collapsed shows name and icon only (S11 as a UI test); accent colour
frame around the page; restore-last-tab per workspace; switching hides the old workspace first and shows a dark neutral gap, never old
content (S8). The spike's frame-capture analyser becomes the S8 regression check.
**Done when:** the S11 and S8 checks pass; switching between workspaces shows no frame of the previous one at 60 fps.

**Status: done 2026-11-13** (see the M1.4 entry in `docs/ai/decisions.md`). S11: collapsed workspaces are only a name and an icon, enforced at the
source (core sends tab data for expanded workspaces only) and in the UI (`Sidebar.test.tsx`). S8: `tools\s8-switch.ps1`, 9 switches at 60 fps
with no out-of-order frame, shown to fail when the old view is not hidden. Also done: the chevron and name behaviour, groups with one visual
order, the workspace accent frame (recolours live), and `tools\m14-sidebar.ps1` on the real window. Not done: drag and drop, persisted
expansion, per-group colours.

### M1.5 Navigation chrome (M)
Address bar and search-or-URL entry, back, forward, reload, stop, loading state, title and favicon, per tab, over IPC. Cross-workspace
open uses the destination's context, never the source's credentials (S10).
**Done when:** normal browsing works in two workspaces and S10 passes.

**Status: done 2026-11-13** (see the M1.5 entry in `docs/ai/decisions.md`). Address bar and search-or-address entry decided in core (`address.rs`, 11
tests, hostile inputs), back, forward, reload, stop and focus-address as commands with shortcuts, per-tab loading state, title and address over
IPC, favicons fetched in the tab's own context and gated by core, and `tabs.open_in`. `tools\m15-navigation.ps1` on the real window covers
browsing in two workspaces and S10 in both directions, and fails when the destination's profile is not used.

### M1.6 Safety nets (L)
Own error pages for load failures and renderer crashes; certificate errors shown, never bypassed (S12, minimal version); popup and
new-window policy (block by default, user-initiated allowed into the same workspace); **app-owned JavaScript dialogs and permission
prompts with safe defaults** (cancel and deny, with a notice; the real prompt UI is M4); a minimal download decision (save to a
chosen folder or cancel). The media decision store and the capture-stop wrapper are designed here and built in M4 (see section 7).
**Done when:** the M0 destroy-on-busy-page and permission checks pass against the production code, and no page can crash the app
with a dialog or prompt open.

### M1.7 Restore, exit test and hardening (M)
Persist and restore the window, workspaces and tabs after a restart or an unclean exit; the M1 exit scenario on a real site with two
real accounts; the S6, S8, S10, S11 and S12 checks run together; a short soak (hours open, many tabs); docs and the release page
updated; a tagged `0.2.0-alpha`.
**Done when:** the exit test passes on Roy's machine and the soak shows no growth in processes or memory beyond a stated bound.

## 6. What the spike taught us, and where it lands

| Finding from M0 | Where it goes |
| --- | --- |
| Request-context `cache_path` must be a direct child of the root and in the same separator form; views can only be created after the async init callback | M1.2 |
| Alloy-style views for every browser view; a Panel with a box layout re-installed to resize; hand-set bounds paint wrongly; children added in layout order | M1.1, M1.3 |
| A dark browser background colour replaces a white flash on a new view; hide the old workspace first | M1.4 |
| DirectComposition switched off avoids the GPU process crashing at start-up with several views (cause unknown) | M1.1, flagged for a second-machine test |
| Detach a view before closing its browser or the whole window closes; closing the visible view needs a fall-back and a focus target | M1.3 |
| Window close must retry until every browser agrees | M1.1 |
| CEF's own JS dialogs crashed the app when their view was destroyed: the app owns all dialog and prompt UI | M1.6 |
| Chrome queues a frame's media requests; media decisions are not remembered by CEF; revoking does not stop a live stream | design in M1.6, build in M4 |
| Render-process wrapper (pending-call, per-frame stop, repeated stop) plus a reload fallback; custom switches need `on_before_child_process_launch`; the renderer needs the app object | M4 |
| Per-workspace permissions are isolated for free by per-workspace profiles; the requesting origin can be an iframe's | M4 |
| Shortcuts must not depend on a page view having focus | M1.3 |
| A forced kill of the app can orphan a helper process | M1.7 (start-up sweep) |

## 7. Testing

- **`core`:** unit tests for every command, scope rule and migration; this is where S1, S2, S3 and S10 are enforced first.
- **`ui`:** unit tests for stores and components; the S11 check as a component test.
- **GUI checks (local, in `tools/`):** the spike's `SendInput` driver, window capture and frame analyser, tidied and documented, run
  before each slice is called done. They found three defects that logs and unit tests missed in M0, so they are part of the
  definition of done, not an extra.
- **Security matrix:** M1 owns S6, S8, S10, S11 and the minimal S12; the rest stay on their milestones.
- Every bug fixed gets a test or a tools check that would have caught it.

## 8. Risks

| Risk | What we do |
| --- | --- |
| The GPU start-up crash returns or differs on other hardware | Test on a second machine early in M1.1; keep the switch behind one function; record the cause if found |
| cef-rs lags a Chromium security release | The pin and upgrade checklist (D4); watch the cef-rs releases; re-run the tools checks on upgrade |
| Shell UI in web views is slower to build than native widgets | Start with the smallest sidebar; reuse `home_screen` components where they fit |
| Alloy style stops being supported | Keep the style choice in one function; CEF issue 3681 is the thing to watch |
| Windows-only habits creep into the code | Isolate platform code in `host`; no new Windows APIs outside it without a note |
| Scope creep from M2 to M4 ideas | The "Not in M1" list above; anything else goes to the roadmap, not the slice |

## 8a. Keyboard-first (decided 2026-11-13)

Roy: keyboard-first belongs in M1, it is an important part of the project. So it is a rule on every slice, not a late polish pass:

- **Every action is a named command** in `core` (id, label, default shortcut, whether it needs a page, a tab, a workspace). The shell,
  the shortcut handler and (in M2) the launcher all run commands from one registry. No action exists only as a mouse click.
- **Shortcuts are routed at window level**, in the host's pre-key-event handler on *every* view (shell and pages), so none depends on
  which view has focus (the native dialog swallowed `Ctrl+6` in M0). A page never sees a reserved chord; chords a page may want
  (`Ctrl+L`, `Ctrl+T`, ...) are reserved by the browser, as in other browsers, and the reserved list is documented.
- **Focus is always visible and always reachable**: visible focus rings in the shell, `F6` and `Ctrl+L` style moves between the sidebar,
  address bar and page, arrow keys inside the sidebar, `Esc` returns to the page. No keyboard trap.
- **A conflict table** (`docs/.../shortcuts.md`) is the source of truth; a unit test in `core` fails if two commands share a chord.
- **Each slice adds its commands and a key-only test** (the tools in `tools\` send real keys). M1.7's exit scenario is run once with
  the mouse unplugged in effect: open, switch, close, restore, create and delete a workspace by keyboard alone.
- **Not in M1:** the `Ctrl+P` launcher itself and user-rebindable shortcuts (M2); M1 ships fixed default chords.

Where it lands: M1.1 shows the shell strip receiving keys; **M1.3** introduces the command registry, window-level routing and the
reserved-chord list (it already owned shortcut routing); M1.4 and M1.5 add the sidebar and chrome commands with focus rings; M1.7 runs
the key-only walk-through.

## 9. Open items for Roy

1. ~~Confirm D1 and D3~~ Confirmed 2026-11-12.
2. ~~Keyboard-first in M1 or M2?~~ **M1** (2026-11-13): see section 8a.
3. ~~Tag and release?~~ **Keep unreleased for now** (2026-11-13). Version stays `0.2.0-alpha` in `Cargo.toml`; nothing is tagged or published until Roy says so.

## 10. First tasks (ready to start)

All nine were done by 2026-11-13.

1. Write D1, D2, D4, D5 and D6 into `docs/ai/decisions.md` (D3 once confirmed). **Done.**
2. Create the Cargo workspace and the three crates with an empty build and a passing test; add `ui/` with Vite and one test. **Done.**
3. Port the spike's `run.ps1` environment checks into a documented `tools/build.ps1`; fill in the Commands table in `AGENTS.md`. **Done.**
4. `core`: workspace and tab records, IDs, the migration runner and its first migration, with tests. **Done** (workspaces, ids, migrations, data directory; tab records arrive in M1.3).
5. `host`: the CEF app skeleton in modules (app, window, views, handlers), opening one window and one page. **Done.**
6. Serve `ui/dist` from `app://shell` and show a shell strip; IPC v0 ping over process messages and the V8 binding. **Done.**
7. The hostile-page test for `window.m0` (S6) as an automated check. **Done** (`tools\s6-hostile-page.ps1`).
8. Promote the spike's `drive`, `shot`, `flicker` and frame-analysis scripts into `tools/` with a README. **Done**, generalised rather than copied: `tools\lib\app.ps1`, `shot.ps1`, `record.ps1` and `analyze-frames.js`, documented in `tools\README.md`. The spike-specific permission, wrapper and destroy scripts were not ported (they belong to M1.6 and M4).
9. Delete `spike/` once tasks 5 to 7 are merged. **Done** (last commit that has it: `ac6960c`).
