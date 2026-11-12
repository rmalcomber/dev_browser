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

**Limits.** (1) Both workspace views already existed and had painted before the first switch; the freshly created case is
checked in the next section. (2) A sliver under 15% of a half, or a flash shorter than the capture can see, would not be
flagged. (3) Screen capture shows what the compositor presented, which is what a user sees, but at 60 fps. (4) Pages were
solid colours, not real sites; tool panels and the widget panel are not in the spike. (5) No OS-level focus or
taskbar-preview leak check: the spec's "collapsed workspace shows no tab data" is a sidebar rule the spike does not have.

### Swap to a freshly created view (checked 2026-11-12)

The case the first swap check left open. `Ctrl+4` creates workspace C on the spot (a new request context with its own cache path,
then a browser view, then it loads a solid green page) and shows it alone; the old workspaces are hidden at once, never left up
while the new one loads (the spec's screen-sharing rule). `flash.ps1 -Fresh` records 60 fps while doing split -> new C -> split -> C again.

| Case | Frames between the last old frame and the new content | What they were |
| --- | --- | --- |
| New view, default browser background | 4 | 1 dark, then **3 white** (about 67 ms): a white flash |
| New view, dark browser background (the spike's default now) | 2 to 5 (33 to 83 ms, three runs) | dark only |
| Switching back to an existing view | 0 | straight to the content |
| New view loading a real site (example.org) | none stale | dark throughout; no red or blue (old-workspace) frame after the switch |

So in no run did a frame of the previous workspace's content appear after the switch, which is the privacy-relevant part. The
real flash was white: a new view's native window shows white until its first paint. Giving the browser a dark
`background_color` (matching the app) turns that gap into a few dark frames. The sequence is: old content, a short dark gap,
new content. Keep that behaviour (hide old first, neutral background) in the real tab model, and re-run this check there.

Still open: a view that is destroyed and recreated rather than created once; lazily restored tabs; switching while a view is
mid-load; and sites that paint white themselves (their own white page is their content, not our flash).

### Swap to a destroyed and recreated view (checked 2026-11-12)

`Ctrl+6` destroys workspace C's browser and view (a discarded tab) but keeps its request context, so its session survives;
`Ctrl+4` then recreates only the view. `flash.ps1 -Recreate` records 60 fps through: create C, back to split, destroy C while
hidden, swap to C (view recreated), destroy C while it is on screen, then swap to C again. Five runs at 60 fps:

| Transition | Frames in between | What they were |
| --- | --- | --- |
| First creation of C | 3 to 6 | dark |
| Recreated while hidden, then swapped in | 0 to 2 (three of five runs had 1 or 2) | dark |
| Visible view destroyed, app falls back to split | 0 | straight to the split view |
| Recreated from split again | 0 or 1 | dark |

No frame of the previous workspace's content appeared after any switch and no white frame appeared (the dark background from
the previous section holds for recreated views). Recreating is no worse than first creation, and usually faster, because the
request context is already initialised.

Two real bugs found on the way, both in the spike's own handling and both worth designing for in the real app:

1. **Closing one view's browser while it is still in the window closed the whole window**, every browser, then the app.
   Detach the view from its parent first, then close its browser.
2. **Destroying the focused, visible view left nothing to receive key events**: the window stayed blank and `Ctrl+4` did
   nothing. When a visible workspace is discarded the app must pick another workspace and give a pane the focus.

Not checked here (done in the next section: mid-load destroys and create/destroy cycles for leaks): a destroyed view's
request context being released, and a one-off `mixed|G` state seen for the last 1.6 s of an earlier, pre-fix run that did not
recur in the five clean runs (cause unknown; possibly the capture, not the app).

### Destroying a view mid-load, and leaks (checked 2026-11-12)

`spike/midload.ps1` runs a local page server (`slow-server.js`) that answers after 3 s, points workspace C at it, and destroys C's view
700 ms into the load; then recreates it and lets it finish; then does destroy/recreate pairs only 120 ms apart. It records 60 fps and
logs what the server saw.

- **App survived** every case: no panic, no hang, no stuck window; the browser list returned to four after each destroy.
- **The load is cancelled.** The server logged each in-flight request as `aborted (client went away before the response)`
  about 0.6 s after the destroy, including the two 120 ms pairs, and only the loads that were allowed to finish were `sent`. The
  favicon requests were aborted too.
- **The kept context still works.** After a mid-load destroy, recreating C loaded the page normally (server: `sent`, frames:
  dark for the 3 s load, then green).
- **Frames:** the sequence was split, dark while C loads, split again after the destroy (straight from dark to the split view,
  no stale or white frame), dark while the recreated C loads, green when the page arrives. Only red|blue, dark and green|green
  states appeared. The dark period lasts as long as the load, so the real app needs a loading state there.
- **No leak in 16 create/destroy cycles:** the process count was 9 at rest and 10 only while C existed, back to 9 after every
  destroy (it counts the browser, GPU, network and renderer processes), and 0 after closing the app. Total private memory went
  251 MB (split view), 268 MB after the first destroy, then 273 MB after 15 more cycles and 274 MB at the end: flat to within
  a few MB, so no growth per cycle that this measure can see.

Not checked: a destroy while a page is running script or holding a connection open (websocket, download, media), a destroy of
a view that has an open popup or permission prompt, and memory with real sites (the pages here are tiny).

### Destroying a view on a busy page (checked 2026-11-12)

`spike/busy.ps1` runs six scenarios against `slow-server.js`: workspace C loads the page, the page goes busy, C's view is destroyed
(`Ctrl+6`), then C is recreated from its kept context. It counts processes, reads the server's log of what the connection did, and
checks the app survived.

| Page | What it was doing | Survived | Processes before / busy / after | Server saw |
| --- | --- | --- | --- | --- |
| `/busy` | `while (true) {}` in the page script | yes | 9 / 10 / 9 | requests finished (page itself is static) |
| `/sse` | endless event stream | yes | 9 / 10 / 9 | event stream closed by the client |
| `/ws` | open WebSocket | yes | 9 / 10 / 9 | `ws closed (client went away)` |
| `/download` | endless download started by script | yes | 9 / 10 / 9 | file request closed by the client |
| `/alert` | `alert()` open and an `onbeforeunload` handler | **crashed with CEF's own dialogs; yes once the app answers dialogs** | 9 / 10 / 9 | request finished |
| `/popup` | `window.open` on load | yes | 9 / 10 / 9 | only the opener was requested (popup not opened) |

A spinning script does not stop the destroy (the renderer process is killed and the count returns to 9). Every connection
the page held was closed from the client side, and the recreated view worked every time.

**The one real bug.** Destroying a view while an `alert()` was open, with CEF's own JS dialogs, crashed the whole app: an
access violation inside `libcef.dll` (+0x8ddbd5b, from the crash dump). Reproduced with `--native-dialogs`. The spike now answers
JS dialogs itself (`SpikeDialogs`: cancels alert and confirm, allows leaving on `beforeunload`), and with that the same destroy
is clean. Two stricter variants also survived: `--hold-dialog` keeps the dialog callback pending like a custom dialog would,
then destroys the view after cancelling it, and `--hold-dialog --drop-dialog` destroys it with the callback released without an
answer. Design consequence for the real app: **the browser owns all dialog UI** (alert, confirm, prompt, beforeunload, and later
auth, file picker and permission prompts) and must resolve or release any pending dialog when its view is destroyed. A dialog
that takes the keyboard also swallowed `Ctrl+6` with the native dialog, so shell shortcuts must not depend on the page's view
having the focus.

Not checked: confirm and prompt dialogs (only alert and beforeunload), a real custom dialog UI, authentication dialogs, file
pickers, permission prompts, downloads with a download handler and UI, media or WebRTC in the page, service workers, and
popups the app chooses to allow.

### Destroying a view with a permission prompt open (checked 2026-11-12)

Same harness (`busy.ps1`, `-Routes geo,notify,clipboard,camera`) with pages that ask for geolocation, notifications, clipboard read,
and camera plus microphone, three ways: CEF's default handling, `--own-permissions` (the app takes every prompt and keeps its
callback pending, as a browser-owned prompt UI would; on destroy it dismisses them), and `--own-permissions --drop-permission`
(the callbacks are released without an answer). The view is destroyed from a timer while the prompt is open.

