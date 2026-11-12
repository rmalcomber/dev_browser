//! M0 feasibility spike (DISPOSABLE, not the product). Candidate A: direct CEF via cef-rs, Views framework.
//!
//! One window, two panes (test 5), a draggable overlay launcher above the pages (test 2), a pre-key-event
//! hook for Ctrl+P (test 3), and one CEF request context per pane with its own cache path (test 4).
//! Everything prints `[m0] ...` lines so a run can be checked from the log. See ../planning/m0-feasibility-spike.md.
//!
//! Flags: --url-a=URL --url-b=URL --single (one pane) --selftest-write (pane A writes localStorage)
//!        --exit-after=SECS (close the window after N seconds)
use cef::*;
use std::cell::RefCell;

#[repr(C)]
struct Rc4 { l: i32, t: i32, r: i32, b: i32 }
#[link(name = "user32")]
extern "system" {
    fn GetWindowRect(h: *mut std::ffi::c_void, r: *mut Rc4) -> i32;
    fn IsWindowVisible(h: *mut std::ffi::c_void) -> i32;
    fn GetParent(h: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

const PANE_A: u8 = 0;
const PANE_B: u8 = 1;
const DIVIDER: u8 = 8;
const OVERLAY: u8 = 9;
const VK_P: i32 = 0x50;
const VK_1: i32 = 0x31;
const VK_2: i32 = 0x32;
const VK_ESCAPE: i32 = 0x1B;
const VK_3: i32 = 0x33;
const VK_4: i32 = 0x34;
const CTRL: u32 = 1 << 2; // EVENTFLAG_CONTROL_DOWN

#[derive(Default)]
struct St {
    browsers: Vec<Browser>,
    pane_views: Vec<BrowserView>,
    overlay_view: Option<BrowserView>,
    overlay: Option<OverlayController>,
    window: Option<Window>,
    overlay_visible: bool,
    last_pane: usize,
    drag_seen: i32,
    divider_view: Option<BrowserView>,
    layout: Option<BoxLayout>,
    div_seen: i32,
    ratio: f32,
    panel: Option<Panel>,
    last_layout: (i32, i32, i32),
    pane_w: [i32; 3],
    fresh: Option<(Panel, BrowserView)>, // workspace C, created on first use
    fresh_ctx: Option<RequestContext>,
    wrappers: Vec<Panel>,
    mode: u8, // 0 split, 1 workspace A only, 2 workspace B only (the spike's stand-in for switching workspace)
    ctxs: Vec<RequestContext>,
    ctx_ready: usize,
}

thread_local! { static ST: RefCell<St> = RefCell::new(St::default()); }
thread_local! { static PENDING_DIALOGS: RefCell<Vec<JsdialogCallback>> = RefCell::new(Vec::new()); }
thread_local! { static PENDING_PERMS: RefCell<Vec<PermissionPromptCallback>> = RefCell::new(Vec::new()); }
thread_local! { static PENDING_MEDIA: RefCell<Vec<MediaAccessCallback>> = RefCell::new(Vec::new()); }

fn arg(name: &str) -> String {
    let cl = command_line_get_global().expect("command line");
    CefString::from(&cl.switch_value(Some(&CefString::from(name)))).to_string()
}
fn flag(name: &str) -> bool {
    command_line_get_global().map(|c| c.has_switch(Some(&CefString::from(name))) == 1).unwrap_or(false)
}
fn role_name(role: u8) -> &'static str {
    match role { PANE_A => "A", PANE_B => "B", DIVIDER => "divider", _ => "overlay" }
}

fn set_overlay_visible(show: bool) {
    let (ctl, ov, pane, last) = ST.with(|s| {
        let mut s = s.borrow_mut();
        s.overlay_visible = show;
        (s.overlay.clone(), s.overlay_view.clone(), s.pane_views.get(s.last_pane).cloned(), s.last_pane)
    });
    if let Some(ctl) = ctl { ctl.set_visible(show as i32); }
    // Finding: hiding only the overlay controller leaves the overlay's browser painting over the pane below; hide the view too.
    if let Some(v) = ov.as_ref() { v.set_visible(show as i32); }
    if show {
        if let Some(v) = ov {
            v.request_focus();
            if let Some(f) = v.browser().and_then(|b| b.main_frame()) {
                f.execute_java_script(Some(&CefString::from("var q=document.getElementById('q');q.value='';q.focus();")), None, 0);
            }
        }
    } else if let Some(v) = pane {
        v.request_focus();
    }
    println!("[m0] overlay {} (focus -> {})", if show { "shown" } else { "hidden" }, if show { "overlay".to_string() } else { format!("pane {}", last) });
}

// Spike stand-in for switching workspace: split -> only workspace A -> only workspace B -> split.
fn cycle_mode() {
    let (mode, wr, dv, panes) = ST.with(|s| { let mut s = s.borrow_mut(); s.mode = if s.mode >= 2 { if s.mode == 3 { 0 } else { 0 } } else { s.mode + 1 }; (s.mode, s.wrappers.clone(), s.divider_view.clone(), s.pane_views.clone()) });
    if wr.len() != 2 { return }
    wr[0].set_visible((mode != 2) as i32);
    wr[1].set_visible((mode != 1) as i32);
    if let Some(d) = dv.as_ref() { d.set_visible((mode == 0) as i32); }
    if let Some((cw, _)) = ST.with(|s| s.borrow().fresh.clone()) { cw.set_visible(0); }
    println!("[m0] mode: {}", match mode { 0 => "split", 1 => "workspace A only", _ => "workspace B only" });
    relayout();
    let focus = match mode { 1 => 0, 2 => 1, _ => ST.with(|s| s.borrow().last_pane) };
    if let Some(p) = panes.get(focus) { p.request_focus(); }
}

// Workspace C is created the first time it is asked for (new request context, then a browser view, then it loads), which is
// the case a flash is most likely in: the view has never painted.
wrap_request_context_handler! {
    struct SpikeRchFresh {}
    impl RequestContextHandler {
        fn on_request_context_initialized(&self, _c: Option<&mut RequestContext>) {
            println!("[m0] workspace C context ready");
            create_fresh_view();
        }
    }
}

fn start_fresh_workspace() {
    let (exists, wr, dv) = ST.with(|s| { let s = s.borrow(); (s.fresh.is_some(), s.wrappers.clone(), s.divider_view.clone()) });
    if wr.len() != 2 { return }
    ST.with(|s| s.borrow_mut().mode = 3);
    // hide the old workspace at once (never leave its content up while the new one loads: the spec's screen-share rule)
    wr[0].set_visible(0); wr[1].set_visible(0);
    if let Some(d) = dv.as_ref() { d.set_visible(0); }
    println!("[m0] mode: workspace C (fresh view), exists={exists}");
    if exists {
        if let Some((cw, _)) = ST.with(|s| s.borrow().fresh.clone()) { cw.set_visible(1); }
        wr[0].set_visible(0); wr[1].set_visible(0);
        if let Some(d) = dv.as_ref() { d.set_visible(0); }
        relayout();
        return;
    }
    // The view was destroyed earlier but its request context (the workspace's session) is kept: recreate only the view.
    if ST.with(|s| s.borrow().fresh_ctx.is_some()) {
        println!("[m0] recreating workspace C's view from its kept context");
        create_fresh_view();
        return;
    }
    let root = std::env::current_dir().unwrap().join(".m0-data").join("chromium-root");
    let cache = root.join("ws-c");
    let _ = std::fs::create_dir_all(&cache);
    let rcs = RequestContextSettings { cache_path: CefString::from(cache.to_string_lossy().as_ref()), persist_session_cookies: 1, ..Default::default() };
    let mut h = SpikeRchFresh::new();
    let ctx = request_context_create_context(Some(&rcs), Some(&mut h)).expect("request context C");
    ST.with(|s| s.borrow_mut().fresh_ctx = Some(ctx));
}

// Destroys workspace C's browser and view (a discarded tab), keeping its request context.
fn destroy_fresh_view(source: &str) {
    let (fresh, panel) = ST.with(|s| { let mut s = s.borrow_mut(); (s.fresh.take(), s.panel.clone()) });
    let (Some((wrap, bv)), Some(panel)) = (fresh, panel) else { println!("[m0] no workspace C view to destroy"); return };
    // a custom dialog UI must resolve or release any dialog the destroyed view still has pending
    let pending: Vec<JsdialogCallback> = PENDING_DIALOGS.with(|p| p.borrow_mut().drain(..).collect());
    if !pending.is_empty() {
        if flag("drop-dialog") { println!("[m0] {} pending dialog(s) dropped without an answer", pending.len()); drop(pending); }
        else { for cb in pending { cb.cont(0, Some(&CefString::from(""))); } println!("[m0] pending dialog(s) cancelled before destroying"); }
    }
    // same for permission prompts: a browser-owned prompt UI must resolve or release what the destroyed view still has pending
    let perms: Vec<PermissionPromptCallback> = PENDING_PERMS.with(|p| p.borrow_mut().drain(..).collect());
    let media: Vec<MediaAccessCallback> = PENDING_MEDIA.with(|p| p.borrow_mut().drain(..).collect());
    if !perms.is_empty() || !media.is_empty() {
        if flag("drop-permission") { println!("[m0] {} permission prompt(s) and {} media request(s) dropped without an answer", perms.len(), media.len()); drop(perms); drop(media); }
        else {
            let n = (perms.len(), media.len());
            for cb in perms { cb.cont(PermissionRequestResult::DISMISS); }
            for cb in media { cb.cancel(); }
            println!("[m0] {} permission prompt(s) and {} media request(s) dismissed before destroying", n.0, n.1);
        }
    }
    // Finding: closing a view's browser while it is still in the window closes the whole window (every browser, then the
    // app). Detach the view first, then close its browser.
    let mut wv = View::from(&wrap);
    panel.remove_child_view(Some(&mut wv));
    if let Some(h) = bv.browser().and_then(|b| b.host()) { h.close_browser(1); }
    println!("[m0] workspace C view destroyed (context kept) by {source}");
    // Finding: if the destroyed view was the one on screen (and focused), nothing was left to receive key events and the window
    // stayed blank. Fall back to the split view and give a pane the focus, as the real app should when a workspace is discarded.
    let (mode, wr, dv, panes, last) = ST.with(|s| { let mut s = s.borrow_mut(); let m = s.mode; if m == 3 { s.mode = 0; } s.last_layout = (0, 0, 0); (m, s.wrappers.clone(), s.divider_view.clone(), s.pane_views.clone(), s.last_pane) });
    if mode == 3 {
        for w in wr.iter() { w.set_visible(1); }
        if let Some(d) = dv.as_ref() { d.set_visible(1); }
        println!("[m0] mode: split (fell back from the destroyed workspace)");
        relayout();
        if let Some(p) = panes.get(last) { p.request_focus(); }
    } else {
        relayout();
    }
}

fn create_fresh_view() {
    let (ctx, panel) = ST.with(|s| { let s = s.borrow(); (s.fresh_ctx.clone(), s.panel.clone()) });
    let (Some(mut ctx), Some(panel)) = (ctx, panel) else { return };
    let url_c = arg("url-c");
    let url = if url_c.is_empty() { "data:text/html,<body%20style=background:%230a0;color:%23fff;font:200px%20sans-serif><b>C</b>".to_string() } else { url_c };
    let url = url.as_str();
    let mut client = SpikeClient::new(2);
    let mut bvd = SpikePaneBvd::new(2);
    let bv = browser_view_create(Some(&mut client), Some(&CefString::from(url)), Some(&bsettings()), None, Some(&mut ctx), Some(&mut bvd)).expect("browser view C");
    let mut wd = SpikeWrapDelegate::new();
    let wrap = panel_create(Some(&mut wd)).expect("wrapper C");
    wrap.set_background_color(argb(ACCENTS[2], false));
    let l = wrap.set_to_box_layout(Some(&BoxLayoutSettings { horizontal: 1, inside_border_insets: Insets { top: BORDER, left: BORDER, bottom: BORDER, right: BORDER }, cross_axis_alignment: AxisAlignment::STRETCH, ..Default::default() }));
    let mut wv = View::from(&wrap);
    panel.add_child_view(Some(&mut wv));
    let mut v = View::from(&bv);
    wrap.add_child_view(Some(&mut v));
    if let Some(l) = l.as_ref() { l.set_flex_for_view(Some(&mut v), 0); }
    ST.with(|s| s.borrow_mut().fresh = Some((wrap.clone(), bv.clone())));
    wrap.set_visible(1);
    println!("[m0] workspace C view created and shown");
    // --auto-destroy-ms=N: destroy it from a timer too, for pages (alert dialogs) that swallow the key event
    let ms: i64 = arg("auto-destroy-ms").parse().unwrap_or(0);
    if ms > 0 { let mut t = AutoDestroyTask::new(); post_delayed_task(ThreadId::UI, Some(&mut t), ms); }
    relayout();
}

fn toggle_overlay() {
    let vis = ST.with(|s| s.borrow().overlay_visible);
    set_overlay_visible(!vis);
}

fn focus_pane(i: usize) {
    let v = ST.with(|s| {
        let mut s = s.borrow_mut();
        let v = s.pane_views.get(i).cloned();
        if v.is_some() { s.last_pane = i; }
        v
    });
    if let Some(v) = v { v.request_focus(); println!("[m0] focus-pane requested {}", i + 1); }
}

// Overlay -> Rust channel for the spike: the overlay page writes commands into document.title.
fn on_overlay_title(t: &str) {
    if let Some(rest) = t.strip_prefix("drag:") {
        let p: Vec<i32> = rest.split(',').filter_map(|x| x.parse().ok()).collect();
        if p.len() == 3 {
            ST.with(|s| {
                let mut s = s.borrow_mut();
                if p[2] <= s.drag_seen { return; }
                s.drag_seen = p[2];
                let (Some(ctl), Some(win)) = (s.overlay.clone(), s.window.clone()) else { return };
                let pos = ctl.position();
                let size = ctl.size();
                let win_size = win.size();
                let x = (pos.x + p[0]).clamp(0, (win_size.width - size.width).max(0));
                let y = (pos.y + p[1]).clamp(0, (win_size.height - size.height).max(0));
                ctl.set_position(Some(&Point { x, y }));
                println!("[m0] overlay moved to ({x},{y}) window={}x{}", win_size.width, win_size.height);
            });
        }
    } else if t.starts_with("close:") {
        set_overlay_visible(false);
    }
}

// Divider -> Rust channel (same title trick as the overlay): "div:dx,n".
// The three views live in a Panel with a box layout. Changing flex weights on an installed layout did not re-flow, so
// relayout() re-installs the layout with new weights. (Positioning the children by hand with set_bounds painted wrongly:
// the left pane showed only its background, or its content flickered over the right pane.)
const DIV_W: i32 = 6;
const MIN_PANE: i32 = 120;
// Workspace outline: each pane sits in a wrapper panel whose background is the workspace accent colour, showing as a
// BORDER-px frame. The focused pane's frame is the full colour, the other is dimmed.
const BORDER: i32 = 3;
const ACCENTS: [u32; 3] = [0xF4A261, 0xA78BFA, 0x2ECC71]; // workspace A amber, workspace B violet (RGB)

// Finding: a freshly created browser view showed about three white frames before its first paint. A dark background colour
// on the browser (matching the app) makes that gap dark instead. Disable with --white-bg to reproduce.
fn bsettings() -> BrowserSettings {
    let white = std::env::args().any(|a| a == "--white-bg");
    BrowserSettings { background_color: if white { 0 } else { 0xFF1E1E1E }, ..Default::default() }
}

fn argb(rgb: u32, dim: bool) -> u32 {
    if !dim { return 0xFF00_0000 | rgb }
    // blend 65% towards the dark page background so the frame is visible but clearly weaker
    let (r, g, b) = ((rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF);
    let mix = |c: u32| (c * 35 + 0x22 * 65) / 100;
    0xFF00_0000 | (mix(r) << 16) | (mix(g) << 8) | mix(b)
}

fn update_outline() {
    let (wr, focused) = ST.with(|s| { let s = s.borrow(); (s.wrappers.clone(), s.last_pane) });
    for (i, w) in wr.iter().enumerate() {
        w.set_background_color(argb(ACCENTS[i], i != focused));
    }
    println!("[m0] outline: focused pane {} full accent, other dimmed", focused + 1);
}

fn state_file() -> std::path::PathBuf { std::env::current_dir().unwrap().join(".m0-data").join("divider.ratio") }
fn load_ratio() -> Option<f32> {
    let r: f32 = std::fs::read_to_string(state_file()).ok()?.trim().parse().ok()?;
    if r > 0.05 && r < 0.95 { Some(r) } else { None }
}
fn save_ratio(r: f32) { let _ = std::fs::write(state_file(), format!("{r:.4}")); }

fn relayout() {
    let (panes, divider, ratio, panel, wrappers) = ST.with(|s| { let s = s.borrow(); (s.pane_views.clone(), s.divider_view.clone(), if s.ratio > 0.0 { s.ratio } else { 0.5 }, s.panel.clone(), s.wrappers.clone()) });
    let (Some(divider), Some(panel), true) = (divider, panel, panes.len() == 2 && wrappers.len() == 2) else { return };
    let size = panel.size();
    if size.width <= 0 || size.height <= 0 { return }
    let mode = ST.with(|s| s.borrow().mode);
    let avail = size.width - DIV_W;
    let wa = ((avail as f32 * ratio) as i32).clamp(MIN_PANE, (avail - MIN_PANE).max(MIN_PANE));
    // Only re-install when something changed, otherwise the layout change this causes would call us again.
    let key = (size.width, size.height, wa + 100000 * mode as i32);
    if ST.with(|s| { let mut s = s.borrow_mut(); let same = s.last_layout == key; s.last_layout = key; same }) { return }
    // A box layout on the panel keeps the native browser windows in step with the views; re-installing it with new
    // weights is what re-flows it (changing flex on an installed layout did not).
    let Some(layout) = panel.set_to_box_layout(Some(&BoxLayoutSettings { horizontal: 1, cross_axis_alignment: AxisAlignment::STRETCH, ..Default::default() })) else { return };
    let (mut va, mut vd, mut vb) = (View::from(&wrappers[0]), View::from(&divider), View::from(&wrappers[1]));
    let (pa, pb) = (View::from(&panes[0]), View::from(&panes[1]));
    for v in [&mut va, &mut vd, &mut vb] { layout.set_flex_for_view(Some(v), 0); }
    if let Some((cw, cb)) = ST.with(|s| s.borrow().fresh.clone()) {
        let mut vc = View::from(&cw); layout.set_flex_for_view(Some(&mut vc), 0);
        let vcb = View::from(&cb); vcb.invalidate_layout(); vc.invalidate_layout();
    }
    // In single-workspace mode the visible pane takes the whole width (hidden views are skipped by the layout).
    let widths = match mode { 1 => [size.width, 0, 0], 2 => [0, size.width, 0], 3 => [0, 0, size.width], _ => [wa, avail - wa, 0] };
    ST.with(|s| s.borrow_mut().pane_w = widths);
    println!("[m0] relayout: mode {mode} panel {}x{} -> A={}px B={}px C={}px (ratio {:.3})", size.width, size.height, widths[0], widths[1], widths[2], wa as f32 / avail as f32);
    pa.invalidate_layout();
    pb.invalidate_layout();
    va.invalidate_layout();
    vb.invalidate_layout();
    panel.invalidate_layout();
}

fn on_divider_title(t: &str) {
    let Some(rest) = t.strip_prefix("div:") else { return };
    let p: Vec<i32> = rest.split(',').filter_map(|x| x.parse().ok()).collect();
    if p.len() != 2 { return }
    let panes = ST.with(|s| {
        let mut s = s.borrow_mut();
        if p[1] <= s.div_seen { return vec![]; }
        s.div_seen = p[1];
        s.wrappers.clone()
    });
    if panes.len() != 2 { return }
    let (wa, wb) = (panes[0].bounds().width, panes[1].bounds().width);
    let avail = wa + wb;
    let new_a = (wa + p[0]).clamp(MIN_PANE, (avail - MIN_PANE).max(MIN_PANE));
    let ratio = new_a as f32 / avail as f32;
    ST.with(|s| s.borrow_mut().ratio = ratio);
    save_ratio(ratio);
    relayout();
    println!("[m0] divider moved: A {wa}px -> {}px, B {wb}px -> {}px", panes[0].bounds().width, panes[1].bounds().width);
}

wrap_keyboard_handler! {
    struct SpikeKeys { role: u8 }
    impl KeyboardHandler {
        fn on_pre_key_event(&self, _b: Option<&mut Browser>, event: Option<&KeyEvent>, _os: Option<&mut sys::MSG>, _shortcut: Option<&mut i32>) -> i32 {
            let Some(e) = event else { return 0 };
            if e.type_ != KeyEventType::RAWKEYDOWN { return 0 }
            let ctrl = e.modifiers & CTRL != 0;
            let focus_in_input = e.focus_on_editable_field;
            if ctrl && e.windows_key_code == VK_P {
                println!("[m0] ctrl+p intercepted (source={}, editable_focus={})", role_name(self.role), focus_in_input);
                toggle_overlay();
                return 1;
            }
            if ctrl && e.windows_key_code == VK_3 && self.role != OVERLAY { cycle_mode(); return 1; }
            if ctrl && e.windows_key_code == VK_4 && self.role != OVERLAY { start_fresh_workspace(); return 1; }
            if ctrl && e.windows_key_code == 0x36 && self.role != OVERLAY { destroy_fresh_view("key"); return 1; }
            if ctrl && (e.windows_key_code == VK_1 || e.windows_key_code == VK_2) && self.role != OVERLAY {
                focus_pane(if e.windows_key_code == VK_1 { 0 } else { 1 });
                return 1;
            }
            if e.windows_key_code == VK_ESCAPE && self.role == OVERLAY {
                println!("[m0] escape intercepted (overlay)");
                set_overlay_visible(false);
                return 1;
            }
            0
        }
    }
}

wrap_focus_handler! {
    struct SpikeFocus { role: u8 }
    impl FocusHandler {
        fn on_got_focus(&self, _b: Option<&mut Browser>) {
            println!("[m0] got-focus {}", role_name(self.role));
            if self.role == PANE_A || self.role == PANE_B {
                ST.with(|s| s.borrow_mut().last_pane = self.role as usize);
                update_outline();
            }
        }
    }
}

wrap_display_handler! {
    struct SpikeDisplay { role: u8 }
    impl DisplayHandler {
        fn on_title_change(&self, _b: Option<&mut Browser>, title: Option<&CefString>) {
            let t = title.map(|t| t.to_string()).unwrap_or_default();
            if self.role == OVERLAY { on_overlay_title(&t); }
            else if self.role == DIVIDER { on_divider_title(&t); }
            else if t.starts_with("[m0-ls]") { println!("{t}"); }
        }
    }
}

// Finding: with CEF's own (Views) JS dialogs, destroying a view while an alert() was open crashed the app (access violation inside
// libcef). The app should own dialog UI. By default the spike answers dialogs itself (cancels alert/confirm, allows leaving);
// --native-dialogs leaves them to CEF (reproduces the crash); --hold-dialog keeps the callback pending like a custom dialog would.
wrap_jsdialog_handler! {
    struct SpikeDialogs { role: u8 }
    impl JsdialogHandler {
        fn on_jsdialog(&self, _b: Option<&mut Browser>, origin: Option<&CefString>, _t: JsdialogType, msg: Option<&CefString>, _d: Option<&CefString>, cb: Option<&mut JsdialogCallback>, _suppress: Option<&mut i32>) -> i32 {
            let origin = origin.map(|o| o.to_string()).unwrap_or_default();
            let msg = msg.map(|m| m.to_string()).unwrap_or_default();
            if flag("hold-dialog") {
                if let Some(cb) = cb { PENDING_DIALOGS.with(|p| p.borrow_mut().push(cb.clone())); }
                println!("[m0] js dialog from {origin} held open: {msg:?}");
            } else {
                if let Some(cb) = cb { cb.cont(0, Some(&CefString::from(""))); }
                println!("[m0] js dialog from {origin} answered by the app (cancelled): {msg:?}");
            }
            1
        }
        fn on_before_unload_dialog(&self, _b: Option<&mut Browser>, _m: Option<&CefString>, _reload: i32, cb: Option<&mut JsdialogCallback>) -> i32 {
            if flag("hold-dialog") {
                if let Some(cb) = cb { PENDING_DIALOGS.with(|p| p.borrow_mut().push(cb.clone())); }
                println!("[m0] beforeunload dialog held open");
            } else {
                if let Some(cb) = cb { cb.cont(1, Some(&CefString::from(""))); }
                println!("[m0] beforeunload dialog answered by the app (leave allowed)");
            }
            1
        }
    }
}

// Permission prompts (geolocation, notifications, clipboard, camera and microphone). With --own-permissions the app takes every
// prompt and keeps its callback pending, as a browser-owned prompt UI would; without it CEF does what it does by default.
wrap_permission_handler! {
    struct SpikePerms {}
    impl PermissionHandler {
        fn on_show_permission_prompt(&self, _b: Option<&mut Browser>, id: u64, origin: Option<&CefString>, perms: u32, cb: Option<&mut PermissionPromptCallback>) -> i32 {
            let answer = arg("answer-permissions");
            let o = origin.map(|o| o.to_string()).unwrap_or_default();
            if let (Some(cb), true) = (cb.as_ref(), !answer.is_empty()) {
                let r = if answer == "accept" { PermissionRequestResult::ACCEPT } else { PermissionRequestResult::DENY };
                println!("[m0] permission prompt #{id} from {o} (bits 0x{perms:x}) answered by the app: {answer}");
                cb.cont(r);
                return 1;
            }
            println!("[m0] permission prompt #{id} from {o} (permission bits 0x{perms:x}) held by the app");
            if let Some(cb) = cb { PENDING_PERMS.with(|p| p.borrow_mut().push(cb.clone())); }
            1
        }
        fn on_request_media_access_permission(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, origin: Option<&CefString>, perms: u32, cb: Option<&mut MediaAccessCallback>) -> i32 {
            let answer = arg("answer-permissions");
            let o = origin.map(|o| o.to_string()).unwrap_or_default();
            if let (Some(cb), true) = (cb.as_ref(), !answer.is_empty()) {
                println!("[m0] media permission request from {o} (bits 0x{perms:x}) answered by the app: {answer}");
                if answer == "accept" { cb.cont(perms); } else { cb.cancel(); }
                return 1;
            }
            println!("[m0] media permission request from {o} (bits 0x{perms:x}) held by the app");
            if let Some(cb) = cb { PENDING_MEDIA.with(|p| p.borrow_mut().push(cb.clone())); }
            1
        }
        fn on_dismiss_permission_prompt(&self, _b: Option<&mut Browser>, id: u64, _r: PermissionRequestResult) {
            println!("[m0] permission prompt #{id} dismissed by CEF");
        }
    }
}

wrap_load_handler! {
    struct SpikeLoad { role: u8 }
    impl LoadHandler {
        fn on_load_end(&self, b: Option<&mut Browser>, frame: Option<&mut Frame>, status: i32) {
            let Some(f) = frame else { return };
            if f.is_main() != 1 { return }
            let url = CefString::from(&f.url()).to_string();
            println!("[m0] load-end pane={} status={status} url={url}", role_name(self.role));
            // Test 4 probe: localStorage in this pane's own request context. Result is reported via the title.
            if self.role != OVERLAY && url.starts_with("https://") {
                let write = self.role == PANE_A && flag("selftest-write");
                let js = format!(
                    "(function(){{var k='m0_who',p=localStorage.getItem(k);{w}document.title='[m0-ls] pane={n} before={{'+p+'}} after={{'+localStorage.getItem(k)+'}}';}})();",
                    w = if write { "localStorage.setItem(k,'written-by-A');" } else { "" },
                    n = role_name(self.role)
                );
                f.execute_java_script(Some(&CefString::from(js.as_str())), None, 0);
            }
            if self.role == PANE_A || self.role == PANE_B {
                let js = format!("setTimeout(function(){{document.title='[m0-ls] viewport pane={}: '+innerWidth+'x'+innerHeight+' dpr='+devicePixelRatio;}},1500);", role_name(self.role));
                f.execute_java_script(Some(&CefString::from(js.as_str())), None, 0);
            }
            let _ = b;
        }
    }
}

wrap_life_span_handler! {
    struct SpikeLife {}
    impl LifeSpanHandler {
        fn on_after_created(&self, b: Option<&mut Browser>) {
            if let Some(b) = b {
                if let Some(rc) = b.host().and_then(|h| h.request_context()) {
                    println!("[m0] browser created; its request context cache_path={:?} global={}", CefString::from(&rc.cache_path()).to_string(), rc.is_global());
                }
                ST.with(|s| s.borrow_mut().browsers.push(b.clone()));
            }
        }
        fn on_before_close(&self, b: Option<&mut Browser>) {
            let mut b = b.cloned().expect("browser");
            let empty = ST.with(|s| {
                let mut s = s.borrow_mut();
                if let Some(i) = s.browsers.iter().position(|x| x.is_same(Some(&mut b)) != 0) { s.browsers.remove(i); }
                s.browsers.is_empty()
            });
            println!("[m0] browser closed; {} left", ST.with(|s| s.borrow().browsers.len()));
            if empty {
                println!("[m0] all browsers closed");
                if let Some(w) = ST.with(|s| s.borrow().window.clone()) { w.close(); }
            }
        }
    }
}

wrap_client! {
    struct SpikeClient { role: u8 }
    impl Client {
        fn keyboard_handler(&self) -> Option<KeyboardHandler> { Some(SpikeKeys::new(self.role)) }
        fn focus_handler(&self) -> Option<FocusHandler> { Some(SpikeFocus::new(self.role)) }
        fn display_handler(&self) -> Option<DisplayHandler> { Some(SpikeDisplay::new(self.role)) }
        fn load_handler(&self) -> Option<LoadHandler> { Some(SpikeLoad::new(self.role)) }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> { Some(SpikeLife::new()) }
        fn permission_handler(&self) -> Option<PermissionHandler> { if flag("own-permissions") || !arg("answer-permissions").is_empty() { Some(SpikePerms::new()) } else { None } }
        fn jsdialog_handler(&self) -> Option<JsdialogHandler> { if flag("native-dialogs") { None } else { Some(SpikeDialogs::new(self.role)) } }
    }
}

wrap_browser_view_delegate! {
    struct SpikeBvd {}
    impl ViewDelegate {}
    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
    }
}

// A Panel holding the two panes and the divider; relayout() gives it a box layout.
wrap_panel_delegate! {
    struct SpikePanelDelegate {}
    impl ViewDelegate {
        fn on_layout_changed(&self, _v: Option<&mut View>, _b: Option<&Rect>) { relayout(); }
    }
    impl PanelDelegate {}
}

// Pane browser views report the width relayout() chose as their preferred size; the panel's box layout (flex 0) then gives
// each exactly that. (Flex weights alone were ignored for these views.)
wrap_browser_view_delegate! {
    struct SpikePaneBvd { idx: usize }
    impl ViewDelegate {
        fn preferred_size(&self, _v: Option<&mut View>) -> Size {
            let w = ST.with(|s| s.borrow().pane_w[self.idx]);
            Size { width: (w - 2 * BORDER).max(1), height: 100 }
        }
    }
    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
    }
}

