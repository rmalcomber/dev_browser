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
| 2 | Overlay | A browser-owned draggable panel renders above a live page; dragging is smooth; works at 100% and 150% DPI and after resize. IME and focus noted. | Screen capture |
| 3 | `Ctrl+P` | Opens our overlay, never the print dialog, while the page, an input, and an iframe have focus. `Escape` returns focus to the prior element. | Pre-key-event handler notes |
| 4 | Two sessions | Two request contexts with distinct `cache_path` under one `root_cache_path`. Sign in to GitHub as two identities; both survive an app restart; cookies are not shared. | Both account names visible after restart |
| 5 | Two panes | Two views side by side with a draggable divider; click and keyboard focus go to the right view; `Ctrl+1/2` focus switch works; no flash of the wrong view when swapping. | Screen capture |

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

Code is in `spike/` (see its README). Checked three ways: the `[m0]` log lines, real mouse and keyboard input sent with `SendInput`, and
screenshots or 60 fps captures of the spike's own window (`spike/shot.ps1`, `flicker.ps1`, `flash.ps1`). Where something was only
checked one of those ways, the row says so. Details and the evidence for each line are in the sections below.

| # | Test | Result |
| --- | --- | --- |
| 1 | Embed and navigate | **Pass.** Window opens, pages load, one click on X closes cleanly. |
| 2 | Overlay | **Pass.** A browser-owned overlay view hosts its own page above the live panes, shows and hides, and a real mouse drag moves it in the expected steps and clamps it to the window. At a 1.5 device scale factor it behaves the same. Not checked: IME (no IME installed), real OS-level DPI scaling and mixed-DPI monitors. |
| 3 | `Ctrl+P` | **Pass.** Intercepted with focus in a page input, an iframe and the other pane; the page never sees it; `Escape` restores focus to the right pane; same at 1.5 scale. Not checked: pages with their own `Ctrl+P` handler, a focused PDF viewer, non-US layouts, a print dialog flashing (the key is consumed first). |
| 4 | Two sessions | **Pass in Alloy style.** One request context per workspace with its own profile directory; a value written in A survives a restart and never reaches B; permissions and the camera and microphone store are per workspace too. The two bugs that made it look blocked were a path-separator mismatch and creating the view before the asynchronous profile was ready. Real logins (cookies) were not tried. |
| 5 | Two panes | **Pass.** Two panes, a draggable divider with a saved position, workspace-coloured outlines that follow focus, resize with the divider moved, and workspace swaps (existing, new, destroyed-and-recreated views) with no flash at 60 fps once a dark browser background replaced a white first frame. Chrome style only hosts one browser view per window; Alloy style, which CEF supports for this, hosts several. |


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

Not checked here: microphone-only pages, screen and window capture (the desktop bits), the permission being asked from an iframe,
expiry of decisions, and the real prompt UI. Subset grants and live revokes are in the next section.

### Subset grants and revoking a live stream (checked 2026-11-12)

`spike/revoke.ps1`, again with fake capture devices (no real camera or microphone opened).

**A. Granting a subset does not work; refuse instead.** The store allows the camera and denies the microphone, and the page asks for
both. With `--grant-subset` the app grants only the camera (`cont(camera)`). The page did **not** get a video-only stream: it got
`AbortError`, a different and less clear failure than the `NotAllowedError` a plain refusal gives. So keep the all-or-nothing rule from
the previous section: grant only when every requested kind is allowed, otherwise `cancel()`, and let the page ask again for just the
kind it wants. (With a real camera a subset grant might also switch the device on before aborting; not tried here.)

**B. Revoking does not stop a stream that is already running.** A page held a live camera stream and reported its track state and
frame count every second.

| Case | What happened |
| --- | --- |
| Revoke at 4 s (store entry deleted), page left alone | `track=live` and the frame count kept climbing (19, 39, ... 159 over 8 s): the revoke had no effect on the running stream |
| Revoke at 4 s, then the app reloads the page, and the user now denies | the old page's reports stop, the new page asks again (the store is empty, so the user is asked), and gets `NotAllowedError`: no stream |

