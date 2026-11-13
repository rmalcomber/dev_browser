//! What the user typed in the address bar, as an address to load. Search-or-URL entry in one field, decided here (core decides, the host
//! only loads what it is told), and strict: the result is always an `http`/`https` address (or the blank page) that `url::is_restorable`
//! accepts, so typing `javascript:...`, `data:...`, `file:...`, `chrome:...` or the app's own `app:` can never load anything.
//!
//! Rules, in order:
//! 1. an address that already starts with `http://` or `https://` is used as typed;
//! 2. `about:blank` is the blank page;
//! 3. text with no spaces that begins `something:` and is not a host and port (`localhost:3000`, `example.com:8080/x`) is a scheme the app
//!    does not open: refused with a message, never searched or loaded;
//! 4. text with no spaces that looks like a host (`example.com`, `localhost:3000`, `192.168.1.1/admin`, `[::1]:8080`) becomes an address:
//!    `http://` for `localhost` and IP literals (the usual dev servers), `https://` for everything else;
//! 5. anything else is a web search.

use crate::url;

/// Where searches go. DuckDuckGo: no account or tracking needed. Becomes a setting later.
pub const SEARCH_URL: &str = "https://duckduckgo.com/?q=";
/// The longest input accepted. Keeps the result under `url::MAX_URL_BYTES` even when every byte of a search is percent-encoded.
pub const MAX_INPUT_BYTES: usize = 2048;

#[derive(Debug, PartialEq, Eq)]
pub enum AddressError {
    Empty,
    TooLong,
    /// Control characters (a newline pasted in, say).
    NotAllowed,
    /// A scheme other than http and https (the text before the colon).
    Unsupported(String),
}

impl std::fmt::Display for AddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddressError::Empty => write!(f, "type an address or something to search for"),
            AddressError::TooLong => write!(f, "that is too long to open or search for"),
            AddressError::NotAllowed => {
                write!(f, "that contains characters that cannot be in an address")
            }
            AddressError::Unsupported(_) => {
                write!(f, "only web addresses (http and https) can be opened here")
            }
        }
    }
}

impl std::error::Error for AddressError {}

/// The `scheme` of text that starts `scheme:` (letters, digits, `+`, `-`, `.`, starting with a letter), if it does.
fn leading_scheme(text: &str) -> Option<&str> {
    let end = text.find(':')?;
    let scheme = &text[..end];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic()
        || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    Some(scheme)
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}