wrap_panel_delegate! {
    struct SpikeWrapDelegate {}
    impl ViewDelegate {}
    impl PanelDelegate {}
}

wrap_browser_view_delegate! {
    struct SpikeDividerBvd {}
    impl ViewDelegate {
        fn preferred_size(&self, _v: Option<&mut View>) -> Size { Size { width: 6, height: 100 } }
    }
    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
    }
}

wrap_window_delegate! {
    struct SpikeWindowDelegate {}
    impl ViewDelegate {
        fn preferred_size(&self, _v: Option<&mut View>) -> Size { Size { width: 1200, height: 720 } }
    }
    impl PanelDelegate {}
    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else { return };
            let (panes, ov) = ST.with(|s| { let s = s.borrow(); (s.pane_views.clone(), s.overlay_view.clone()) });
            // Test 5: two panes with a draggable divider between them, positioned by hand (see relayout).
            let divider = ST.with(|s| s.borrow().divider_view.clone());
            let mut pd = SpikePanelDelegate::new();
            let panel = panel_create(Some(&mut pd)).expect("panel");
            // Attach the (empty) panel to the window first so the browser views' native windows are created inside it.
            let mut pv = View::from(&panel);
            window.add_child_view(Some(&mut pv));
            // The panel's box layout positions children in add order: left pane, divider, right pane.
            let order: Vec<usize> = vec![0, 1];
            let mut wrappers: Vec<Panel> = vec![];
            for (n, &i) in order.iter().enumerate() {
                if n == 1 { if let Some(d) = divider.as_ref() { let mut dv = View::from(d); panel.add_child_view(Some(&mut dv)); } }
                // wrapper panel = the coloured frame; the browser view fills it inside a BORDER-px inset
                let mut wd = SpikeWrapDelegate::new();
                let wrap = panel_create(Some(&mut wd)).expect("wrapper panel");
                wrap.set_background_color(argb(ACCENTS[i], i != 0));
                let l = wrap.set_to_box_layout(Some(&BoxLayoutSettings { horizontal: 1, inside_border_insets: Insets { top: BORDER, left: BORDER, bottom: BORDER, right: BORDER }, cross_axis_alignment: AxisAlignment::STRETCH, ..Default::default() }));
                let mut wv = View::from(&wrap);
                panel.add_child_view(Some(&mut wv));
                let mut v = View::from(&panes[i]);
                wrap.add_child_view(Some(&mut v));
                if let Some(l) = l.as_ref() { l.set_flex_for_view(Some(&mut v), 0); }
                wrappers.push(wrap);
            }
            ST.with(|s| { let mut s = s.borrow_mut(); s.panel = Some(panel.clone()); s.wrappers = wrappers; });
            update_outline();
            let layout: Option<BoxLayout> = None;
            // Test 2: overlay launcher above the live pages, custom docking, starts top-centre.
            let mut ctl = None;
            if let Some(ov) = ov {
                let mut v = View::from(&ov);
                ctl = window.add_overlay_view(Some(&mut v), DockingMode::CUSTOM, 1);
                if let Some(c) = ctl.as_ref() {
                    c.set_bounds(Some(&Rect { x: 300, y: 24, width: 600, height: 160 }));
                    c.set_visible(0);
                }
                ov.set_visible(0);
            }
            ST.with(|s| { let mut s = s.borrow_mut(); s.overlay = ctl; s.layout = layout.clone(); s.window = Some(window.clone()); });
            window.show();
            if let Some(v) = ST.with(|s| s.borrow().pane_views.first().cloned()) { v.request_focus(); }
            relayout();
            let mut r = RelayoutTask::new(); post_delayed_task(ThreadId::UI, Some(&mut r), 300);
            let mut d = DumpTask::new(); post_delayed_task(ThreadId::UI, Some(&mut d), 2500);
            println!("[m0] window created");
        }
        fn on_window_destroyed(&self, _w: Option<&mut Window>) {
            ST.with(|s| { let mut s = s.borrow_mut(); s.pane_views.clear(); s.overlay_view = None; s.overlay = None; s.divider_view = None; s.panel = None; s.window = None; });
            quit_message_loop();
        }
        fn window_runtime_style(&self) -> RuntimeStyle { if !flag("chrome") { RuntimeStyle::ALLOY } else { RuntimeStyle::DEFAULT } }
        fn can_close(&self, _w: Option<&mut Window>) -> i32 {
            // Ask every browser to close. Each one answers 0 the first time (it is shutting down) and 1 afterwards, and CEF only
            // re-tries the window close for one of them, which is why this needed a second click on X. So retry ourselves
            // until every browser agrees.
            let bs = ST.with(|s| s.borrow().browsers.clone());
            let mut ok = 1;
            for b in bs { if let Some(h) = b.host() { if h.try_close_browser() != 1 { ok = 0; } } }
            if ok == 0 { let mut t = RetryCloseTask::new(); post_delayed_task(ThreadId::UI, Some(&mut t), 150); }
            ok
        }


    }
}

