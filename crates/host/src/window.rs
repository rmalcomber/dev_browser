//! The top-level window: the trusted shell, the workspace pages beside it, and a clean close.

use crate::{
    handlers::{HostClient, Role},
    layout, pages, shell, state,
};
use cef::*;
use dev_browser_core::settings::WindowState;

/// Create the shell view and the window. Workspace pages are added later by `pages` (the first one when the window exists).
pub fn open() {
    let mut shell_client = HostClient::new(Role::Shell, String::new(), String::new());
    let mut shell_style = layout::ViewStyle::new(false);
    // no request context: the shell uses CEF's global one, so it shares no cookies or storage with any workspace
    let Some(shell) = browser_view_create(
        Some(&mut shell_client),
        Some(&CefString::from(shell::START_URL)),
        Some(&layout::browser_settings()),
        None,
        None,
        Some(&mut shell_style),
    ) else {
        eprintln!("[host] could not create the shell view");
        quit_message_loop();
        return;
    };
    state::with(|s| s.shell = Some(shell));
    let launcher_view = crate::launcher::create_view();
    state::with(|s| s.pending_launcher = launcher_view);
    let side_view = crate::sidepanel::create_view();
    state::with(|s| s.pending_side = side_view);
    let mut delegate = Frame::new();
    window_create_top_level(Some(&mut delegate));
}

wrap_window_delegate! {
    struct Frame {}
    impl ViewDelegate {
        fn preferred_size(&self, _v: Option<&mut View>) -> Size {
            let w = state::with(|s| s.backend.as_ref().map(|b| b.window_state())).unwrap_or(WindowState::DEFAULT);
            Size { width: w.width, height: w.height }
        }
    }
    impl PanelDelegate {}
    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else { return };
            let mut events = layout::PanelEvents::new();
            let Some(panel) = panel_create(Some(&mut events)) else { return };
            // The panel goes into the window first so the browser views' native windows are created inside it.
            let mut pv = View::from(&panel);
            window.add_child_view(Some(&mut pv));
            if let Some(shell) = state::with(|s| s.shell.clone()) {
                let mut vs = View::from(&shell);
                panel.add_child_view(Some(&mut vs));
            }
            // the panel first, then the launcher: a later overlay is drawn above an earlier one, and the launcher must be above the panel
            if let Some(v) = state::with(|s| s.pending_side.take()) { crate::sidepanel::attach(window, v); }
            if let Some(v) = state::with(|s| s.pending_launcher.take()) { crate::launcher::attach(window, v); }
            window.set_title(Some(&CefString::from("The Dev Browser")));
            state::with(|s| { s.panel = Some(panel); s.window = Some(window.clone()); });
            window.show();
            if state::with(|s| s.backend.as_ref().is_some_and(|b| b.window_state().maximized)) { window.maximize(); }
            layout::changed();
            // The first layout pass can come before the panel has its final size; look again shortly.
            let mut again = Relayout::new();
            post_delayed_task(ThreadId::UI, Some(&mut again), 300);
            // the window exists, so the selected tab can be shown in it
            if let Some(outcome) = state::with(|s| s.backend.as_mut().map(|b| b.startup())) {
                pages::deliver(outcome);
            }
        }
        fn on_window_destroyed(&self, _w: Option<&mut Window>) {
            state::with(|s| { s.shell = None; s.launcher = None; s.launcher_ctl = None; s.side_view = None; s.side_ctl = None; s.tabs.clear(); s.contexts.clear(); s.ready.clear(); s.waiting.clear(); s.panel = None; s.window = None; });
            quit_message_loop();
        }
        fn window_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
        // The title bar has minimise, maximise and close buttons (the Views default has only close), and the window can be resized.
        fn can_resize(&self, _w: Option<&mut Window>) -> i32 { 1 }
        fn can_maximize(&self, _w: Option<&mut Window>) -> i32 { 1 }
        fn can_minimize(&self, _w: Option<&mut Window>) -> i32 { 1 }
        // Ask every browser to close. Each answers "not yet" the first time and CEF retries the window close for only one of them, which
        // needed a second click on X in M0. So retry here until every browser agrees.
        fn can_close(&self, _w: Option<&mut Window>) -> i32 {
            // from here on browsers going away are the window closing, not tabs being closed
            if let Some(w) = _w { save_window_state(w); }
            let browsers = state::with(|s| { s.shutting_down = true; s.browsers.clone() });
            let mut all = 1;
            for b in browsers { if let Some(h) = b.host() { if h.try_close_browser() != 1 { all = 0; } } }
            if all == 0 { let mut t = RetryClose::new(); post_delayed_task(ThreadId::UI, Some(&mut t), 150); }
            all
        }
    }
}

wrap_task! {
    struct Relayout {}
    impl Task { fn execute(&self) { layout::relayout(); } }
}

wrap_task! {
    struct RetryClose {}
    impl Task { fn execute(&self) { if let Some(w) = state::with(|s| s.window.clone()) { w.close(); } } }
}

/// Remember the window's size (and whether it is maximised) for the next start. A maximised window keeps the size it had when it was
/// not, so un-maximising later gives a sensible window. Saved at every close attempt, which happens once per real close.
fn save_window_state(w: &mut Window) {
    let maximized = w.is_maximized() != 0;
    let saved = state::with(|s| s.backend.as_ref().map(|b| b.window_state()));
    let Some(saved) = saved else { return };
    let size = w.size();
    let next = if maximized {
        WindowState::new(saved.width, saved.height, true)
    } else {
        WindowState::new(size.width, size.height, false)
    };
    state::with(|s| {
        if let Some(b) = s.backend.as_ref() {
            b.set_window_state(next)
        }
    });
    println!("[host] window state saved: {}", next.format());
}
