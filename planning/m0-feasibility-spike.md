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
| 2 | Overlay | **Pass for behaviour, not yet seen by eye.** An overlay (`add_overlay_view`, custom docking) hosting its own browser view is created, shown and hidden over the live pages. A real mouse drag (`SendInput`) moved it by the expected steps (+10,+5 each, 12 steps) and the position is clamped to the window. Not checked: IME, real OS-level DPI (see below). |
| 3 | `Ctrl+P` | **Pass in the cases run** (real key input via `SendInput`, read from the log): intercepted with focus in a page text input, in the iframe test area, and in the other pane; the page never saw it (`on_pre_key_event` returned handled). `Escape` closes the overlay and focus returns to the pane that had it (A or B). Not checked: that no print dialog flashes (the key is consumed before the page, so it should not), pages with their own `Ctrl+P` handler, a focused PDF viewer, non-US keyboard layouts. |
| 4 | Two sessions | Two request contexts with distinct `cache_path` under one `root_cache_path`. Sign in to GitHub as two identities; both survive an app restart; cookies are not shared. | Both account names visible after restart |
| 5 | Two panes | **Pass, seen by eye (screenshots) and by log.** In Alloy style both panes, a draggable divider and the overlay work in one window. A real mouse drag of the divider resizes the panes smoothly (+150, -300, then -400 stopping at the 120px minimum). Clicks and `Ctrl+1` / `Ctrl+2` move keyboard focus to the intended pane. In Chrome style only the first browser view gets a browser. 20 screenshots over a few seconds settle to one steady frame. Window resize with the divider moved works, each pane has a workspace-coloured outline that follows focus, and the divider position is saved and restored (see below). Workspace swaps show no flash in the cases tried (see below). |

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

### Workspace swap flash (checked 2026-11-12)

The spike's stand-in for switching workspace is `Ctrl+3`: split, then only workspace A, then only workspace B, then split
(the other views are hidden and the layout re-flows). Pane A is a solid red page and pane B a solid blue one, so a frame of the
wrong colour layout is easy to spot. `spike/flash.ps1` records the client area at 60 fps with ffmpeg (gdigrab) while sending
the key, and `flash-analyze.js` classifies each frame's left and right halves.

| Run | Swaps | Frames | Unexpected frames |
| --- | --- | --- | --- |
| 3 runs, 1.8 s apart | 3 each | 479 to 540 each | 0 |
| rapid, 250 ms apart | 9 | 360 | 0 (states of about 16 frames) |
| very rapid, 90 ms apart | 12 | 300 | 0 (states of about 5 frames) |

Every frame was exactly split (red|blue), only A (red|red) or only B (blue|blue): no blank, black, white or mixed frame, and
no frame still showing the old workspace's colour after the switch. The analyser was sanity-checked on a synthetic capture and
flags a mixed frame (over 15% of a half wrong) and a black one.