| Request | Bits the app saw | Default | App-held, dismissed on destroy | App-held, dropped |
| --- | --- | --- | --- | --- |
| geolocation | `0x100` | survived | survived | survived |
| notifications | `0x8000` | survived | survived | survived |
| clipboard read | `0x10` | survived | survived | survived |
| camera + microphone | `0x3` (media path) | survived | survived | survived |

Every case survived, recreated the view fine and left the browser list at four. Camera and microphone arrive through a different
callback (`on_request_media_access_permission`, cancelled with `cancel()`) from the other three (`on_show_permission_prompt`,
answered with `PermissionRequestResult::DISMISS`); a browser-owned prompt UI needs both. After a dismiss CEF still reports the prompt as
dismissed, and after a drop it dismisses it itself.

One side effect worth knowing: **a camera or microphone request starts helper processes that outlive the view.** The count went
from 9 to 12 with the request pending and settled at 11 after the destroy, stayed at 11 for over a minute, did not grow over five more
create/destroy cycles, and went to 0 on a normal close (one click on X). The two extra processes are media services (audio capture
and video capture). When the app's own processes were force-killed, an orphaned `--type=utility` AudioService process survived the
parent, so the real app should not rely on children dying with a crashed parent (look at job objects and a start-up sweep).

Not checked: what CEF's default handling shows or does with these prompts (no screenshot of a native prompt was taken; the
default run only proves it does not crash), real accept and deny paths (`PermissionRequestResult::ACCEPT` and the stored
decision), per-origin persistence, permission prompts in the second pane while a different workspace is on screen, and prompts
from iframes.

