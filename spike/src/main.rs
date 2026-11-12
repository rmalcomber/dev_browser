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

const PANE_A: u8 = 0;
const PANE_B: u8 = 1;
const DIVIDER: u8 = 8;
const OVERLAY: u8 = 9;
const VK_P: i32 = 0x50;
const VK_1: i32 = 0x31;
const VK_2: i32 = 0x32;
const VK_ESCAPE: i32 = 0x1B;
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
    ctxs: Vec<RequestContext>,
    ctx_ready: usize,
}

thread_local! { static ST: RefCell<St> = RefCell::new(St::default()); }

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
// Views' BoxLayout flex weights did not re-flow when changed, so the three views are positioned by hand.
const DIV_W: i32 = 6;
const MIN_PANE: i32 = 120;

fn relayout() {
    let (panes, divider, ratio) = ST.with(|s| { let s = s.borrow(); (s.pane_views.clone(), s.divider_view.clone(), if s.ratio > 0.0 { s.ratio } else { 0.5 }) });
    let (Some(divider), true) = (divider, panes.len() == 2) else { return };
    let Some(panel) = ST.with(|s| s.borrow().panel.clone()) else { return };
    let size = panel.size();
    if size.width <= 0 || size.height <= 0 { return }
    let avail = size.width - DIV_W;
    let wa = ((avail as f32 * ratio) as i32).clamp(MIN_PANE, (avail - MIN_PANE).max(MIN_PANE));
    panes[0].set_bounds(Some(&Rect { x: 0, y: 0, width: wa, height: size.height }));
    divider.set_bounds(Some(&Rect { x: wa, y: 0, width: DIV_W, height: size.height }));
    panes[1].set_bounds(Some(&Rect { x: wa + DIV_W, y: 0, width: avail - wa, height: size.height }));
}

fn on_divider_title(t: &str) {
    let Some(rest) = t.strip_prefix("div:") else { return };
    let p: Vec<i32> = rest.split(',').filter_map(|x| x.parse().ok()).collect();
    if p.len() != 2 { return }
    let panes = ST.with(|s| {
        let mut s = s.borrow_mut();
        if p[1] <= s.div_seen { return vec![]; }
        s.div_seen = p[1];
        s.pane_views.clone()
    });
    if panes.len() != 2 { return }
    let (wa, wb) = (panes[0].bounds().width, panes[1].bounds().width);
    let avail = wa + wb;
    let new_a = (wa + p[0]).clamp(MIN_PANE, (avail - MIN_PANE).max(MIN_PANE));
    ST.with(|s| s.borrow_mut().ratio = new_a as f32 / avail as f32);
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
            if self.role != OVERLAY { ST.with(|s| s.borrow_mut().last_pane = self.role as usize); }
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
            if empty { println!("[m0] all browsers closed"); quit_message_loop(); }
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
    }
}

wrap_browser_view_delegate! {
    struct SpikeBvd {}
    impl ViewDelegate {}
    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
    }
}

// A Panel without a layout: its children are positioned by hand in relayout().
wrap_panel_delegate! {
    struct SpikePanelDelegate {}
    impl ViewDelegate {
        fn on_layout_changed(&self, _v: Option<&mut View>, _b: Option<&Rect>) { relayout(); }
    }
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
            let mut panel = panel_create(Some(&mut pd)).expect("panel");
            for (i, bv) in panes.iter().enumerate() {
                if i == 1 { if let Some(d) = divider.as_ref() { let mut dv = View::from(d); panel.add_child_view(Some(&mut dv)); } }
                let mut v = View::from(bv);
                panel.add_child_view(Some(&mut v));
            }
            let mut pv = View::from(&panel);
            window.add_child_view(Some(&mut pv));
            ST.with(|s| s.borrow_mut().panel = Some(panel.clone()));
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
            ST.with(|s| { let mut s = s.borrow_mut(); s.pane_views.clear(); s.overlay_view = None; s.overlay = None; s.window = None; });
        }
        fn window_runtime_style(&self) -> RuntimeStyle { if !flag("chrome") { RuntimeStyle::ALLOY } else { RuntimeStyle::DEFAULT } }
        fn on_window_bounds_changed(&self, _w: Option<&mut Window>, _b: Option<&Rect>) { relayout(); }
        fn can_close(&self, _w: Option<&mut Window>) -> i32 {
            let bs = ST.with(|s| s.borrow().browsers.clone());
            let mut ok = 1;
            for b in bs { if let Some(h) = b.host() { if h.try_close_browser() != 1 { ok = 0; } } }
            ok
        }
    }
}