**Limits.** (1) Both workspace views already existed and had painted before the first switch; the risky case is switching
to a view that is created or first shown at that moment (a stale GPU surface could show the old workspace's pixels), which
needs the real tab model. (2) A sliver under 15% of a half, or a flash shorter than the capture can see, would not be
flagged. (3) Screen capture shows what the compositor presented, which is what a user sees, but at 60 fps. (4) Pages were
solid colours, not real sites; tool panels and the widget panel are not in the spike. (5) No OS-level focus or
taskbar-preview leak check: the spec's "collapsed workspace shows no tab data" is a sidebar rule the spike does not have.

### Workspace outline and saved divider position (checked 2026-11-12)

- **Outline.** Each pane sits in a wrapper panel whose background is the workspace accent colour (A amber, B violet), showing as
  a 3px frame around the page. The focused pane's frame is the full colour; the other is the same colour dimmed. Screenshots
  show it, and that clicking the other pane swaps which frame is bright (log: `outline: focused pane N`). Frames
  are steady across a ten-frame series. Wrapper panels needed the browser view at flex 0 with its preferred width reported;
  with flex 1 the first pane took the whole window. Reminder from the spec: the outline is a visual cue, not a security boundary.
- **Saved divider position.** The split ratio is written to `.m0-data/divider.ratio` on every divider move (a stand-in for
  the global `browser.db` preference) and read at startup. `spike/persist.ps1`: run 1 dragged the divider +150px (ratio
  0.626, A=738 / B=440) and closed with a single click on X; the file held `0.6265`; run 2 logged `divider position loaded: 0.6265`
  and laid out A=738 / B=440 again, and its screenshot showed the wider left pane with both outlines. Not done: saving per
  workspace pair, or the other split-view state (which tab is in which pane); out-of-range or corrupt files fall back to 0.5.
- The earlier checks (`Ctrl+P`, `Escape`, pane focus, overlay drag, divider drag to the 120px minimum) still pass with the
  wrapper panels.

### IME (not run, 2026-11-12)

Not run: the test PC has only the English (UK) keyboard, so there is no input method editor to compose with, and adding one
is a Windows system change that was left to Roy. When one exists (Chinese Pinyin or Japanese), the check is: switch to it
for the spike window, type with real key input into the page input, the overlay input and the other pane, and confirm
composition shows in place, the candidate window sits next to the caret (also at 150% and inside the moved overlay), commit
lands in the right pane, `Escape` ends composition before closing the overlay, and `Ctrl+P` still opens the launcher mid-composition.
Spec risk 2 and the accessibility item stay open until then.

### 150% scale (checked 2026-11-12, with a caveat)

`spike/drive.ps1 -Scale 1.5 -AppFlags '--force-device-scale-factor=1.5' -SkipDivider` repeats the pane and `Ctrl+P` checks with
the window running at a 1.5 device scale factor (window 1800x1080 physical for 1200x720 logical, `devicePixelRatio` 1.5; click
offsets scaled to match). The log was the same as at 1.0: `Ctrl+P` intercepted from the page input, the iframe area and pane B,
`Escape` returned focus to the right pane, `Ctrl+1/2` and clicks moved focus, and the overlay drag moved in the same
10-logical-pixel steps. A screenshot showed both panes crisp and correctly sized.

**Caveat:** this is Chromium's own scale factor. The Windows display setting was not changed (that is a system setting), so
OS-level DPI scaling, per-monitor DPI changes when dragging the window between displays, and a window that is not
DPI-aware are **not** checked. Redo with Windows set to 150% (and a mixed-DPI pair of monitors) before calling DPI done.

### Resize with the divider moved (checked 2026-11-12)

`spike/resize.ps1` drags the divider with the real mouse (+150px), then resizes the window and reads the `[m0] relayout` lines
and screenshots. The split ratio held at 0.626 across three window sizes: 1184px wide gave A=738 / B=440, 884px gave A=550 /
B=328, and 1484px gave A=925 / B=553 (the 6px divider is excluded). Both pages reflowed to their new widths and the
screenshots showed no blank pane or overlap. Not checked: dragging the divider after a resize, minimum widths at very small
window sizes, 150% DPI.

### What looking at it found (2026-11-12, after the log-only passes above)

Roy ran it and saw a blank left pane, flicker (the left page jumping over the right one) and a window that needed two
clicks on X. Screenshots of the spike's own window (`spike/shot.ps1`, `spike/flicker.ps1`) reproduced the first two.

- **Blank left pane / flicker.** Giving the panes and divider their position by hand (`set_bounds` in a panel with no
  layout) painted wrongly: the first-added pane showed only its background colour (no text), and in another order the
  left page's content flickered over the right pane about half the time. A box layout on the panel fixed both. Resizing
  needs the layout re-installed with new sizes; flex weights alone were ignored for browser views, so the panes report the
  wanted width as their preferred size (`SpikePaneBvd`) and use flex 0. Cause of the hand-positioning problem not found.
- **Two clicks to close.** Each browser answers `try_close_browser` with "not yet" the first time and CEF retries the
  window close for only one of them. The spike now retries the window close itself every 150 ms until all agree; one click
  on X closes it (checked with a real mouse click). This also fixed the earlier Alloy hang on exit.
- **GPU process crashes at startup (investigated).** The log showed the GPU process exiting unexpectedly three times in
  the first second (exit code 0x80000003, a debug-break), then running on the fourth start. Findings, each from runs
  counting the "GPU process exited" log lines:
  - It needs two or more content browser views in the window: one pane (with or without the overlay) never crashes; two
    panes crash with or without the divider or overlay, with one shared profile or two, and in Chrome style too.
  - It goes away with `--disable-direct-composition`, `--disable-gpu-compositing`, `--use-angle=swiftshader`, `--use-angle=gl`
    or `--in-process-gpu`; it stays with the default, `--use-angle=d3d11`, `--disable-gpu-sandbox` and
    `--disable-features=DirectComposition`. So it is in the D3D11 plus DirectComposition path with several child windows.
  - **Workaround in the spike:** `--disable-direct-composition` (set in `on_before_command_line_processing`, opt out with
    `--keep-dcomp`). Crashes 3 to 0, ANGLE D3D11 still initialises, and the screenshot series is steady from the first frame
    (before the fix it changed once about two seconds in). Not found: the root cause (the crash prints nothing and no dump
    is written), whether it depends on this machine (NVIDIA, two displays, one at a negative x), or what the flag costs.
  - To do before relying on it: try another machine or GPU, and measure the cost of no DirectComposition (power, tearing).
- **Lesson:** log lines said "pass" while the screen was wrong. Keep the screenshot check (`flicker.ps1`) in the loop.

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
- **Divider.** Views has no splitter. The spike uses a 6px third browser view as the divider (it reports mouse drags
  through the page title, like the overlay) in a `Panel` with a box layout that is re-installed on each change (see above).
  A native-looking handle can come later; this proves the mechanism.

### Next steps for M0

1. Look at the window once by eye (overlay position and look, no print dialog, no flashes) and try 150% DPI and a resize.
   Check resizing the window with the divider moved, and persist the divider position. Input automation lives in `spike/drive.ps1` (it takes over the mouse and keyboard while it runs).
2. Fix Alloy shutdown; try a real login cookie flow (not just localStorage) for test 4.
3. List which Chrome-only features we lose with Alloy-style views (downloads, permissions, printing, extensions) and
   keep the style choice behind one function so a CEF change is a small edit.
4. Time-box candidate B (Tauri with CEF) against the same five tests only if A cannot reach a pass.