### Accepting and denying permissions (checked 2026-11-12)

`spike/permanswer.ps1` has the app answer each prompt itself (`--answer-permissions=accept|deny`) and records what the page sees. For
each of geolocation, notifications, clipboard read, and camera, and for each answer, it does: **visit 1** (C asks), **visit 2** (C's view
destroyed and recreated, asks again), a graceful close and restart, **visit 3** (asks again; a new prompt would be held and logged).
Panes A and B load the same origin in their own profiles and only query the state. The page reports to the test server.

| Permission | Accept: page sees | Deny: page sees | Asked again on visit 2 / 3? | Survives restart? | Leaks to A and B? |
| --- | --- | --- | --- | --- | --- |
| geolocation | state `granted`; request ends in error code 2 (no location data on this PC, permission itself was granted) | `denied`, error code 1 | no / no | yes (granted and denied) | no (`prompt`) |
| notifications | `granted` | `denied` | no / no | yes | no |
| clipboard read | `granted`, `readText()` works | `denied`, `NotAllowedError` | no / no | yes | no |
| camera | `getUserMedia` returns a stream (a webcam is attached) | `NotAllowedError` | **yes / yes** | **no** | no (`prompt`) |

What this shows:

- **The prompt reaches the app exactly once** for the three ordinary permissions, with the answer applied: accept gives a working API,
  deny gives the standard rejection. CEF then remembers the decision in that workspace's profile: no second prompt after a destroy
  and recreate, and none after a restart, for both grants and denials.
- **Permissions are per workspace.** The same origin in panes A and B (other profiles) stayed at `prompt` in every run, so a grant in
  one workspace does not reach another. This is the isolation the spec asks for, and it comes free with a profile per workspace.
- **Camera and microphone are different: CEF does not remember them.** They come through the media callback; the permissions API
  stays at `prompt` after an accept, and the app was asked again on every visit and after every restart. So the app must store media
  decisions itself, per workspace and origin, and answer from that store; and show the prompt UI each time otherwise.
- **Disclosure:** the accept runs for camera really opened this PC's webcam for a few seconds per visit (the stream was acquired and
  never displayed or recorded). Rerun it only where that is acceptable.
- Panes A and B reached the same results with `--own-permissions` after restart, i.e. nothing was silently granted by the restart.

Not checked: the revoke path (changing a stored decision), per-origin lists in a settings page, `ACCEPT` for permissions beyond
these four (MIDI, sensors, payment, storage access), prompts from iframes (permissions policy), a permission asked while another
workspace is on screen, microphone alone, and what the UI should say for a permission the OS itself blocks.

### An app-owned store for camera and microphone decisions (checked 2026-11-12)

CEF does not remember camera and microphone answers (previous section), so the spike keeps its own: one small file per workspace
(`.m0-data/workspaces/<ws>/media-permissions.txt`, a stand-in for the workspace's database) with `origin, allowed bits, denied bits`.
On a media request the app looks up the origin in the asking workspace's store, asks only about the kinds not yet decided (the
"user" is `--answer-permissions`), remembers the answer, and answers CEF: grant when every requested kind is allowed, otherwise
cancel. `spike/mediastore.ps1` uses Chromium's fake capture devices (`--use-fake-device-for-media-stream`; the page sees
`fake_device_0`), so **no real camera or microphone was opened** in these runs.

| Step | What the app did | Page got |
| --- | --- | --- |
| 1. first visit, C and pane B (same origin, other workspace) both ask for the camera | asked the user twice (once per workspace), accepted, stored in `ws-c` and `ws-b` separately | a stream each |
| 2. C's view destroyed and recreated | answered from the store, no question | a stream |
| 3. restart, the user would now deny | answered from the store: the store wins | a stream, in both C and B |
| 4. C asks camera plus microphone | camera from the store, asked only about the microphone, the user denied it, remembered (`allowed 2, denied 1`) | `NotAllowedError` (all or nothing) |
| 5. revoke: `ws-c`'s store cleared at start-up | asked again, this time denied (`0, 2`); `ws-b` untouched and still answered from its own store | `NotAllowedError` in C, a stream in B |

So the pattern works: decisions are per workspace and origin, survive destroy and restart, partial coverage asks only about the
new kind, and a revoke is just deleting an entry. Things to carry into the real app: (1) the page's own `permissions.query` still says
`prompt` after a grant because CEF does not know about the store, so a site that checks the Permissions API before asking sees the
wrong state; (2) a request for both kinds is refused as a whole when one is denied (this run did not try granting a subset);
(3) revoke needs UI (a per-origin list) and should also stop a stream already running, which this check did not do; (4) the file
format here is only a stand-in and must live in the workspace database, never in a shared one.

Not checked: granting a subset, revoking while a stream is live, microphone-only pages, screen and window capture (the desktop
bits), the permission being asked from an iframe, expiry of decisions, and the real prompt UI.

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