CEF has no call to stop capture for an existing stream, and a revoke only changes what the app will answer next time. So the real
app's revoke action must end the stream itself: reload or navigate the page, or discard the view, and tell the user that is what
will happen. A gentler option (not tried) is a small render-process extension that wraps `getUserMedia`, remembers the streams it
handed out and stops their tracks on a command from the browser; that is more work and more attack surface in the page's process.

Not checked (the wrapper is in the next section): revoking from a settings page while the page is in another workspace (it would need a reload on
activation), revoking only the microphone while the camera stays live, and a stream held by an iframe.

### A render-process getUserMedia wrapper (checked 2026-11-12)

The idea left open above: stop a running stream without reloading the page. The renderer process (the same exe, which now hands its
app object to `execute_process` in every process) runs a small script in each frame's V8 context as it is created, before any page
script. It wraps `getUserMedia`, remembers the streams it hands out, and on a `m0-stop-capture` process message from the browser
(`all`, `audio` or `video`) stops their tracks, forwarding to same-origin child frames. Custom switches such as `--wrap=` are not passed
to child processes on their own; `on_before_child_process_launch` has to add them. `spike/wrapper.ps1`, fake devices (no real camera or
microphone opened). The page reports its tracks' `readyState` and a frame count every second.

| Scenario | Wrapper | Result |
| --- | --- | --- |
| W1 control, no revoke | full | stream live, frames climbing: the wrapper does not break `getUserMedia` |
| W2 revoke and stop all at 4 s | full | track `ended`, frame count frozen (73), the page's retry asked again and got `NotAllowedError` (the user denied): **the stream really stops, no reload** |
| W3 camera + microphone, revoke only the microphone | full | `audio=ended video=live`, video frames keep climbing: **one kind can be revoked while the other stays live** |
| W4 stream held by a same-origin iframe | full | the iframe's track ended too (forwarded by script); its retry was refused |
| W5 page calls `MediaDevices.prototype.getUserMedia` directly | instance-only | **not stopped** (frames kept climbing): patching only `navigator.mediaDevices` is bypassable |
| W6 same page | prototype | stopped |
| W7 page clones the stream and uses only the clone | prototype, no clone tracking | **not stopped** (the clone is a separate track) |
| W8 same page | full (also tracks `MediaStream.clone` and `MediaStreamTrack.clone`) | stopped |
| W9 hostile page replaces `MediaStreamTrack.prototype.stop` with a no-op | full | **not stopped**: the stream stays live |

Conclusion: the wrapper works for ordinary pages and gives a clean, no-reload revoke of the whole stream or just one kind, which
is nicer than reloading. But it lives in the page's own JS world and a page that wants to keep the camera can defeat it (W9 is the
simplest way; calling a captured original, legacy `navigator.getUserMedia`, `getDisplayMedia`, or WebRTC paths are others). So treat
it as a courtesy for cooperative sites and keep a browser-side guarantee behind it: after a revoke, stop the stream with the wrapper,
and if the page does not report its tracks ended (the wrapper can reply with the count, or the browser can check after a moment),
reload, navigate away or discard the view. Never rely on it alone to honour a revoke.

Not checked here (cross-origin iframes are in the next section): the legacy and
screen-capture APIs, WebRTC peers that hold a clone, workers, popups, whether the wrapper changes page behaviour on real sites, and
its cost per frame.

### The stop command across frames, including cross-origin iframes (checked 2026-11-12)

The wrapper section left cross-origin iframes open: a script call cannot cross origins, and the stop command had only gone to the main
frame. The browser side now keeps a list of every frame of workspace C's page from the `FrameHandler` callbacks (`on_frame_created`,
`on_frame_destroyed`; this avoids `browser.frame_identifiers`, whose string list this cef-rs build cannot read) and sends the stop message
to each frame, so each frame's own renderer runs it. `wrapper.ps1` scenarios X1 to X4, fake devices:

