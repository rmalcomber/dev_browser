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

Flags: `--url-a= --url-b= --single --chrome --selftest-write --exit-after=SECS`. Alloy style is the default (it is the one that
supports several browser views in one window); `--chrome` selects Chrome style. Output lines start with `[m0]`.
`cwd` matters: Chromium data goes in `.m0-data/` under the current directory (git-ignored).

Keys in the window: `Ctrl+P` toggles the overlay, `Esc` closes it, `Ctrl+1` / `Ctrl+2` focus a pane.

`drive.ps1` launches the spike and sends real mouse and keyboard input (overlay drag, `Ctrl+P` from several focus states, `Escape`,
`Ctrl+1/2`, pane clicks), then prints the `[m0]` log. It takes over the mouse and keyboard for about 30 seconds: leave them alone.
`testpage.html` is the page with a text input and an iframe that it loads in pane A.
