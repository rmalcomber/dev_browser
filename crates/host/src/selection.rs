//! `page.selection.read`: asking the shown page for the text the user has selected, on the user action of opening the launcher.
//!
//! One request at a time, with a one-time id, to **the shown tab's main frame only** (S7: with one pane that is the focused pane; split
//! view will bind it to the focused pane). The renderer evaluates `window.getSelection()` and answers `sel.rep` with the id and the text.
//! The browser accepts the answer only from the tab it asked, only for the id it is waiting for, only once, and hands the text to core,
//! which keeps it in memory until the launcher closes. The text is never logged here: only its length.
//!
//! A page can say what it likes about its own selection (it is the page's own content); nothing the page does reaches anything else.

use crate::{ipc, state};
use cef::*;
use dev_browser_core::id::Id;

pub const REQUEST: &str = "sel.req";
pub const REPLY: &str = "sel.rep";
/// The most text taken from a page, in bytes. Core caps it again.
pub const MAX_BYTES: usize = 48 * 1024;

/// Ask the shown tab for its selection. Does nothing if there is no page to ask.
pub fn request() {
    let target = state::with(|s| {
        let tab = s.shown.clone()?;
        let frame = s.tabs.get(&tab)?.view.browser()?.main_frame()?;
        s.selection_seq = s.selection_seq.wrapping_add(1);
        let id = s.selection_seq;
        s.selection_pending = Some((id, tab));
        Some((id, frame))
    });
    if let Some((id, frame)) = target {
        ipc::send(&frame, ProcessId::RENDERER, REQUEST, &id.to_string());
    }
}

/// Split a reply payload `id \n text`.
pub fn parse_reply(payload: &str) -> Option<(u32, &str)> {
    let (id, text) = payload.split_once('\n')?;
    Some((id.parse().ok()?, text))
}

/// A page's renderer answered. Believed only if it is the tab and the id that were asked, and only once.
pub fn on_reply(tab: &str, payload: &str) {
    let (Ok(tab), Some((id, text))) = (Id::parse(tab), parse_reply(payload)) else {
        return;
    };
    let accepted = state::with(|s| match &s.selection_pending {
        Some((want, from)) if *want == id && *from == tab && s.shown.as_ref() == Some(&tab) => {
            s.selection_pending = None;
            true
        }
        _ => false,
    });
    if !accepted {
        eprintln!("[host] ignored a selection answer that was not asked for");
        return;
    }
    let text = if text.len() > MAX_BYTES {
        // cut at a character boundary
        let mut end = MAX_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    } else {
        text
    };
    println!(
        "[host] page selection received ({} characters)",
        text.chars().count()
    );
    let reply = state::with(|s| s.backend.as_mut().and_then(|b| b.set_selection(text)));
    if let Some(r) = reply {
        ipc::push_to_launcher(r.name, &r.payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_are_an_id_then_the_text_and_nothing_else_parses() {
        assert_eq!(parse_reply("7\nhello\nworld"), Some((7, "hello\nworld")));
        assert_eq!(parse_reply("7\n"), Some((7, "")));
        for bad in ["", "7", "x\ny", "-1\ny", "99999999999\ny", "\ny"] {
            assert_eq!(parse_reply(bad), None, "{bad:?}");
        }
    }
}