wrap_task! {
    struct RelayoutTask {}
    impl Task { fn execute(&self) {
        relayout();
    } }
}

wrap_task! {
    struct WidthsTask {}
    impl Task {
        fn execute(&self) {
            let panes = ST.with(|s| s.borrow().pane_views.clone());
            if panes.len() == 2 { println!("[m0] widths after divider: A={} B={}", panes[0].bounds().width, panes[1].bounds().width); }
        }
    }
}

wrap_task! {
    struct DumpTask {}
    impl Task {
        fn execute(&self) {
            let (panes, ov, win) = ST.with(|s| { let s = s.borrow(); (s.pane_views.clone(), s.overlay_view.clone(), s.window.clone()) });
            if let Some(w) = win { let b = w.bounds(); println!("[m0] dump window {}x{}", b.width, b.height); }
            for (i, p) in panes.iter().enumerate() { let b = p.bounds(); println!("[m0] dump pane {} bounds=({},{} {}x{}) visible={} drawn={} has_browser={}", i, b.x, b.y, b.width, b.height, p.is_visible(), p.is_drawn(), p.browser().is_some()); }
            for (i, b) in ST.with(|s| s.borrow().browsers.clone()).iter().enumerate() {
                if let Some(h) = b.host() {
                    let hw = h.window_handle().0 as *mut std::ffi::c_void;
                    let mut r = Rc4 { l: 0, t: 0, r: 0, b: 0 };
                    unsafe { GetWindowRect(hw, &mut r); println!("[m0] dump native hwnd of browser #{i}: ({},{}) {}x{} visible={} parent={:?}", r.l, r.t, r.r - r.l, r.b - r.t, IsWindowVisible(hw), GetParent(hw)); }
                }
            }
            let dvw = ST.with(|s| s.borrow().divider_view.clone());
            if let Some(d) = dvw { let b = d.bounds(); println!("[m0] dump divider bounds=({},{} {}x{}) has_browser={}", b.x, b.y, b.width, b.height, d.browser().is_some()); }
            if let Some(o) = ov { let b = o.bounds(); println!("[m0] dump overlay view bounds=({},{} {}x{}) visible={} has_browser={}", b.x, b.y, b.width, b.height, o.is_visible(), o.browser().is_some()); }
        }
    }
}