wrap_task! {
    struct RelayoutTask {}
    impl Task { fn execute(&self) { relayout(); } }
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
            let dvw = ST.with(|s| s.borrow().divider_view.clone());
            if let Some(d) = dvw { let b = d.bounds(); println!("[m0] dump divider bounds=({},{} {}x{}) has_browser={}", b.x, b.y, b.width, b.height, d.browser().is_some()); }
            if let Some(o) = ov { let b = o.bounds(); println!("[m0] dump overlay view bounds=({},{} {}x{}) visible={} has_browser={}", b.x, b.y, b.width, b.height, o.is_visible(), o.browser().is_some()); }
        }
    }
}

wrap_task! {
    struct ExitTask {}
    impl Task {
        fn execute(&self) {
            println!("[m0] exit-after reached; closing window");
            let bs = ST.with(|s| s.borrow().browsers.clone());
            for b in bs { if let Some(h) = b.host() { h.close_browser(1); } }
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
    let root = std::env::current_dir().unwrap().join(".m0-data").join("chromium-root");
    let urls = [arg("url-a"), arg("url-b")];
    let mut ctxs = ST.with(|s| s.borrow().ctxs.clone());
    let mut panes = vec![];
    for (i, ctx) in ctxs.iter_mut().enumerate() {
        let role = i as u8;
        let url = if urls[i].is_empty() { "https://example.com/".to_string() } else { urls[i].clone() };
        let mut client = SpikeClient::new(role);
        let mut bvd = if !flag("chrome") { Some(SpikeBvd::new()) } else { None };
        let bv = browser_view_create(Some(&mut client), Some(&CefString::from(url.as_str())), Some(&BrowserSettings::default()), None, Some(ctx), bvd.as_mut()).expect("browser view");
        println!("[m0] pane {} cef cache_path={:?} global={}", role_name(role), CefString::from(&ctx.cache_path()).to_string(), ctx.is_global());
        panes.push(bv);
    }
    let page = root.parent().unwrap().join("overlay.html");
    std::fs::write(&page, overlay_html()).unwrap();
    let ourl = format!("file:///{}", page.to_string_lossy().replace("\\", "/"));
    let mut oclient = SpikeClient::new(OVERLAY);
    let mut bvd2 = if !flag("chrome") { Some(SpikeBvd::new()) } else { None };
    let ov = browser_view_create(Some(&mut oclient), Some(&CefString::from(ourl.as_str())), Some(&BrowserSettings::default()), None, None, bvd2.as_mut()).expect("overlay view");
    let dpage = root.parent().unwrap().join("divider.html");
    std::fs::write(&dpage, divider_html()).unwrap();
    let durl = format!("file:///{}", dpage.to_string_lossy().replace("\\", "/"));
    let mut dclient = SpikeClient::new(DIVIDER);
    let mut dbvd = SpikeDividerBvd::new();
    let dv = if panes.len() == 2 { browser_view_create(Some(&mut dclient), Some(&CefString::from(durl.as_str())), Some(&BrowserSettings::default()), None, None, Some(&mut dbvd)) } else { None };
    ST.with(|s| { let mut s = s.borrow_mut(); s.pane_views = panes; s.overlay_view = Some(ov); s.divider_view = dv; });

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
            let n = if flag("single") { 1 } else { 2 };
            let mut ctxs = vec![];
            for ws in ["ws-a", "ws-b"].into_iter().take(n) {
                // Test 4: one request context per workspace, distinct cache_path directly under the root.
                // CEF compares these as strings against its user data dir: use the same (backslash) form as root_cache_path.
                let cache = root.join(ws);
                std::fs::create_dir_all(&cache).unwrap();
                let rcs = RequestContextSettings { cache_path: CefString::from(cache.to_string_lossy().as_ref()), persist_session_cookies: 1, ..Default::default() };
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
