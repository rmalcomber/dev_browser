//! IPC plumbing shared by both processes: how a message (name + text payload) is packed into a CEF process message, and the
//! browser-side gate that decides whether to believe one. What a message *means* is decided by `dev_browser_core::backend::Backend`;
//! this module only carries the replies and executes the effects it returns.

use crate::{handlers::Role, pages, shell, state};
use cef::*;
use dev_browser_core::backend::Effect;

/// Process-message name carrying every IPC v0 message; the real message name is argument 0.
const WIRE: &str = "m0.msg";

pub fn send(frame: &Frame, to: ProcessId, name: &str, payload: &str) {
    let Some(mut msg) = process_message_create(Some(&CefString::from(WIRE))) else {
        return;
    };
    if let Some(args) = msg.argument_list() {
        args.set_string(0, Some(&CefString::from(name)));
        args.set_string(1, Some(&CefString::from(payload)));
    }
    frame.send_process_message(to, Some(&mut msg));
}

/// Unpack a message, or `None` if it is not an IPC v0 message.
pub fn read(message: &ProcessMessage) -> Option<(String, String)> {
    if CefString::from(&message.name()).to_string() != WIRE {
        return None;
    }
    let args = message.argument_list()?;
    Some((
        CefString::from(&args.string(0)).to_string(),
        CefString::from(&args.string(1)).to_string(),
    ))
}

/// Should the browser process believe a message from this view and frame? Only the trusted shell view, and only while its main frame is
/// still an `app://shell/` page. Everything else is dropped and logged (S6).
pub fn trusted(role: Role, frame_url: &str) -> bool {
    role == Role::Shell && shell::is_shell_url(frame_url)
}

/// Browser side: a message arrived from a renderer.
pub fn on_message(role: Role, frame: &Frame, message: &ProcessMessage) -> bool {
    let Some((name, payload)) = read(message) else {
        return false;
    };
    let url = CefString::from(&frame.url()).to_string();
    if !trusted(role, &url) {
        eprintln!(
            "[host] dropped IPC message {:?} from an untrusted {} frame ({url})",
            name.chars().take(64).collect::<String>(),
            role.name()
        );
        return true;
    }
    // The backend decides; the borrow of the state ends before any CEF call below.
    let outcome = state::with(|s| s.backend.as_mut().map(|b| b.handle(&name, &payload)));
    match outcome {
        Some(Ok(outcome)) => {
            for effect in outcome.effects {
                match effect {
                    Effect::Activate(id) => pages::show(&id),
                    Effect::Delete(id) => pages::delete(&id),
                }
            }
            for reply in outcome.replies {
                send(frame, ProcessId::RENDERER, reply.name, &reply.payload);
            }
        }
        Some(Err(why)) => eprintln!("[host] refused IPC message: {why}"),
        None => eprintln!("[host] dropped IPC message: no backend"),
    }
    true
}

/// A string as a JavaScript string literal, safe to put in code: quotes, backslashes, line breaks, `</script>` lookalikes and the two
/// Unicode line separators are all escaped.
pub fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_shell_view_on_a_shell_page_is_trusted() {
        assert!(trusted(Role::Shell, "app://shell/index.html"));
        assert!(
            !trusted(Role::Page, "app://shell/index.html"),
            "a page view never is, whatever its URL says"
        );
        assert!(
            !trusted(Role::Shell, "https://example.com/"),
            "a shell view that has navigated away is not"
        );
        assert!(!trusted(Role::Shell, ""));
    }

    #[test]
    fn js_strings_cannot_break_out() {
        assert_eq!(js_string("plain"), "\"plain\"");
        assert_eq!(js_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(js_string("x\ny\r"), "\"x\\ny\\r\"");
        assert_eq!(js_string("</script>"), "\"\\u003c/script\\u003e\"");
        assert_eq!(
            js_string("\u{2028}\u{2029}\u{1}"),
            "\"\\u2028\\u2029\\u0001\""
        );
        assert_eq!(js_string("héllo ✓"), "\"héllo ✓\"");
    }
}