wrap_task! {
    struct AutoDestroyTask {}
    impl Task { fn execute(&self) { destroy_fresh_view("timer"); } }
}

wrap_task! {
    struct RetryCloseTask {}
    impl Task {
        fn execute(&self) {
            if let Some(w) = ST.with(|s| s.borrow().window.clone()) { w.close(); }
        }
    }
}

wrap_task! {
    struct ExitTask {}
    impl Task {
        fn execute(&self) {
            println!("[m0] exit-after reached; closing window");
            if let Some(w) = ST.with(|s| s.borrow().window.clone()) { w.close(); }
        }
    }
}

fn divider_html() -> &'static str {
    r#"<!doctype html><meta charset=utf-8><body style="margin:0;height:100vh;background:#8fdde8;cursor:col-resize;user-select:none;overflow:hidden">
<script>
var n=0,down=false,lx=0;
document.body.onmousedown=function(e){down=true;lx=e.screenX;document.body.style.background='#ffd166';e.preventDefault();};
addEventListener('mouseup',function(){down=false;document.body.style.background='#8fdde8';});
addEventListener('mousemove',function(e){if(!down)return;var dx=e.screenX-lx;if(!dx)return;lx=e.screenX;document.title='div:'+dx+','+(++n);});
</script>"#
}

fn overlay_html() -> &'static str {
    r#"<!doctype html><meta charset=utf-8><body style="margin:0;font:14px system-ui;background:#1e1e2e;color:#eee;border:2px solid #8fdde8;box-sizing:border-box;height:100vh">
