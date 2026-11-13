//! The app's own error pages (load failures, certificate errors, a crashed page). Generated here, shown by the host as a `data:` address.
//!
//! The page has **no script** and a CSP that forbids everything but inline style; every piece of text that came from outside (the
//! address, CEF's error text) is HTML-escaped and cut short. There is no button that bypasses anything: a certificate error is shown, and
//! the only ways out are another address or Reload (which retries the same address and meets the same check).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The page could not be loaded (network, DNS, refused ...).
    Load,
    /// The site's certificate was not accepted. Never bypassed.
    Certificate,
    /// The page's renderer process died.
    Crashed,
}

const MAX_URL_CHARS: usize = 300;
const MAX_DETAIL_CHARS: usize = 120;

pub fn escape(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for c in text.chars().take(max_chars) {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() => out.push('?'),
            c => out.push(c),
        }
    }
    if text.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// The page for a failure. `url` is the address that failed, `detail` CEF's short error text (both untrusted).
pub fn html(kind: Kind, url: &str, detail: &str) -> String {
    let (title, lead, advice) = match kind {
        Kind::Load => (
            "This page could not be loaded",
            "The address did not answer, or the connection failed.",
            "Check the address and your connection, then reload (F5).",
        ),
        Kind::Certificate => (
            "This site's certificate is not trusted",
            "The connection was stopped because the site could not prove who it is. Nothing was sent to it.",
            "The Dev Browser does not offer a way past this. Reloading checks again; the site's owner has to fix the certificate.",
        ),
        Kind::Crashed => (
            "This page stopped working",
            "The process that draws this page ended unexpectedly. Your other tabs are not affected.",
            "Reload (F5) to try again.",
        ),
    };
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\
<title>{title}</title>\
<style>:root{{color-scheme:light dark}}body{{font:16px system-ui,sans-serif;margin:12vh auto;max-width:36em;padding:0 1.5em;line-height:1.5}}\
h1{{font-size:1.4em}}code{{word-break:break-all}}.d{{opacity:.7;font-size:.85em}}</style></head>\
<body><h1>{title}</h1><p>{lead}</p><p><code>{url}</code></p><p>{advice}</p><p class=\"d\">{detail}</p></body></html>",
        url = escape(url, MAX_URL_CHARS),
        detail = escape(detail, MAX_DETAIL_CHARS),
    )
}

/// The page as a `data:` address (every byte outside letters and digits percent-encoded).
pub fn data_url(html: &str) -> String {
    let mut out = String::from("data:text/html;charset=utf-8,");
    for b in html.bytes() {
        if b.is_ascii_alphanumeric() {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Is this address one of the app's own error pages? (So it is not stored as the tab's address.)
pub fn is_error_page(url: &str) -> bool {
    url.starts_with("data:text/html;charset=utf-8,")
}

/// Whether a CEF network error number is a certificate problem (net error codes -200 to -220).
pub fn is_certificate_error(code: i32) -> bool {
    (-220..=-200).contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_text_cannot_make_markup_or_script() {
        let h = html(
            Kind::Load,
            "http://x/\"><script>alert(1)</script>",
            "<img src=x onerror=1>",
        );
        assert!(!h.contains("<script"));
        assert!(!h.contains("<img"));
        assert!(h.contains("&lt;script&gt;"));
        assert!(h.contains("default-src 'none'"));
    }

    #[test]
    fn long_and_control_text_is_cut_and_flattened() {
        let h = html(Kind::Crashed, &"a".repeat(5000), "x\ny\u{0}");
        assert!(h.len() < 3000);
        assert!(h.contains("x?y?"));
    }

    #[test]
    fn the_certificate_page_offers_no_way_past() {
        let h = html(
            Kind::Certificate,
            "https://bad/",
            "ERR_CERT_AUTHORITY_INVALID",
        );
        assert!(!h.to_lowercase().contains("proceed"));
        assert!(!h.contains("<a "));
        assert!(!h.contains("<button"));
    }

    #[test]
    fn data_url_has_only_safe_characters_and_is_recognised() {
        let u = data_url(&html(Kind::Load, "http://a/?x=1&y=<2>", ""));
        assert!(is_error_page(&u));
        assert!(u["data:text/html;charset=utf-8,".len()..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '%' || c.is_ascii_hexdigit()));
        assert!(!is_error_page("data:text/html,<b>x"));
    }

    #[test]
    fn certificate_codes() {
        assert!(is_certificate_error(-201));
        assert!(is_certificate_error(-202));
        assert!(!is_certificate_error(-105));
        assert!(!is_certificate_error(-3));
    }
}
