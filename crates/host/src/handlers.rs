//! Client handlers, split by concern. Every browser view in the app uses `HostClient`; what differs (trusted shell or ordinary page, and
//! for a page which workspace and tab it belongs to) is carried in the client.

use crate::{ipc, keys, navigation, pages, state};
use cef::*;
use dev_browser_core::commands::{self, Source};
use dev_browser_core::id::Id;

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

    fn source(self) -> Source {
        match self {
            Role::Shell => Source::Shell,
            Role::Page => Source::Page,
        }
    }
}

// Tracks live browsers so the window can ask each to close, ends the app when the last one is gone, and tells core when a tab's page
// closed itself.
wrap_life_span_handler! {
    struct Lifetime { workspace: String, tab: String }
    impl LifeSpanHandler {
        fn on_after_created(&self, b: Option<&mut Browser>) {
            if let Some(b) = b { state::with(|s| s.browsers.push(b.clone())); }
        }
        fn on_before_close(&self, b: Option<&mut Browser>) {
            let Some(mut b) = b.cloned() else { return };
            state::with(|s| {
                if let Some(i) = s.browsers.iter().position(|x| x.is_same(Some(&mut b)) != 0) { s.browsers.remove(i); }
            });
            if !self.tab.is_empty() { pages::browser_closed(&self.workspace, &self.tab); }
            let (left, window) = state::with(|s| (s.browsers.len(), s.window.clone()));
            if left == 0 { if let Some(w) = window { w.close(); } }
        }
    }
}