<div id=h style="padding:6px 10px;background:#2b2b40;cursor:move;user-select:none">Launcher (drag me) - Esc closes</div>
<input id=q autofocus placeholder="Search..." style="margin:10px;width:calc(100% - 20px);padding:8px;box-sizing:border-box;background:#111;color:#fff;border:1px solid #555">
<script>
var n=0,down=false,lx=0,ly=0;
h.onmousedown=function(e){down=true;lx=e.screenX;ly=e.screenY;e.preventDefault();};
addEventListener('mouseup',function(){down=false;});
addEventListener('mousemove',function(e){if(!down)return;var dx=e.screenX-lx,dy=e.screenY-ly;if(!dx&&!dy)return;lx=e.screenX;ly=e.screenY;document.title='drag:'+dx+','+dy+','+(++n);});
</script>"#
}

// Test 4 finding: with a valid path CEF creates the workspace profile asynchronously, so the context
// is not usable at once and browser_view_create returns None until on_request_context_initialized fires.
wrap_request_context_handler! {
    struct SpikeRch { name: String }
    impl RequestContextHandler {
        fn on_request_context_initialized(&self, _c: Option<&mut RequestContext>) {
            println!("[m0] request context initialized: {}", self.name);
            let all = ST.with(|s| { let mut s = s.borrow_mut(); s.ctx_ready += 1; s.ctx_ready == s.ctxs.len() });
            if all { build_ui(); }
        }
    }
}