/// A DNS-style name: non-empty labels of letters, digits, `-`, `_` or non-ASCII characters, at least one dot, a last label of at least two
/// characters that is not all digits (so `3.14` and `1.5.2` are not hosts, but `1.2.3.4` is an IP literal, handled separately).
fn is_dns_name(host: &str) -> bool {
    if host.starts_with('.') || host.ends_with('.') || host.len() > 253 {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let ok_label = |l: &str| {
        !l.is_empty()
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    };
    let last = labels[labels.len() - 1];
    labels.iter().all(|l| ok_label(l))
        && last.chars().count() >= 2
        && !last.chars().all(|c| c.is_ascii_digit())
}

/// The scheme to put in front of text that looks like a host, or `None` if it does not look like one.
fn scheme_for_host(text: &str) -> Option<&'static str> {
    let authority_end = text.find(['/', '?', '#']).unwrap_or(text.len());
    let authority = &text[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return None; // an email address or user info: search, never guess
    }
    let (host, port_ok) = if let Some(rest) = authority.strip_prefix('[') {
        // [::1] or [::1]:8080
        let close = rest.find(']')?;
        let (inner, after) = (&rest[..close], &rest[close + 1..]);
        if !inner.contains(':')
            || !inner
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return None;
        }
        let port_ok = after.is_empty()
            || (after.starts_with(':')
                && after.len() > 1
                && after[1..].bytes().all(|b| b.is_ascii_digit()));
        return port_ok.then_some("http");
    } else if let Some((h, p)) = authority.rsplit_once(':') {
        (
            h,
            !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()),
        )
    } else {
        (authority, true)
    };
    if !port_ok {
        return None;
    }
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") || is_ipv4(&lower) {
        Some("http")
    } else if is_dns_name(&lower) {
        Some("https")
    } else {
        None
    }
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Turn what was typed into the address to load. See the module comment for the rules.
pub fn resolve(input: &str) -> Result<String, AddressError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(AddressError::Empty);
    }
    if text.len() > MAX_INPUT_BYTES {
        return Err(AddressError::TooLong);
    }
    if text.chars().any(char::is_control) {
        return Err(AddressError::NotAllowed);
    }
    let lower = text.to_ascii_lowercase();
    let checked = |address: String| {
        if url::is_restorable(&address) {
            Ok(address)
        } else {
            Err(AddressError::NotAllowed)
        }
    };

    if lower.starts_with("http://") || lower.starts_with("https://") {
        return checked(text.to_string());
    }
    if lower == "about:blank" {
        return Ok("about:blank".to_string());
    }
    if !text.chars().any(char::is_whitespace) {
        if let Some(scheme) = leading_scheme(text) {
            // `localhost:3000` and `example.com:8080/x` look like schemes but are a host and a port
            let rest = &text[scheme.len() + 1..];
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let after = rest[digits..].chars().next();
            let is_port = digits > 0
                && digits <= 5
                && matches!(after, None | Some('/') | Some('?') | Some('#'));
            if !is_port {
                return Err(AddressError::Unsupported(scheme.to_ascii_lowercase()));
            }
        }
        if let Some(scheme) = scheme_for_host(text) {
            return checked(format!("{scheme}://{text}"));
        }
    }
    checked(format!("{SEARCH_URL}{}", percent_encode(text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(input: &str) -> String {
        resolve(input).unwrap_or_else(|e| panic!("{input:?}: {e:?}"))
    }
    fn search(term_encoded: &str) -> String {
        format!("{SEARCH_URL}{term_encoded}")
    }

    #[test]
    fn web_addresses_are_used_as_typed() {
        assert_eq!(
            ok("https://example.com/a?b=c#d"),
            "https://example.com/a?b=c#d"
        );
        assert_eq!(ok("http://localhost:8080/x"), "http://localhost:8080/x");
        assert_eq!(ok("  HTTPS://Example.com  "), "HTTPS://Example.com");
        assert_eq!(ok("about:blank"), "about:blank");
        assert_eq!(ok("ABOUT:BLANK"), "about:blank");
    }

    #[test]
    fn host_like_text_gets_a_scheme() {
        assert_eq!(ok("example.com"), "https://example.com");
        assert_eq!(ok("Example.COM/path?q=1"), "https://Example.COM/path?q=1");
        assert_eq!(
            ok("sub.example.co.uk:8443/x"),
            "https://sub.example.co.uk:8443/x"
        );
        assert_eq!(ok("m\u{fc}nchen.de"), "https://m\u{fc}nchen.de");
        assert_eq!(ok("my-site_1.example.org"), "https://my-site_1.example.org");
    }

    #[test]
    fn local_and_ip_addresses_get_http() {
        assert_eq!(ok("localhost"), "http://localhost");
        assert_eq!(ok("localhost:3000"), "http://localhost:3000");
        assert_eq!(
            ok("localhost:3000/admin?x=1"),
            "http://localhost:3000/admin?x=1"
        );
        assert_eq!(ok("app.localhost"), "http://app.localhost");
        assert_eq!(ok("127.0.0.1:8000"), "http://127.0.0.1:8000");
        assert_eq!(ok("192.168.1.1/admin"), "http://192.168.1.1/admin");
        assert_eq!(ok("[::1]:8080/x"), "http://[::1]:8080/x");
        assert_eq!(ok("[2001:db8::1]"), "http://[2001:db8::1]");
    }

    #[test]
    fn everything_else_is_a_search() {
        assert_eq!(ok("rust borrow checker"), search("rust%20borrow%20checker"));
        assert_eq!(
            ok("intranet"),
            search("intranet"),
            "a single word with no dot is a search"
        );
        assert_eq!(ok("3.14"), search("3.14"), "a number is not a host");
        assert_eq!(ok("1.5.2"), search("1.5.2"));
        assert_eq!(ok("what is 2+2?"), search("what%20is%202%2B2%3F"));
        assert_eq!(ok("caf\u{e9}"), search("caf%C3%A9"));
        assert_eq!(
            ok("me@example.com"),
            search("me%40example.com"),
            "an email address is searched, not opened as user info"
        );
        assert_eq!(
            ok("example.com is down"),
            search("example.com%20is%20down"),
            "spaces mean a search"
        );
        assert_eq!(
            ok("foo: bar"),
            search("foo%3A%20bar"),
            "a colon followed by a space is part of a search"
        );
        assert_eq!(ok("a&b=c d"), search("a%26b%3Dc%20d"));
    }

    #[test]
    fn schemes_other_than_http_are_refused_not_searched_or_loaded() {
        for (input, scheme) in [
            ("javascript:alert(1)", "javascript"),
            ("JavaScript:alert(1)", "javascript"),
            ("data:text/html,<b>x</b>", "data"),
            ("file:///C:/Windows/win.ini", "file"),
            ("app://shell/index.html", "app"),
            ("chrome://settings", "chrome"),
            ("view-source:https://example.com/", "view-source"),
            ("mailto:me@example.com", "mailto"),
            ("ftp://example.com/", "ftp"),
            ("ms-settings:privacy", "ms-settings"),
            ("blob:https://example.com/id", "blob"),
            ("about:srcdoc", "about"),
            ("devtools://devtools/bundled/inspector.html", "devtools"),
            ("tel:+15551234", "tel"),
        ] {
            assert_eq!(
                resolve(input),
                Err(AddressError::Unsupported(scheme.to_string())),
                "{input}"
            );
        }
    }

    #[test]
    fn a_host_and_port_is_not_mistaken_for_a_scheme() {
        assert_eq!(ok("example.com:8080"), "https://example.com:8080");
        assert_eq!(ok("localhost:1"), "http://localhost:1");
        // but too many digits, or something after the digits, is not a port
        assert_eq!(
            resolve("localhost:123456"),
            Err(AddressError::Unsupported("localhost".into()))
        );
        assert_eq!(
            resolve("localhost:80x"),
            Err(AddressError::Unsupported("localhost".into()))
        );
    }

    #[test]
    fn empty_long_and_control_characters_are_refused() {
        assert_eq!(resolve(""), Err(AddressError::Empty));
        assert_eq!(resolve("   \t "), Err(AddressError::Empty));
        assert_eq!(
            resolve(&"a".repeat(MAX_INPUT_BYTES + 1)),
            Err(AddressError::TooLong)
        );
        assert!(
            resolve(&"a b ".repeat(MAX_INPUT_BYTES / 4)).is_ok(),
            "right at the limit is fine"
        );
        assert_eq!(
            resolve("example.com\nhttps://evil.example"),
            Err(AddressError::NotAllowed)
        );
        assert_eq!(resolve("bad\u{7}beep"), Err(AddressError::NotAllowed));
    }

    #[test]
    fn whatever_is_typed_the_result_is_always_openable() {
        // the guarantee the rest of the app relies on: the result passes the same check a stored tab address does
        let nasty = [
            "javascript:alert(1)",
            "//evil.example",
            "///",
            "http:///x",
            "https://",
            "http://?q",
            "\\\\server\\share",
            "a:b",
            ":",
            "::",
            "[",
            "[]",
            "[::1",
            "example.com:",
            "example.com:/x",
            "-.com",
            "a..b",
            ".com",
            "x.",
            "1.2.3",
            "256.1.1.1",
            "..",
            "/",
            "?",
            "#",
            "%00",
            "<script>",
            "ex ample.com",
            "\u{202e}moc.elpmaxe",
            "\u{0}",
            &"\u{1F600}".repeat(500),
        ];
        for input in nasty {
            if let Ok(address) = resolve(input) {
                assert!(
                    url::is_restorable(&address) || address == "about:blank",
                    "{input:?} -> {address:?}"
                );
            }
        }
    }

    #[test]
    fn a_search_term_is_fully_percent_encoded() {
        let address = ok("\"><script>alert(1)</script> & ' % #hash");
        assert!(address.starts_with(SEARCH_URL));
        let query = &address[SEARCH_URL.len()..];
        assert!(query.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'%')), "{query}");
        assert!(
            !query.contains('<')
                && !query.contains('"')
                && !query.contains('#')
                && !query.contains(' ')
        );
    }

    #[test]
    fn messages_do_not_echo_the_input() {
        for input in ["javascript:alert('s3cr3t')", "data:s3cr3t"] {
            let msg = resolve(input).unwrap_err().to_string();
            assert!(!msg.contains("s3cr3t"), "{msg}");
        }
    }
}
