//! The CEF `App` and the browser-process handler: startup switches and the moment the profile can be created.

use crate::{render, shell, window};
use cef::*;

/// Finding (M0): with two or more content views in one window the GPU process crashed at start-up on the development machine.
/// Turning DirectComposition off avoided it; the cause is unknown, so this stays in one place, easy to remove after a
/// second-machine test (planning/m1-plan.md, risks). `--keep-dcomp` on the command line leaves it on, to test that.
fn gpu_workaround(process_type: Option<&CefString>, cl: &mut CommandLine) {
    let browser_process = process_type
        .map(|p| p.to_string().is_empty())
        .unwrap_or(true);
    if browser_process && cl.has_switch(Some(&CefString::from("keep-dcomp"))) != 1 {
        cl.append_switch(Some(&CefString::from("disable-direct-composition")));
    }
}

wrap_app! {
    pub struct HostApp {}
    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> { Some(HostBrowserProcess::new()) }
        fn render_process_handler(&self) -> Option<RenderProcessHandler> { Some(render::Render::new()) }
        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            if let Some(r) = registrar { shell::register_scheme(r); }
        }
        fn on_before_command_line_processing(&self, process_type: Option<&CefString>, command_line: Option<&mut CommandLine>) {
            if let Some(cl) = command_line { gpu_workaround(process_type, cl); }
        }
    }
}

wrap_browser_process_handler! {
    struct HostBrowserProcess {}
    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            shell::register_factory();
            window::open();
        }
    }
}