fn build_ui() {
    if let Some(r) = load_ratio() { ST.with(|s| s.borrow_mut().ratio = r); println!("[m0] divider position loaded: {r:.4}"); } else { println!("[m0] no saved divider position; using 0.5"); }
    let root = std::env::current_dir().unwrap().join(".m0-data").join("chromium-root");
    let urls = [arg("url-a"), arg("url-b")];
    let mut ctxs = ST.with(|s| s.borrow().ctxs.clone());
    if flag("sharectx") { let c = ctxs[0].clone(); ctxs.push(c); }
    let mut panes = vec![];
    for (i, ctx) in ctxs.iter_mut().enumerate() {
        let role = i as u8;
        let url = if urls[i].is_empty() { "https://example.com/".to_string() } else { urls[i].clone() };
        let mut client = SpikeClient::new(role);
        let mut bvd = if !flag("chrome") { Some(SpikePaneBvd::new(i)) } else { None };
        let bv = browser_view_create(Some(&mut client), Some(&CefString::from(url.as_str())), Some(&bsettings()), None, Some(ctx), bvd.as_mut()).expect("browser view");
        println!("[m0] pane {} cef cache_path={:?} global={}", role_name(role), CefString::from(&ctx.cache_path()).to_string(), ctx.is_global());
        panes.push(bv);
    }
    let page = root.parent().unwrap().join("overlay.html");
    std::fs::write(&page, overlay_html()).unwrap();
    let ourl = format!("file:///{}", page.to_string_lossy().replace("\\", "/"));
    let mut oclient = SpikeClient::new(OVERLAY);
    let mut bvd2 = if !flag("chrome") { Some(SpikeBvd::new()) } else { None };
    let ov = if flag("noov") { None } else { Some(browser_view_create(Some(&mut oclient), Some(&CefString::from(ourl.as_str())), Some(&bsettings()), None, None, bvd2.as_mut()).expect("overlay view")) };
    let dpage = root.parent().unwrap().join("divider.html");
    std::fs::write(&dpage, divider_html()).unwrap();
    let durl = format!("file:///{}", dpage.to_string_lossy().replace("\\", "/"));
    let mut dclient = SpikeClient::new(DIVIDER);
    let mut dbvd = SpikeDividerBvd::new();
    let dv = if panes.len() == 2 && !flag("nodiv") { browser_view_create(Some(&mut dclient), Some(&CefString::from(durl.as_str())), Some(&bsettings()), None, None, Some(&mut dbvd)) } else { None };
    ST.with(|s| { let mut s = s.borrow_mut(); s.pane_views = panes; s.overlay_view = ov; s.divider_view = dv; });

    let secs: i64 = arg("exit-after").parse().unwrap_or(0);
    if secs > 0 { let mut t = ExitTask::new(); post_delayed_task(ThreadId::UI, Some(&mut t), secs * 1000); }
    let mut wd = SpikeWindowDelegate::new();
    window_create_top_level(Some(&mut wd));
}