wrap_load_handler! {
    struct Loading { role: Role, tab: String }
    impl LoadHandler {
        // Loading state and whether the tab can go back or forward: kept per tab, and sent to the shell while the tab is the shown one.
        fn on_loading_state_change(&self, _b: Option<&mut Browser>, is_loading: i32, can_go_back: i32, can_go_forward: i32) {
            let Ok(tab) = Id::parse(&self.tab) else { return };
            navigation::update(&tab, |n| {
                n.loading = is_loading != 0;
                n.can_back = can_go_back != 0;
                n.can_forward = can_go_forward != 0;
            });
        }
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

// A page's title and address go to its tab record (core cleans and checks them) and on to the shell. Titles are also logged so the test
// pages can report back (tools/s6-hostile-page.ps1, tools/m13-tabs.ps1).
wrap_display_handler! {
    struct Titles { role: Role, workspace: String, tab: String }
    impl DisplayHandler {
        fn on_title_change(&self, _b: Option<&mut Browser>, title: Option<&CefString>) {
            let t = title.map(|t| t.to_string()).unwrap_or_default();
            println!("[host] {} title: {}", self.role.name(), log_safe(&t, 600));
            if self.tab.is_empty() { return }
            let (Ok(ws), Ok(tab)) = (Id::parse(&self.workspace), Id::parse(&self.tab)) else { return };
            navigation::update(&tab, |n| n.title = t.clone());
            let push = state::with(|s| s.backend.as_mut().and_then(|b| b.tab_title(&ws, &tab, &t)));
            if let Some(r) = push { ipc::push_to_shell(r.name, &r.payload); }
        }
        fn on_address_change(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if self.tab.is_empty() { return }
            if frame.map(|f| f.is_main()) != Some(1) { return }
            let u = url.map(|u| u.to_string()).unwrap_or_default();
            let (Ok(ws), Ok(tab)) = (Id::parse(&self.workspace), Id::parse(&self.tab)) else { return };
            navigation::update(&tab, |n| n.url = u.clone());
            let push = state::with(|s| s.backend.as_mut().and_then(|b| b.tab_url(&ws, &tab, &u)));
            if let Some(r) = push { ipc::push_to_shell(r.name, &r.payload); }
        }
        // The page announced its icon: fetched in this tab's own browser (so its own workspace's context) and handed to core.
        fn on_favicon_urlchange(&self, b: Option<&mut Browser>, icon_urls: Option<&mut CefStringList>) {
            if self.tab.is_empty() { return }
            let (Some(b), Some(list)) = (b, icon_urls) else { return };
            let urls = navigation::string_list(list);
            navigation::favicon_urls_changed(&self.workspace, &self.tab, b, urls);
        }
    }
}

// Window-level shortcuts: every view's keyboard handler asks the registry, so no shortcut depends on which view has focus. A matched chord
// is swallowed (the page never sees it). Only key-down events are matched.
wrap_keyboard_handler! {
    struct Keys { role: Role }
    impl KeyboardHandler {
        fn on_pre_key_event(&self, _b: Option<&mut Browser>, event: Option<&KeyEvent>, _os: Option<&mut sys::MSG>, _shortcut: Option<&mut i32>) -> i32 {
            let Some(e) = event else { return 0 };
            if e.type_ != KeyEventType::RAWKEYDOWN { return 0 }
            let Some(chord) = keys::chord_from_event(e.windows_key_code, e.modifiers) else { return 0 };
            let source = self.role.source();
            let Some(cmd) = commands::lookup(&chord, source) else { return 0 };
            println!("[host] shortcut {} -> {} (from {})", chord.display(), cmd.id, self.role.name());
            let outcome = state::with(|s| s.backend.as_mut().map(|b| b.run_command(cmd.id, source)));
            if let Some(o) = outcome { pages::deliver(o); }
            1
        }
    }
}

// A page that navigates to a scheme the app does not handle (mailto:, ms-settings:, app:, ...) would otherwise hand the address to the
// operating system to launch whatever is registered for it. The app denies every such launch and logs it; a prompt of its own (which
// scheme, which page asked, allow once) comes with the safety nets in M1.6.
wrap_resource_request_handler! {
    struct Protocols { role: Role }
    impl ResourceRequestHandler {
        // The trait's default answer is 0, which is CANCEL: without this every request of every page would be cancelled.
        fn on_before_resource_load(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, _r: Option<&mut Request>, _cb: Option<&mut Callback>) -> ReturnValue { ReturnValue::CONTINUE }
        fn on_protocol_execution(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, request: Option<&mut Request>, allow_os_execution: Option<&mut i32>) {
            if let Some(a) = allow_os_execution { *a = 0; }
            let url = request.map(|r| CefString::from(&r.url()).to_string()).unwrap_or_default();
            eprintln!("[host] {} blocked a launch of an external protocol: {}", self.role.name(), log_safe(&url, 200));
        }
    }
}

wrap_request_handler! {
    struct Requests { role: Role }
    impl RequestHandler {
        fn resource_request_handler(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, _r: Option<&mut Request>, _nav: i32, _dl: i32, _init: Option<&CefString>, _disable: Option<&mut i32>) -> Option<ResourceRequestHandler> {
            Some(Protocols::new(self.role))
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

// `workspace` and `tab` are the ids of the tab this view shows (empty for the shell).
wrap_client! {
    pub struct HostClient { role: Role, workspace: String, tab: String }
    impl Client {
        fn on_process_message_received(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame>, source: ProcessId, message: Option<&mut ProcessMessage>) -> i32 {
            let (Some(frame), Some(message)) = (frame, message) else { return 0 };
            if source != ProcessId::RENDERER { return 0 }
            ipc::on_message(self.role, frame, message) as i32
        }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> { Some(Lifetime::new(self.workspace.clone(), self.tab.clone())) }
        fn load_handler(&self) -> Option<LoadHandler> { Some(Loading::new(self.role, self.tab.clone())) }
        fn display_handler(&self) -> Option<DisplayHandler> { Some(Titles::new(self.role, self.workspace.clone(), self.tab.clone())) }
        fn keyboard_handler(&self) -> Option<KeyboardHandler> { Some(Keys::new(self.role)) }
        fn request_handler(&self) -> Option<RequestHandler> { Some(Requests::new(self.role)) }
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
