//! Which URLs may be stored in a tab record and loaded again later. A tab record is restored automatically at the next start and on
//! recreating a discarded view, so the rule is strict: only ordinary web pages (and the blank page). Anything else a page navigates to
//! (`javascript:`, `data:`, `blob:`, `file:`, `chrome:`, the app's own `app:`) is never persisted, so it can never be re-opened by the
//! app on the user's behalf.

/// Longest URL stored. Longer ones are not restorable (and not worth storing).
pub const MAX_URL_BYTES: usize = 8192;

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(prefix))
}

/// True if `url` may be stored in a tab record and loaded again.
pub fn is_restorable(url: &str) -> bool {
    if url.is_empty() || url.len() > MAX_URL_BYTES {
        return false;
    }
    // no control characters or whitespace: a URL with a newline or a space is not one the app should be re-opening
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    if url.eq_ignore_ascii_case("about:blank") {
        return true;
    }
    for scheme in ["http://", "https://"] {
        if starts_with_ci(url, scheme) {
            // something must follow the scheme, and it must not start with another separator (`http:///x`, `https://?x`)
            let rest = &url[scheme.len()..];
            return rest
                .chars()
                .next()
                .is_some_and(|c| !matches!(c, '/' | '?' | '#' | '\\'));
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_web_pages_and_blank_are_restorable() {
        for ok in [
            "https://example.com/",
            "http://localhost:8080/a?b=c#d",
            "HTTPS://EXAMPLE.COM/",
            "about:blank",
            "ABOUT:BLANK",
            "https://user@host/",
        ] {
            assert!(is_restorable(ok), "{ok}");
        }
    }

    #[test]
    fn everything_else_is_not() {
        for bad in [
            "",
            "javascript:alert(1)",
            "data:text/html,<b>x</b>",
            "blob:https://example.com/uuid",
            "file:///C:/Windows/win.ini",
            "app://shell/index.html",
            "chrome://settings",
            "devtools://devtools/bundled/inspector.html",
            "view-source:https://example.com/",
            "ftp://example.com/",
            "about:srcdoc",
            "https://",
            "http:///x",
            "https://?x",
            "https:/example.com",
            "example.com",
            "https://exa mple.com/",
            "https://example.com/\n",
            "https://example.com/\u{7f}",
            "  https://example.com/",
        ] {
            assert!(!is_restorable(bad), "{bad:?}");
        }
        assert!(!is_restorable(&format!(
            "https://example.com/{}",
            "a".repeat(MAX_URL_BYTES)
        )));
    }

    #[test]
    fn a_url_at_the_limit_is_fine_and_multibyte_input_does_not_panic() {
        let at_limit = format!("https://e.com/{}", "a".repeat(MAX_URL_BYTES - 14));
        assert_eq!(at_limit.len(), MAX_URL_BYTES);
        assert!(is_restorable(&at_limit));
        // slicing by bytes must not split a multi-byte character
        assert!(!is_restorable("ééééééééé"));
        assert!(!is_restorable("https:é"));
    }
}