wrap_browser_process_handler! {
    struct SpikeBph {}
    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            let root = std::env::current_dir().unwrap().join(".m0-data").join("chromium-root");
            let n = if flag("single") || flag("sharectx") { 1 } else { 2 };
            let mut ctxs = vec![];
            for ws in ["ws-a", "ws-b"].into_iter().take(n) {
                // Test 4: one request context per workspace, distinct cache_path directly under the root.
                // CEF compares these as strings against its user data dir: use the same (backslash) form as root_cache_path.
                let cache = root.join(ws);
                std::fs::create_dir_all(&cache).unwrap();
                let cp = if flag("mem-b") && ws == "ws-b" { String::new() } else { cache.to_string_lossy().to_string() };
                let rcs = RequestContextSettings { cache_path: CefString::from(cp.as_str()), persist_session_cookies: 1, ..Default::default() };
                let mut h = SpikeRch::new(ws.to_string());
                ctxs.push(request_context_create_context(Some(&rcs), Some(&mut h)).expect("request context"));
            }
            ST.with(|s| s.borrow_mut().ctxs = ctxs);
            // Contexts that are already initialised never fire the callback; kick the check once.
            let ready = ST.with(|s| s.borrow().ctxs.iter().filter(|c| c.is_global() == 0).count());
            println!("[m0] waiting for {} request context(s)", ready);
        }
    }
}

