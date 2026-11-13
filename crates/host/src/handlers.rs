//! Client handlers, split by concern. Every browser view in the app uses `HostClient`; what differs (trusted shell or ordinary page)
//! is the `Role`.

use crate::{ipc, state};
use cef::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Shell,
    Page,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Shell => "shell",
            Role::Page => "page",
        }
    }
}

// Tracks live browsers so the window can ask each to close, and ends the app when the last one is gone.
wrap_life_span_handler! {
    struct Lifetime {}
    impl LifeSpanHandler {
        fn on_after_created(&self, b: Option<&mut Browser>) {
            if let Some(b) = b { state::with(|s| s.browsers.push(b.clone())); }
        }
        fn on_before_close(&self, b: Option<&mut Browser>) {
            let Some(mut b) = b.cloned() else { return };
            let (left, window) = state::with(|s| {
                if let Some(i) = s.browsers.iter().position(|x| x.is_same(Some(&mut b)) != 0) { s.browsers.remove(i); }
                (s.browsers.len(), s.window.clone())
            });
            if left == 0 { if let Some(w) = window { w.close(); } }
        }
    }
}

wrap_load_handler! {
    struct Loading { role: Role }
    impl LoadHandler {
        fn on_load_end(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame>, status: i32) {
            let Some(f) = frame else { return };
            if f.is_main() == 1 { println!("[host] {} loaded (status {status}): {}", self.role.name(), CefString::from(&f.url())); }
        }
        fn on_load_error(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame>, code: Errorcode, text: Option<&CefString>, url: Option<&CefString>) {
            let Some(f) = frame else { return };
            if f.is_main() == 1 { eprintln!("[host] {} failed to load {:?}: {code:?} {:?}", self.role.name(), url.map(|u| u.to_string()), text.map(|t| t.to_string())); }
        }
    }
}

/// A page's title is untrusted text. For a log line: control characters become `?` (no fake log lines) and it is cut short.
pub fn log_safe(text: &str, max_chars: usize) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .take(max_chars)
        .collect()
}

// Titles are logged for pages so the S6 test page can report what it saw (tools/s6-hostile-page.ps1); the title bar will use them in M1.5.
wrap_display_handler! {
    struct Titles { role: Role }
    impl DisplayHandler {
        fn on_title_change(&self, _b: Option<&mut Browser>, title: Option<&CefString>) {
            let t = title.map(|t| t.to_string()).unwrap_or_default();
            println!("[host] {} title: {}", self.role.name(), log_safe(&t, 600));
        }
    }
}

// Finding (M0): CEF's own JavaScript dialogs crashed the app when their view was destroyed while one was open. Until the real
// dialog UI exists (M1.6 and M4) the app answers every dialog itself with the safe choice: cancel, and allow leaving a page.
wrap_jsdialog_handler! {
    struct Dialogs { role: Role }
    impl JsdialogHandler {
        fn on_jsdialog(&self, _b: Option<&mut Browser>, origin: Option<&CefString>, _t: JsdialogType, _m: Option<&CefString>, _d: Option<&CefString>, cb: Option<&mut JsdialogCallback>, _suppress: Option<&mut i32>) -> i32 {
            eprintln!("[host] {} dialog from {:?} cancelled by the app", self.role.name(), origin.map(|o| o.to_string()));
            if let Some(cb) = cb { cb.cont(0, Some(&CefString::from(""))); }
            1
        }
        fn on_before_unload_dialog(&self, _b: Option<&mut Browser>, _m: Option<&CefString>, _reload: i32, cb: Option<&mut JsdialogCallback>) -> i32 {
            if let Some(cb) = cb { cb.cont(1, Some(&CefString::from(""))); }
            1
        }
    }
}

// Permission prompts are denied until the app has its own prompt UI (M1.6 safe default, M4 real prompts).
wrap_permission_handler! {
    struct Permissions { role: Role }
    impl PermissionHandler {
        fn on_show_permission_prompt(&self, _b: Option<&mut Browser>, _id: u64, origin: Option<&CefString>, _perms: u32, cb: Option<&mut PermissionPromptCallback>) -> i32 {
            eprintln!("[host] {} permission prompt from {:?} denied by default", self.role.name(), origin.map(|o| o.to_string()));
            if let Some(cb) = cb { cb.cont(PermissionRequestResult::DENY); }
            1
        }
        fn on_request_media_access_permission(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, origin: Option<&CefString>, _perms: u32, cb: Option<&mut MediaAccessCallback>) -> i32 {
            eprintln!("[host] {} camera/microphone request from {:?} denied by default", self.role.name(), origin.map(|o| o.to_string()));
            if let Some(cb) = cb { cb.cancel(); }
            1
        }
    }
}

wrap_client! {
    pub struct HostClient { role: Role }
    impl Client {
        fn on_process_message_received(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame>, source: ProcessId, message: Option<&mut ProcessMessage>) -> i32 {
            let (Some(frame), Some(message)) = (frame, message) else { return 0 };
            if source != ProcessId::RENDERER { return 0 }
            ipc::on_message(self.role, frame, message) as i32
        }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> { Some(Lifetime::new()) }
        fn load_handler(&self) -> Option<LoadHandler> { Some(Loading::new(self.role)) }
        fn display_handler(&self) -> Option<DisplayHandler> { Some(Titles::new(self.role)) }
        fn jsdialog_handler(&self) -> Option<JsdialogHandler> { Some(Dialogs::new(self.role)) }
        fn permission_handler(&self) -> Option<PermissionHandler> { Some(Permissions::new(self.role)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_text_cannot_forge_lines_or_run_on() {
        assert_eq!(log_safe("a\nb\r[host] fake\t", 100), "a?b?[host] fake?");
        assert_eq!(log_safe(&"x".repeat(1000), 600).len(), 600);
    }
}
