//! The top-level window: creates the views once the profile is ready, and closes cleanly.

use crate::{
    handlers::{HostClient, Role},
    layout, state,
};
use cef::*;

/// The trusted shell. A placeholder page until `app://shell` serves `ui/dist` (M1.1 task 6).
const SHELL_PLACEHOLDER: &str = "data:text/html;charset=utf-8,<body style='margin:0;background:%23252526;color:%23ccc;font:14px system-ui;padding:12px'>The Dev Browser<br><small>alpha</small>";

/// Called when the workspace profile is ready: build both views and the window.
pub fn open() {
    let config = state::config();
    let mut ctx = state::with(|s| s.context.clone());

    let mut shell_client = HostClient::new(Role::Shell);
    let mut shell_style = layout::ViewStyle::new(false);
    let shell = browser_view_create(
        Some(&mut shell_client),
        Some(&CefString::from(SHELL_PLACEHOLDER)),
        Some(&layout::browser_settings()),
        None,
        None,
        Some(&mut shell_style),
    );

    let mut page_client = HostClient::new(Role::Page);
    let mut page_style = layout::ViewStyle::new(true);
    let page = browser_view_create(
        Some(&mut page_client),
        Some(&CefString::from(config.start_url.as_str())),
        Some(&layout::browser_settings()),
        None,
        ctx.as_mut(),
        Some(&mut page_style),
    );

    let (Some(shell), Some(page)) = (shell, page) else {
        eprintln!("[host] could not create the views (profile not usable?)");
        quit_message_loop();
        return;
    };
    state::with(|s| {
        s.shell = Some(shell);
        s.page = Some(page);
    });
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
            let (shell, page) = state::with(|s| (s.shell.clone(), s.page.clone()));
            if let (Some(shell), Some(page)) = (shell, page) {
                let (mut vs, mut vp) = (View::from(&shell), View::from(&page));
                panel.add_child_view(Some(&mut vs));
                panel.add_child_view(Some(&mut vp));
            }
            window.set_title(Some(&CefString::from("The Dev Browser")));
            state::with(|s| { s.panel = Some(panel); s.window = Some(window.clone()); });
            window.show();
            if let Some(p) = state::with(|s| s.page.clone()) { p.request_focus(); }
            layout::relayout();
            // The first layout pass can come before the panel has its final size; look again shortly.
            let mut again = Relayout::new();
            post_delayed_task(ThreadId::UI, Some(&mut again), 300);
        }
        fn on_window_destroyed(&self, _w: Option<&mut Window>) {
            state::with(|s| { s.shell = None; s.page = None; s.panel = None; s.window = None; });
            quit_message_loop();
        }
        fn window_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
        // Ask every browser to close. Each answers "not yet" the first time and CEF retries the window close for only one of them, which
        // needed a second click on X in M0. So retry here until every browser agrees.
        fn can_close(&self, _w: Option<&mut Window>) -> i32 {
            let browsers = state::with(|s| s.browsers.clone());
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