wrap_app! {
    struct SpikeApp {}
    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> { Some(SpikeBph::new()) }
        // Finding: with two or more content browser views in one window the GPU process crashed three times at startup
        // (exit code 0x80000003, then it settled) on this machine. Turning DirectComposition off avoids it.
        fn on_before_command_line_processing(&self, process_type: Option<&CefString>, command_line: Option<&mut CommandLine>) {
            let Some(cl) = command_line else { return };
            let browser_process = process_type.map(|p| p.to_string().is_empty()).unwrap_or(true);
            if browser_process && cl.has_switch(Some(&CefString::from("keep-dcomp"))) != 1 {
                cl.append_switch(Some(&CefString::from("disable-direct-composition")));
            }
        }
    }
}

fn main() {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = cef::args::Args::new();
    let cmd = args.as_cmd_line().expect("command line");
    let is_browser = cmd.has_switch(Some(&CefString::from("type"))) != 1;
    let ret = execute_process(Some(args.as_main_args()), None::<&mut App>, std::ptr::null_mut());
    if !is_browser { assert!(ret >= 0); return; }
    assert_eq!(ret, -1);

    let root = std::env::current_dir().unwrap().join(".m0-data").join("chromium-root");
    std::fs::create_dir_all(&root).unwrap();
    let settings = Settings {
        no_sandbox: 1,
        log_severity: LogSeverity::VERBOSE,
        log_file: CefString::from(root.parent().unwrap().join("cef.log").to_string_lossy().as_ref()),
        root_cache_path: CefString::from(root.to_string_lossy().as_ref()),
        ..Default::default()
    };
    let mut app = SpikeApp::new();
    assert_eq!(initialize(Some(args.as_main_args()), Some(&settings), Some(&mut app), std::ptr::null_mut()), 1);
    run_message_loop();
    shutdown();
}
