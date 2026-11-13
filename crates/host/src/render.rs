//! The render-process half of IPC v0. It runs inside every renderer, so it is deliberately tiny and only acts for the trusted shell.
//!
//! `window.m0` exists **only** in a main frame whose URL is `app://shell/...`. Ordinary pages never get the object, so they cannot call
//! it. This is the first line of defence (S6); the browser process checks again on every message (`ipc::on_message`), because a
//! compromised renderer could send a message without going through JavaScript.
//!
//! Shape: `m0.send(name, payload)` posts a process message to the browser; the browser's reply arrives here and is delivered by calling
//! `m0.onmessage(name, payload)` if the shell has set one.

use crate::{ipc, shell};
use cef::*;

fn install(ctx: &mut V8Context) {
    let Some(global) = ctx.global() else { return };
    let mut handler = SendToBrowser::new();
    let Some(mut send) =
        v8_value_create_function(Some(&CefString::from("send")), Some(&mut handler))
    else {
        return;
    };
    let Some(mut m0) = v8_value_create_object(None, None) else {
        return;
    };
    m0.set_value_bykey(
        Some(&CefString::from("send")),
        Some(&mut send),
        V8Propertyattribute::default(),
    );
    let mut null = v8_value_create_null();
    m0.set_value_bykey(
        Some(&CefString::from("onmessage")),
        null.as_mut(),
        V8Propertyattribute::default(),
    );
    global.set_value_bykey(
        Some(&CefString::from("m0")),
        Some(&mut m0),
        V8Propertyattribute::default(),
    );
}

wrap_v8_handler! {
    struct SendToBrowser {}
    impl V8Handler {
        fn execute(&self, name: Option<&CefString>, _object: Option<&mut V8Value>, arguments: Option<&[Option<V8Value>]>, _retval: Option<&mut Option<V8Value>>, exception: Option<&mut CefString>) -> i32 {
            if name.map(|n| n.to_string()).as_deref() != Some("send") { return 0 }
            let text = |i: usize| arguments.and_then(|a| a.get(i)).and_then(|v| v.as_ref()).filter(|v| v.is_string() == 1).map(|v| CefString::from(&v.string_value()).to_string());
            let (Some(msg_name), payload) = (text(0), text(1).unwrap_or_default()) else {
                if let Some(e) = exception { *e = CefString::from("m0.send(name, payload) needs a string name"); }
                return 1;
            };
            let Some(frame) = v8_context_get_current_context().and_then(|c| c.frame()) else { return 1 };
            ipc::send(&frame, ProcessId::BROWSER, &msg_name, &payload);
            1
        }
    }
}

wrap_render_process_handler! {
    pub struct Render {}
    impl RenderProcessHandler {
        fn on_context_created(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame>, ctx: Option<&mut V8Context>) {
            let (Some(frame), Some(ctx)) = (frame, ctx) else { return };
            if frame.is_main() != 1 { return }
            if shell::is_shell_url(&CefString::from(&frame.url()).to_string()) { install(ctx); }
        }
        fn on_process_message_received(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame>, source: ProcessId, message: Option<&mut ProcessMessage>) -> i32 {
            let (Some(frame), Some(message)) = (frame, message) else { return 0 };
            if source != ProcessId::BROWSER { return 0 }
            // Only the shell's own frame is ever sent to; if something else shows up, do nothing with it.
            if !shell::is_shell_url(&CefString::from(&frame.url()).to_string()) { return 0 }
            let Some((name, payload)) = ipc::read(message) else { return 0 };
            let js = format!("window.m0 && typeof window.m0.onmessage === 'function' && window.m0.onmessage({}, {});", ipc::js_string(&name), ipc::js_string(&payload));
            frame.execute_java_script(Some(&CefString::from(js.as_str())), None, 0);
            1
        }
    }
}
