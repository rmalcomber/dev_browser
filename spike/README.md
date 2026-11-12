# M0 spike (disposable)

Feasibility spike for candidate A: direct CEF through the [tauri-apps/cef-rs](https://github.com/tauri-apps/cef-rs) bindings
(Apache-2.0 OR MIT). It is **not the product** and nothing here is promoted to `src/`. Plan and exit criteria:
[`../planning/m0-feasibility-spike.md`](../planning/m0-feasibility-spike.md). Results so far are in that file.

## Build and run (Windows x64)

Needs Rust, Visual Studio 2022 C++ tools, CMake and Ninja on `PATH`, and a CEF 156.0.28 "minimal" Windows x64 build in `CEF_PATH`
(default `~/.local/share/cef`). The `cef-dll-sys` build script cannot fetch it itself right now: the CDN `index.json` exceeds
the 10 MB response limit in `download-cef` (also on cef-rs master), so export it with a local one-line patch that raises the
limit (`.with_config().limit(256 * 1024 * 1024)` before `read_json()`) and run `cargo run -p export-cef-dir -- --force <dir>`.

```powershell
./run.ps1 --exit-after=20            # builds, copies the CEF runtime beside the exe, runs
./run.ps1 --selftest-write           # pane A writes a localStorage marker (test 4 probe)
```

Flags: `--url-a= --url-b= --single --chrome --noov --nodiv --keep-dcomp --sharectx --mem-b --selftest-write --exit-after=SECS`. Alloy style is the default (it is the one that
supports several browser views in one window); `--chrome` selects Chrome style. Output lines start with `[m0]`.
`cwd` matters: Chromium data goes in `.m0-data/` under the current directory (git-ignored).

Keys in the window: `Ctrl+P` toggles the overlay, `Esc` closes it, `Ctrl+1` / `Ctrl+2` focus a pane.

`drive.ps1` launches the spike and sends real mouse and keyboard input (overlay drag, `Ctrl+P` from several focus states, `Escape`,
`Ctrl+1/2`, pane clicks, divider drags; `-Scale 1.5 -AppFlags '--force-device-scale-factor=1.5' -SkipDivider` for the 150% run), then prints the `[m0]` log. It takes over the mouse and keyboard for about 30 seconds: leave them alone.
`testpage.html` is the page with a text input and an iframe that it loads in pane A.

`shot.ps1` saves a screenshot of the spike's window (it makes the window topmost first so the picture holds only the spike).
`flicker.ps1` takes a series of screenshots and counts distinct frames: 1 means steady. Both take extra app flags as one string,
e.g. `-AppArgs '--url-a=data:text/html,<body%20style=background:red>'`. Keep the images out of the repo.
`resize.ps1` moves the divider with the mouse, then resizes the window and prints the `[m0] relayout` lines (add `-OutDir` for screenshots).
`persist.ps1` checks the saved divider position: it drags the divider, closes with one click on X, restarts and prints the loaded ratio.
The split ratio lives in `.m0-data/divider.ratio` (delete it to reset to 50/50).
`Ctrl+3` cycles split / only workspace A / only workspace B (the spike's stand-in for switching workspace). `flash.ps1` records the
window at 60 fps with ffmpeg while pressing it and `flash-analyze.js` reports any frame that is not a clean split or single-workspace
layout (`-Presses` and `-GapMs` set how many swaps and how fast). Needs ffmpeg and node on PATH.
`Ctrl+4` creates workspace C on the spot and shows it alone; `flash.ps1 -Fresh` checks that swap (add `-AppFlags '--white-bg'` to see the white
flash a new view shows with the default browser background).
`Ctrl+6` destroys workspace C's view (keeping its context) and `Ctrl+4` recreates it; `flash.ps1 -Recreate` checks both, including destroying the
view while it is on screen.
`midload.ps1` destroys workspace C's view while its page is still loading (`slow-server.js` answers after 3 s) and checks the app survives,
the request is aborted, the frames are clean and no processes leak. Needs node and ffmpeg on PATH.
`busy.ps1` destroys workspace C's view while its page is busy (spinning script, event stream, WebSocket, download, alert dialog, popup) and
reports survival, processes and what the server saw (`-Routes`, `-AppFlags`, `-AutoDestroyMs`). Needs node on PATH.
`busy.ps1 -Routes geo,notify,clipboard,camera` (with `-AppFlags '--own-permissions'` and optionally `--drop-permission`) destroys the view with a permission prompt open.
`permanswer.ps1` has the app accept or deny each permission and checks the decision survives destroy/recreate and restart, and does not leak to
the other panes' profiles. Needs node on PATH. The camera accept runs open a real webcam for a few seconds each; use `-Kinds geo,notify,clipboard` to avoid that.
`mediastore.ps1` checks the app-owned camera/microphone decision store (ask, remember, destroy, restart, partial coverage, other workspace, revoke).
It uses Chromium's fake capture devices, so no real camera or microphone is opened.
`revoke.ps1` checks granting a subset of the requested media kinds and revoking while a stream is live (fake devices; no real camera or microphone).
`wrapper.ps1` checks the render-process getUserMedia wrapper that stops a running stream from the browser (`-Only W5` etc. runs one scenario).
Fake capture devices; no real camera or microphone is opened.
`wrapper.ps1 -Only "X[1234]"` runs the cross-origin and cross-site iframe stop checks (the test server also listens on port+1 and on ::1 so `localhost` is a second site).
`wrapper.ps1 -Only "N[123]"` runs the nested, late-added and srcdoc/sandboxed iframe stop checks.
`wrapper.ps1 -Only "N[45]"` checks frames navigating mid-stream; `-Only G -Sweep grant-once|grant-repeat` and `-Only S -Sweep once|repeat` sweep the revoke against a navigation or a grant to show the lost-stop race and the repeated-stop fix.
`wrapper.ps1 -Only "(L|R)" -Sweep level-full|sticky|pending` compares the wrapper variants against a stop that races a grant and against a retry the user allowed; `-Only D1 -Sweep slow-full|pending` uses a grant delivered 4 s late.
