# tools

Scripts for building, running and checking The Dev Browser. Windows only for now (decision D3). Run them from the repository root.

## Build setup (once)

You need Rust, the Visual Studio 2022 C++ build tools, CMake and Ninja on `PATH` (`winget install Kitware.CMake Ninja-build.Ninja`),
Node for `ui/` and the test server, and a **CEF 156.0.28 "minimal" Windows x64 build** in `CEF_PATH` (default `~/.local/share/cef`).

The `cef-dll-sys` build script cannot download CEF by itself right now: the CDN's `index.json` is bigger than the 10 MB response
limit in `download-cef` (still true on cef-rs master). Export CEF yourself with a local one-line patch to the pinned cef-rs checkout:
in `download-cef`, raise the limit with `.with_config().limit(256 * 1024 * 1024)` before `read_json()`, then run
`cargo run -p export-cef-dir -- --force <dir>` and point `CEF_PATH` at `<dir>`. (Background: `docs/ai/decisions.md`, D4, and
`planning/m0-feasibility-spike.md`.)

Always spell `CEF_PATH` with forward slashes: it is an input of the `cef-dll-sys` build script, and a different spelling of the same
folder forces a full rebuild. `build.ps1` does this for you.

## Build, test, run

| Script | What it does |
| --- | --- |
| `build.ps1 check` | `cargo fmt --check`, clippy (warnings are errors), all Rust tests, ui tests, ui build. |
| `build.ps1 test` | Rust and ui tests only. |
| `build.ps1 build` | ui build, `cargo build`, copies `ui/dist` and the CEF runtime beside the exe in `target\debug`. |
| `build.ps1 run [-- args]` | `build`, then starts the app. Useful flags: `--data-dir=PATH` (a throw-away profile), `--url=URL`, `--keep-dcomp`. |

## Checks that drive the real window

These start the built app on a throw-away data folder, send **real** mouse and keyboard input, and take real pictures. They need an
unlocked interactive desktop: **leave the mouse and keyboard alone while one runs.** The app is always closed afterwards.

| Script | What it does |
| --- | --- |
| `s6-hostile-page.ps1` | S6: a hostile page cannot see `window.m0` or reach `app://shell`. Needs node. Exit code 0 = pass. |
| `m32-panel.ps1` | M3.2 checks: pin a tool from the launcher with the keyboard, the panel opens and the page ends where it begins, `Ctrl+Shift+B` hides and shows it (and focuses the tool), a token pasted into the widget decodes and copies, a real drag resizes it, the layout survives a restart, and the token is in no database or log (S9). About 2 minutes; needs node. |
| `m31-tools.ps1` | M3.1 checks: a real triple-click selects a token, `Ctrl+P` captures it, the JWT decoder starts from it and Copy puts a chosen claim on the clipboard, the UUID tool, nothing captured over another tab, and the token in no database or log line (S9). About 2 minutes; needs node. |
| `m23-entries.ps1` | M2.3 checks: saving entries from the launcher (with an environment), finding and opening one, ranking by how often a result was opened, and clearing history resetting the order. About 2 minutes; needs node. |
| `m22-places.ps1` | M2.2 checks: a page found in history, `Ctrl+D` and the bookmark found, clearing history keeps the bookmark, and a bookmark of another workspace is not found until "All workspaces" is chosen. About 2 minutes; needs node. |
| `m21-launcher.ps1` | M2.1 checks: `Ctrl+P` from a page, a frame and the address bar, `Esc`, a real mouse drag, scope (a page in another workspace is not found until "All workspaces" is chosen, and the wide scope is gone after closing), and running a command. About 2 minutes; needs node. Clicks the launcher at fixed positions; see the script. |
| `m17-restore.ps1` | M1.7 checks: the window size comes back after a clean close; after the app is killed, tabs, the selected tab and each workspace's separate login are back. About 3 minutes; needs node. |
| `soak.ps1 -Minutes N` | Opens tabs in two workspaces and churns them for N minutes, sampling processes and memory once a minute; fails above 22 processes or 40% growth. Needs 8+ minutes to judge growth. |
| `m16-safety.ps1` | M1.6 checks: eight kinds of risky page closed while active (dialogs, permission prompts, `beforeunload`, an endless loop), popups, an error page and Reload, S12 (self-signed https; no request reaches the server), downloads (Save, Cancel, no overwrite) and killed renderer processes. About 3 minutes; needs node and openssl. |
| `m15-navigation.ps1` | M1.5 checks: typing an address and a search, a link click, Back, Forward, Reload, `javascript:` refused, Stop on a slow load (with a control), the page icon, browsing in a second workspace, and S10 in both directions. About 2 minutes; needs node. |
| `m14-sidebar.ps1` | M1.4 checks: every page framed in its workspace colour (sampled from a screenshot), the frame follows switches and recolouring, the chevron expands without switching, clicking another workspace's tab switches to it, the group shortcuts run. Needs node. |
| `s8-switch.ps1` | S8: three workspaces in three colours, 9 `Alt+PageDown` switches recorded at 60 fps; frames must go round in order with none of the previous workspace coming back, then new-tab and close-tab churn. Needs ffmpeg and node. |
| `m13-tabs.ps1` | M1.3 checks: shortcuts work with the focus in a page, an iframe and the sidebar and never reach a page; `-Cycles 200` open / unload / recreate / close rounds keep the process count flat and the page never blank; a page that closes itself; tabs and the selection survive a restart. About 6 minutes. Needs node. |
| `m12-workspaces.ps1` | M1.2 exit scenario: makes and deletes a workspace through the real sidebar and shows two workspaces on one site keep separate logins, which persist across a restart, and that deleted data is gone at exit. Needs node. |
| `shot.ps1 -Out file.png` | Start the app, optionally `-Click x,y` and `-Keys "ctrl+3"`, save a picture of the window (`-ClientOnly` for the page area). |
| `record.ps1 -Keys "ctrl+3@1500"` | Record the client area at 60 fps while sending timed keys, then print every colour state the page area went through (`analyze-frames.js`; the sidebar has live content, so it is not watched). `-Expect "R,B"` makes any other frame a failure. The engine of the S8 check. Needs ffmpeg and node. |
| `lib\app.ps1` | Shared helpers: `Start-App` (`-ReuseDir` restarts on the same data), `Send-Keys` (`ctrl+shift+tab`, `alt+1`, `f6`, `ctrl+.`), `Send-Text` (letters, digits and common punctuation; US keyboard layout), `Click-Client`, `Close-App`, `Save-Window`, `Wait-Log`, `Start-TestServer`, `Start-Recording` / `Wait-Recording`. `Send-Keys` and `Click-Client` bring the app window back to the foreground first if something else took it, and `Get-FocusRestores` says how often. Dot-source it from a new script. |

`-Page colours.html` (on `shot.ps1` and `record.ps1`) serves a solid-colour page from `test-pages\` over http through
`test-server.js`; pressing `1`, `2` or `3` turns it red, blue or green, which is how the tools prove that keys arrive and that the frame
analyser sees changes. Keep pictures out of the repository.

## What was left behind with the spike

The M0 spike (`spike/`) was deleted once M1.1 landed. The last commit that still has it is `ac6960c`. The destroy-while-busy and permission checks were rebuilt against the production code in M1.6 (`m16-safety.ps1`). The media permission store, the
`getUserMedia` wrapper and the revoke checks (`mediastore`, `revoke`, `wrapper`) are not ported: they belong to M4. Read them with
`git show ac6960c:spike/<name>`.
