//! Client handlers, split by concern. Every browser view in the app uses `HostClient`; what differs (trusted shell or ordinary page, and
//! for a page which workspace and tab it belongs to) is carried in the client.

use crate::{downloads, ipc, keys, navigation, pages, state};
use cef::*;
use dev_browser_core::commands::{self, Source};
use dev_browser_core::errorpage::{self, Kind};
use dev_browser_core::id::Id;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Shell,
    Page,
    /// The launcher overlay: as trusted as the shell, but it has its own keys (Esc closes it) and its own replies.
    Launcher,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Shell => "shell",
            Role::Page => "page",
            Role::Launcher => "launcher",
        }
    }

    fn source(self) -> Source {
        match self {
            Role::Shell | Role::Launcher => Source::Shell,
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
        // Popup policy: the native popup window is never made. A popup the user asked for (a click) to a web address becomes a new tab of
        // the same workspace, decided in core; anything else is blocked with a notice.
        fn on_before_popup(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, _popup_id: i32, target_url: Option<&CefString>, _name: Option<&CefString>,
            _disposition: WindowOpenDisposition, user_gesture: i32, _features: Option<&PopupFeatures>, _wi: Option<&mut WindowInfo>, _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>, _extra: Option<&mut Option<DictionaryValue>>, _nojs: Option<&mut i32>) -> i32 {
            let url = target_url.map(|u| u.to_string()).unwrap_or_default();
            println!("[host] popup requested (user gesture: {}): {}", user_gesture != 0, log_safe(&url, 200));
            if !self.tab.is_empty() {
                let mut task = PopupTask::new(self.workspace.clone(), url, user_gesture != 0);
                post_task(ThreadId::UI, Some(&mut task));
            }
            1
        }
        fn on_before_close(&self, b: Option<&mut Browser>) {
            let Some(mut b) = b.cloned() else { return };
            state::with(|s| {
                if let Some(i) = s.browsers.iter().position(|x| x.is_same(Some(&mut b)) != 0) { s.browsers.remove(i); }
            });
            if let Ok(tab) = Id::parse(&self.tab) { downloads::tab_closed(&tab); }
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
        fn on_load_error(&self, b: Option<&mut Browser>, frame: Option<&mut Frame>, code: Errorcode, text: Option<&CefString>, url: Option<&CefString>) {
            let Some(f) = frame else { return };
            if f.is_main() != 1 { return }
            let failed = url.map(|u| u.to_string()).unwrap_or_default();
            let detail = text.map(|t| t.to_string()).unwrap_or_default();
            let raw: sys::cef_errorcode_t = code.into();
            let number = raw as i32;
            eprintln!("[host] {} page failed to load {}: error {number} {}", self.role.name(), log_safe(&failed, 200), log_safe(&detail, 100));
            // a load the app or the page itself cancelled (a new navigation, Stop) is not an error to show; nor is our own error page
            if number == -3 || errorpage::is_error_page(&failed) || self.role != Role::Page { return }
            let (Some(b), Ok(tab)) = (b, Id::parse(&self.tab)) else { return };
            let kind = if errorpage::is_certificate_error(number) { Kind::Certificate } else { Kind::Load };
            navigation::error_page(&tab, b, kind, &failed, &format!("{detail} ({number})"));
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
            // the app's own error page: the tab keeps the address that failed
            if errorpage::is_error_page(&u) { return }
            let (Ok(ws), Ok(tab)) = (Id::parse(&self.workspace), Id::parse(&self.tab)) else { return };
            state::with(|s| s.errors.remove(&tab));
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
            // The launcher takes only Esc and Ctrl+P (both close it); every other key is for its own search box.
            if self.role == Role::Launcher {
                let closes = e.windows_key_code == 0x1B || (e.windows_key_code == 0x50 && e.modifiers & 4 != 0);
                if !closes { return 0 }
                let outcome = state::with(|s| s.backend.as_mut().map(|b| b.handle(dev_browser_core::backend::LAUNCHER_CLOSE, "")));
                if let Some(Ok(o)) = outcome { pages::deliver(o); }
                return 1;
            }
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
    struct Requests { role: Role, workspace: String, tab: String }
    impl RequestHandler {
        // A certificate error is never bypassed: returning 0 cancels the request, and the load error that follows shows the error page (S12).
        fn on_certificate_error(&self, _b: Option<&mut Browser>, cert_error: Errorcode, request_url: Option<&CefString>, _ssl: Option<&mut Sslinfo>, _cb: Option<&mut Callback>) -> i32 {
            eprintln!("[host] {} certificate error {cert_error:?} for {}: the request is cancelled", self.role.name(), log_safe(&request_url.map(|u| u.to_string()).unwrap_or_default(), 200));
            0
        }
        // The page's process died. A page gets the crash page (its tab and every other tab are unaffected); the shell is reloaded, a few
        // times at most so a shell that crashes at once does not loop.
        fn on_render_process_terminated(&self, b: Option<&mut Browser>, status: TerminationStatus, _code: i32, _text: Option<&CefString>) {
            eprintln!("[host] {} renderer process ended: {status:?}", self.role.name());
            let Some(b) = b else { return };
            match self.role {
                Role::Page => {
                    let Ok(tab) = Id::parse(&self.tab) else { return };
                    let failed = state::with(|s| s.errors.get(&tab).cloned().or_else(|| s.nav.get(&tab).map(|n| n.url.clone()))).unwrap_or_default();
                    navigation::error_page(&tab, b, Kind::Crashed, &failed, "the page's process ended");
                }
                Role::Shell | Role::Launcher => {
                    let n = state::with(|s| { s.shell_crashes += 1; s.shell_crashes });
                    if n <= 3 { eprintln!("[host] reloading the shell (attempt {n})"); b.reload(); } else { eprintln!("[host] the shell keeps crashing: not reloading it again"); }
                }
            }
        }
        fn resource_request_handler(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, _r: Option<&mut Request>, _nav: i32, _dl: i32, _init: Option<&CefString>, _disable: Option<&mut i32>) -> Option<ResourceRequestHandler> {
            Some(Protocols::new(self.role))
        }
    }
}

/// Tell the shell something about a page of this workspace (core decides whether the workspace may speak: the active one only).
pub fn tell(workspace: &str, text: &str) {
    let Ok(ws) = Id::parse(workspace) else { return };
    let reply = state::with(|s| s.backend.as_ref().and_then(|b| b.notice(&ws, text)));
    if let Some(r) = reply {
        ipc::push_to_shell(r.name, &r.payload);
    }
}

/// A page's origin, for a notice: the text is the page's own, so it is short and plain.
fn origin_label(origin: Option<&CefString>) -> String {
    log_safe(&origin.map(|o| o.to_string()).unwrap_or_default(), 60)
}

wrap_task! {
    struct PopupTask { workspace: String, url: String, gesture: bool }
    impl Task {
        fn execute(&self) {
            let Ok(ws) = Id::parse(&self.workspace) else { return };
            let outcome = state::with(|s| s.backend.as_mut().map(|b| b.popup(&ws, &self.url, self.gesture)));
            if let Some(o) = outcome { pages::deliver(o); }
        }
    }
}

wrap_download_handler! {
    struct Downloads { workspace: String, tab: String }
    impl DownloadHandler {
        // The trait's default answer is 0: no download is ever allowed. A page may start one (it is then held for the user's answer); the shell may not.
        fn can_download(&self, _b: Option<&mut Browser>, _url: Option<&CefString>, _method: Option<&CefString>) -> i32 { i32::from(!self.tab.is_empty()) }
        fn on_before_download(&self, _b: Option<&mut Browser>, _item: Option<&mut DownloadItem>, suggested_name: Option<&CefString>, cb: Option<&mut BeforeDownloadCallback>) -> i32 {
            let (Some(cb), Some(name)) = (cb, suggested_name) else { return 0 };
            downloads::offer(&self.workspace, &self.tab, &name.to_string(), cb)
        }
        fn on_download_updated(&self, _b: Option<&mut Browser>, item: Option<&mut DownloadItem>, _cb: Option<&mut DownloadItemCallback>) {
            let Some(item) = item else { return };
            if item.is_complete() != 0 { println!("[host] a download finished"); }
            else if item.is_canceled() != 0 { println!("[host] a download was cancelled"); }
        }
    }
}

// Finding (M0): CEF's own JavaScript dialogs crashed the app when their view was destroyed while one was open. Until the real
// dialog UI exists (M1.6 and M4) the app answers every dialog itself with the safe choice: cancel, and allow leaving a page.
wrap_jsdialog_handler! {
    struct Dialogs { role: Role, workspace: String }
    impl JsdialogHandler {
        fn on_jsdialog(&self, _b: Option<&mut Browser>, origin: Option<&CefString>, _t: JsdialogType, _m: Option<&CefString>, _d: Option<&CefString>, cb: Option<&mut JsdialogCallback>, _suppress: Option<&mut i32>) -> i32 {
            eprintln!("[host] {} dialog from {:?} cancelled by the app", self.role.name(), origin.map(|o| o.to_string()));
            if self.role == Role::Page { tell(&self.workspace, &format!("A dialog from {} was dismissed.", origin_label(origin))); }
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
    struct Permissions { role: Role, workspace: String }
    impl PermissionHandler {
        fn on_show_permission_prompt(&self, _b: Option<&mut Browser>, _id: u64, origin: Option<&CefString>, _perms: u32, cb: Option<&mut PermissionPromptCallback>) -> i32 {
            eprintln!("[host] {} permission prompt from {:?} denied by default", self.role.name(), origin.map(|o| o.to_string()));
            if self.role == Role::Page { tell(&self.workspace, &format!("{} asked for a permission; it was denied.", origin_label(origin))); }
            if let Some(cb) = cb { cb.cont(PermissionRequestResult::DENY); }
            1
        }
        fn on_request_media_access_permission(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, origin: Option<&CefString>, _perms: u32, cb: Option<&mut MediaAccessCallback>) -> i32 {
            eprintln!("[host] {} camera/microphone request from {:?} denied by default", self.role.name(), origin.map(|o| o.to_string()));
            if self.role == Role::Page { tell(&self.workspace, &format!("{} asked for the camera or microphone; it was denied.", origin_label(origin))); }
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
            // a page's renderer answering a selection request (page.selection.read): handled apart from the trusted shell's messages
            if self.role == Role::Page && frame.is_main() == 1 {
                if let Some((name, payload)) = ipc::read(message) {
                    if name == crate::selection::REPLY {
                        crate::selection::on_reply(&self.tab, &payload);
                        return 1;
                    }
                }
            }
            ipc::on_message(self.role, frame, message) as i32
        }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> { Some(Lifetime::new(self.workspace.clone(), self.tab.clone())) }
        fn load_handler(&self) -> Option<LoadHandler> { Some(Loading::new(self.role, self.tab.clone())) }
        fn display_handler(&self) -> Option<DisplayHandler> { Some(Titles::new(self.role, self.workspace.clone(), self.tab.clone())) }
        fn keyboard_handler(&self) -> Option<KeyboardHandler> { Some(Keys::new(self.role)) }
        fn request_handler(&self) -> Option<RequestHandler> { Some(Requests::new(self.role, self.workspace.clone(), self.tab.clone())) }
        fn jsdialog_handler(&self) -> Option<JsdialogHandler> { Some(Dialogs::new(self.role, self.workspace.clone())) }
        fn permission_handler(&self) -> Option<PermissionHandler> { Some(Permissions::new(self.role, self.workspace.clone())) }
        fn download_handler(&self) -> Option<DownloadHandler> { Some(Downloads::new(self.workspace.clone(), self.tab.clone())) }
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
