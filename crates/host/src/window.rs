//! The top-level window: the trusted shell, the workspace pages beside it, and a clean close.

use crate::{
    handlers::{HostClient, Role},
    layout, pages, shell, state,
};
use cef::*;

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
    let mut delegate = Frame::new();
    window_create_top_level(Some(&mut delegate));
}

wrap_window_delegate! {
    struct Frame {}
    impl ViewDelegate {
        fn preferred_size(&self, _v: Option<&mut View>) -> Size { Size { width: 1200, height: 720 } }
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
            window.set_title(Some(&CefString::from("The Dev Browser")));
            state::with(|s| { s.panel = Some(panel); s.window = Some(window.clone()); });
            window.show();
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
            state::with(|s| { s.shell = None; s.tabs.clear(); s.contexts.clear(); s.ready.clear(); s.waiting.clear(); s.panel = None; s.window = None; });
            quit_message_loop();
        }
        fn window_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
        // Ask every browser to close. Each answers "not yet" the first time and CEF retries the window close for only one of them, which
        // needed a second click on X in M0. So retry here until every browser agrees.
        fn can_close(&self, _w: Option<&mut Window>) -> i32 {
            // from here on browsers going away are the window closing, not tabs being closed
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