| Scenario | Stop sent to | Result |
| --- | --- | --- |
| X1 iframe on another origin of the same site (other port) | main frame only | **not stopped**, frames kept climbing |
| X2 same page | every frame | stopped (`ended`, count frozen), retry refused |
| X3 iframe on another site (`localhost`, probably its own renderer process) | main frame only | **not stopped** |
| X4 same page | every frame | stopped, retry refused |

So "stop everywhere" needs the per-frame list, and it works across origins and sites because the wrapper is installed in every
frame's context in whichever renderer process hosts it. Two more things the runs showed: the camera request from the cross-site iframe
arrived with the **iframe's own origin** (`http://localhost:...`), so a store keyed only by the requesting origin treats an embedded
widget and the page that embeds it as separate grants, and the real app should decide whether to key on (top-level origin,
requesting origin); and the same-origin iframe case from before is covered by the same list, so the script forwarding is no longer needed.

Not checked here (nested, late, `srcdoc` and sandboxed frames are in the next section): the page-tampering limit (W9 still applies in every frame).

### Nested, late and odd frames (checked 2026-11-12)

The cases the cross-origin section left open, same harness (`wrapper.ps1 -Only "N[123]"`, fake devices, stop sent to every frame):

| Scenario | Frames the command reached | Result |
| --- | --- | --- |
| N1 nested chain: top page, then an iframe on `localhost`, then an iframe on another port holding the stream | 3 (top, middle, inner) | inner track `ended`, frame count frozen, retry refused |
| N2 frames added late: one at the start, one on another site at 1.5 s, one created at 2.2 s and removed again 0.5 s later | 3 (top and the two still alive; the removed one had been pruned from the list) | both live frames ended (the late one had 48 frames to the early one's 72) and both were refused on retry |
| N3 a `srcdoc` iframe and a sandboxed iframe with an opaque origin (`allow-scripts` only) | 3 (top, srcdoc, sandboxed) | srcdoc: stream granted, then `ended` after the stop; sandboxed: `getUserMedia` was refused by Chromium itself with `SecurityError` (not a secure context), so it never held a stream |

So the per-frame list from the `FrameHandler` callbacks copes with nesting, late-created frames and frames removed mid-stream, and
the wrapper is installed in `srcdoc` frames as well. Sandboxed opaque-origin frames cannot capture at all.

Not checked here (navigation is in the next section): frames in popups or other windows of the same workspace, very deep or very many frames, the cost of keeping the list, and
the page-tampering limit (W9), which still applies inside every frame.

### Frames that navigate while streaming, and a lost stop (checked 2026-11-12)

`wrapper.ps1` scenarios N4 to N8 and the sweeps, fake devices, stop sent to every frame:

- **A cross-site iframe that navigates to another origin mid-stream (N4).** The first attempt was refused: with `allow="camera; microphone"
  the iframe's permission is delegated only to the origin it was loaded from, so after navigating elsewhere the new document got
  `NotAllowedError` from the permissions policy and the app was never asked. That is correct browser behaviour, not a spike bug.
  With `allow="camera *; microphone *"` the new document streamed (the old stream died with its document) and the stop reached it and
  ended it (2 frames: top and the iframe).
- **The main frame navigating cross-site mid-stream (N5).** The old stream died with its document, the new document in the new
  renderer streamed, and the stop reached its wrapper and ended it; the retry was refused. Nothing needed the old frame object.
- **A stop that is lost.** With the stop sent once, a revoke fired 0 ms after the new document's grant left its stream **live**: the
  message reached the renderer before the stream existed (grant answered, stream still being created), so there was nothing to stop,
  and nothing stopped it later. 40, 120 and 400 ms after the grant it worked. A timing sweep of the navigation against a revoke at a
  fixed time showed the same loss at two of five navigation times. Stopping is idempotent, so the browser now repeats the command
  (immediately and at +300, +600 and +900 ms; `--stop-once` turns that off to reproduce). With the repeats every offset ended the stream.

Design recommendation (built and compared in the next section): make the wrapper level-triggered instead of repeating on a timer. After a stop for a frame, the
wrapper remembers "revoked" and stops any stream that appears later in that frame (however long `getUserMedia` takes), until the
browser sends an allow message when it next grants. The timer version covers a 0.9 s window; a slow device open or a permission
answered late could still slip past it. The browser-side backstop (reload, navigate away or discard the view if the page has not
reported its tracks ended) stays necessary regardless, because of the page-tampering limit.

Not checked: frames navigating inside popups or other windows, `history` navigations and bfcache restores, a stream surviving a
same-document navigation (it should), a very slow device open, and the allow-message design above.

### A level-triggered wrapper (checked 2026-11-12)

The design recommended above, built and compared. Three wrapper variants, `--wrap=full|sticky|pending`; the stop is sent **once**
(`--stop-once`) so the wrapper alone decides:

- **full** (edge-triggered, what there was): a stop affects only streams that already exist.
- **sticky**: after a stop, every stream created later in that frame is killed, for good.
- **pending** (now the default): a stop also marks the `getUserMedia` calls that were already started and are still waiting; when one of
  those resolves, its tracks are stopped and the call is rejected with `NotAllowedError` (or, if only one kind was revoked, only that kind
  is stopped). Calls started after the stop are untouched.

| Case | full | sticky | pending |
| --- | --- | --- | --- |
| stop 0 ms after the new document's grant | **stream live** | rejected | rejected |
| stop 40 ms and 120 ms after the grant | ended | ended | ended |
| the user re-allows after the revoke and the page retries | stream works | **retry refused (wrongly)** | stream works |
| grant delivered 4 s late (a slow device or a late answer), stop sent at 0.5 s and repeated to 2.3 s | **stream granted and live for good** | (not run) | rejected, no stream |

So: the edge-triggered wrapper loses the race at 0 ms and cannot cope with a slow grant at all, even with the timed repeats; the sticky
wrapper fixes the race but also kills a retry the user has legitimately allowed, which makes it unusable (it would need an "allow"
message from the browser, with its own ordering race); the pending-call wrapper gets all four right with no extra message, because it
ties the revoke to the requests that were in flight when it happened rather than to the clock or to a flag. The repeated stop stays in the
spike as a harmless backstop for streams that already exist.

What it does not cover: a hostile page can still defeat it (the W9 limit: all its state lives in the page's JS world), so the browser-side
backstop (reload, navigate away or discard if the page does not report its tracks ended) is still required; calls made through paths the
wrapper does not patch (legacy and screen-capture APIs); and a request that is pending in a frame the stop never reaches.

Not checked here (a one-kind revoke while a two-kind request is pending is in the next section): many pending calls at once, and the cost of the
extra bookkeeping on real pages.

### Revoking one kind while a camera-and-microphone request is pending (checked 2026-11-12)

The page asks for camera and microphone together; the app decides to grant both but delivers the grant 4 s late; 0.5 s after the
decision it revokes one kind (`--grant-n=1 --revoke-after-grant-ms=500 --delay-grant-ms=4000`), with the stop sent once. `wrapper.ps1`
`-Only K -Sweep kind-pending|kind-full`, fake devices.

| Revoke | Pending-call wrapper (default) | Edge-triggered wrapper | Store afterwards (allowed, denied) |
| --- | --- | --- | --- |
| microphone only | stream arrives with `audio=ended video=live`, video frames keep climbing | both tracks live | `2, 1`: camera allowed, microphone denied |
| camera only | stream arrives with `audio=live video=ended`, video frozen | both tracks live | `1, 2` |
| all | the call is rejected with `NotAllowedError`, no stream | both tracks live | store file deleted |

So the pending-call wrapper honours a partial revoke of a request that is still waiting: the revoked kind is stopped the moment the
stream resolves, the other kind keeps running, and the store records exactly that. The edge-triggered wrapper loses all three cases.

Two behaviours to decide on for the real app. First, the page gets a stream whose revoked track is already `ended` (and no `ended`
event fires, because the track was never live when the page first saw it); the alternative is to remove the track from the stream so
it holds only the allowed kind, which fits a page that checks `getAudioTracks()[0]` but may break one that expects both. An ended
track matches what a live revoke looks like, so the spike keeps it. Second, when a request mixes allowed and revoked kinds the page's
next request for both will now be refused as a whole (all or nothing, from earlier), so a site that wants to carry on must ask for the
allowed kind alone; that is the standard way to handle a denied microphone, but it is worth a visible hint in the real prompt UI.

Not checked here (several pending calls are in the next section): a revoke arriving after only one of the two tracks has been created, and a
pending call in an iframe other than the one the stop reached.

### Several getUserMedia calls pending at once (checked 2026-11-12)

Two or three calls made together: video only (V), audio only (A) and audio plus video (AV). One kind is revoked while they wait
(`wrapper.ps1 -Only P -Sweep multi`, fake devices, stop sent once).

**A finding first:** Chrome queues a frame's `getUserMedia` requests. The second call's request only reached the app after the first
grant had been delivered (the first grant was held for 4 s and the next requests arrived behind it, roughly 4 s apart). So "pending
in the page" is not the same as "pending in the app": the calls behind the first have not been decided yet. With the revoke also
updating the store, a first run showed the app refusing the queued calls itself (store `0, 3` after revoking video), which hid the
wrapper's own logic. The runs below use `--revoke-no-store`: only the stop goes out, the store keeps allowing everything, so what happens
to each call is the wrapper's doing.

| Scenario | V (video) | A (audio) | AV (audio + video) |
| --- | --- | --- | --- |
| P1 two calls, revoke audio | live | **rejected** `NotAllowedError` | - |
| P2 two calls, revoke video | **rejected** | live | - |
| P3 three calls, revoke audio | live | **rejected** | stream with `audio=ended video=live` |
| P4 three calls, revoke all | rejected | rejected | rejected |

So the marking is selective as intended: a call dies only if it asked for the revoked kind; a call for the other kind is untouched; a
mixed call loses just the revoked kind. Calls that were still queued behind the first are included (they were pending in the page when
the stop arrived), even though the app had not decided them and the store still allowed them. That is the right reading of "revoke now"
for a real revoke, where the store changes too and would refuse them anyway; it would be wrong only if a revoke could be undone within
the same instant, which the all-or-nothing rule and the store make unnecessary.

Not checked here: many more than three calls, calls from different frames, and the timing of a stop that lands between the first and
second grant.

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

Everything the five tests ask for has a written result on Windows with candidate A (direct CEF through cef-rs). What is left before the
exit decision, and before M1:

1. **Record the decision** in `docs/ai/decisions.md`: candidate A is viable on Windows. Candidate B (Tauri with CEF) was never tried and
   only needs doing if A is rejected for another reason; say so explicitly rather than leaving it open.
2. **Close the gaps the checks could not reach:** IME (needs an IME installed), real OS-level DPI scaling and mixed-DPI monitors, a real
   login to a real site in two workspaces, and the first non-Windows platform.
3. **Measure and publish the cost** the spec asks for: installed size of the runtime plus our binary, memory at rest and with several
   workspaces, start-up time, and the Chromium update procedure.
4. **Understand the open oddities:** the GPU process that crashed at start-up until DirectComposition was switched off (root cause and
   cost unknown), and what happens when the app's own processes are killed.
5. **Turn the findings into M1 design notes** (they are written up above): the app owns dialogs and permission prompts, a per-workspace
   media decision store, the stop-capture wrapper plus a browser-side reload fallback, discard and recreate of views, per-frame stop
   commands, and key handling that does not depend on a page's view having focus.

