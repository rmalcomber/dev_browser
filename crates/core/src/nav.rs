//! The `nav` message the host pushes to the shell for the shown tab: where it is, what it is called, whether it is loading, and whether it can
//! go back or forward. The address and title come from a page, so they are cleaned here before they reach the shell: control characters are
//! removed and both are cut short.

use crate::id::Id;
use crate::ipc::Reply;
use crate::tab::clean_title;
use crate::url::MAX_URL_BYTES;
use serde::Serialize;

#[derive(Serialize)]
struct NavOut<'a> {
    tab: &'a str,
    url: String,
    title: String,
    loading: bool,
    #[serde(rename = "canBack")]
    can_back: bool,
    #[serde(rename = "canForward")]
    can_forward: bool,
}

/// An address as shown in the address bar: no control characters, at most `MAX_URL_BYTES` (cut on a character boundary).
pub fn clean_display_url(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(MAX_URL_BYTES));
    for c in raw.chars().filter(|c| !c.is_control()) {
        if out.len() + c.len_utf8() > MAX_URL_BYTES {
            break;
        }
        out.push(c);
    }
    out
}

pub fn reply(
    tab: &Id,
    url: &str,
    title: &str,
    loading: bool,
    can_back: bool,
    can_forward: bool,
) -> Reply {
    Reply {
        name: "nav",
        payload: serde_json::to_string(&NavOut {
            tab: tab.as_str(),
            url: clean_display_url(url),
            title: clean_title(title),
            loading,
            can_back,
            can_forward,
        })
        .expect("nav serialises"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn the_reply_carries_the_state_with_camel_case_names() {
        let id = Id::new("tab");
        let r = reply(&id, "https://example.com/", "Example", true, true, false);
        assert_eq!(r.name, "nav");
        let v: Value = serde_json::from_str(&r.payload).unwrap();
        assert_eq!(v["tab"], id.as_str());
        assert_eq!(v["url"], "https://example.com/");
        assert_eq!(v["title"], "Example");
        assert_eq!(
            (
                v["loading"].as_bool(),
                v["canBack"].as_bool(),
                v["canForward"].as_bool()
            ),
            (Some(true), Some(true), Some(false))
        );
    }

    #[test]
    fn page_supplied_text_is_cleaned() {
        let id = Id::new("tab");
        let r = reply(
            &id,
            "https://example.com/\n\u{7}x",
            "  A\nB\u{0}  ",
            false,
            false,
            false,
        );
        let v: Value = serde_json::from_str(&r.payload).unwrap();
        assert_eq!(v["url"], "https://example.com/x");
        assert_eq!(v["title"], "A B");
        let long = format!("https://e.com/{}", "\u{e9}".repeat(MAX_URL_BYTES));
        let url = clean_display_url(&long);
        assert!(url.len() <= MAX_URL_BYTES && url.is_char_boundary(url.len()));
    }
}
