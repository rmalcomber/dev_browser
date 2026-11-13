//! The bundled tools: pure functions from text to a structured result. A tool gets its input and the current time and **nothing else**: it
//! cannot reach a tab, a workspace, a database, a file, the network or the clipboard, because no function here is given a handle to any
//! of them and this module imports none (a test checks that). That is the whole sandbox for bundled code, which is why bundled tools are
//! plain Rust and not a JavaScript realm shared with privileged UI (planning/m3-plan.md, T1).
//!
//! Output is a list of labelled values the user can copy, and notes. Tool input is often sensitive (a token): nothing here logs, stores or
//! keeps it, and the callers keep it in memory only (S9).

use serde::Serialize;

pub const MAX_INPUT_BYTES: usize = 48 * 1024;

/// What the launcher shows about a tool.
#[derive(Debug, Clone, Copy)]
pub struct ToolInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    /// Does the tool read the input at all? (The UUID generator does not.)
    pub takes_input: bool,
    /// Is the input usually a secret? Such input is never persisted, and a pinned widget must not store it.
    pub sensitive: bool,
}

pub const TOOLS: &[ToolInfo] = &[
    ToolInfo {
        id: "jwt",
        name: "JWT decoder",
        description:
            "Decode a token: header, claims, times and expiry. Nothing is verified or stored.",
        keywords: &["jwt", "token", "bearer", "claims", "decode", "oauth"],
        takes_input: true,
        sensitive: true,
    },
    ToolInfo {
        id: "json",
        name: "JSON formatter",
        description: "Check JSON and show it indented or minified.",
        keywords: &["json", "format", "pretty", "minify", "validate"],
        takes_input: true,
        sensitive: false,
    },
    ToolInfo {
        id: "base64",
        name: "Base64",
        description: "Decode or encode Base64 (standard and URL-safe).",
        keywords: &["base64", "encode", "decode", "b64"],
        takes_input: true,
        sensitive: true,
    },
    ToolInfo {
        id: "url",
        name: "URL parts and encoding",
        description: "Take an address apart, and percent-encode or decode text.",
        keywords: &[
            "url",
            "uri",
            "query",
            "percent",
            "encode",
            "decode",
            "parameters",
        ],
        takes_input: true,
        sensitive: false,
    },
    ToolInfo {
        id: "uuid",
        name: "UUID generator",
        description: "A fresh random (version 4) identifier in the usual spellings.",
        keywords: &["uuid", "guid", "id", "random", "generate"],
        takes_input: false,
        sensitive: false,
    },
];

pub fn info(id: &str) -> Option<&'static ToolInfo> {
    TOOLS.iter().find(|t| t.id == id)
}

/// One labelled value. `copy` values get a copy button; `block` values are long text shown as a block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub label: String,
    pub value: String,
    pub block: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Output {
    pub fields: Vec<Field>,
    pub notes: Vec<String>,
}

impl Output {
    fn field(&mut self, label: impl Into<String>, value: impl Into<String>) {
        self.fields.push(Field {
            label: label.into(),
            value: value.into(),
            block: false,
        });
    }
    fn block(&mut self, label: impl Into<String>, value: impl Into<String>) {
        self.fields.push(Field {
            label: label.into(),
            value: value.into(),
            block: true,
        });
    }
    fn note(&mut self, text: impl Into<String>) {
        self.notes.push(text.into());
    }
}

/// Which tools probably suit this text (a selection), so the launcher can put them first. Cheap guesses, never a promise.
pub fn suggest(text: &str) -> Vec<&'static str> {
    let t = text.trim();
    let mut out = Vec::new();
    let t = t
        .strip_prefix("Bearer ")
        .or_else(|| t.strip_prefix("bearer "))
        .unwrap_or(t)
        .trim();
    let parts: Vec<&str> = t.split('.').collect();
    if (parts.len() == 3 || parts.len() == 2)
        && t.len() > 20
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'=')
        })
    {
        out.push("jwt");
    }
    if (t.starts_with('{') || t.starts_with('['))
        && serde_json::from_str::<serde::de::IgnoredAny>(t).is_ok()
    {
        out.push("json");
    }
    if t.contains("://") && !t.contains(char::is_whitespace) {
        out.push("url");
    }
    out
}

/// Run a tool. `now` is the current time in seconds since 1970 (only the JWT decoder uses it). An error is a message for the user and never
/// repeats the input.
pub fn run(id: &str, input: &str, now: i64) -> Result<Output, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err("that is too much text for this tool".into());
    }
    match id {
        "jwt" => jwt(input, now),
        "json" => json(input),
        "base64" => base64(input),
        "url" => url_tool(input),
        "uuid" => Ok(uuid()),
        _ => Err("there is no such tool".into()),
    }
}

// ---- Base64 ----

const STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64_encode(data: &[u8], alphabet: &[u8; 64], pad: bool) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..=chunk.len() {
            out.push(alphabet[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
        if pad {
            for _ in chunk.len()..3 {
                out.push('=');
            }
        }
    }
    out
}

/// Decode either alphabet, with or without padding, ignoring whitespace. `None` if it is not Base64.
fn b64_decode(text: &str) -> Option<Vec<u8>> {
    let mut vals = Vec::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' => vals.push(b - b'A'),
            b'a'..=b'z' => vals.push(b - b'a' + 26),
            b'0'..=b'9' => vals.push(b - b'0' + 52),
            b'+' | b'-' => vals.push(62),
            b'/' | b'_' => vals.push(63),
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => {}
            _ => return None,
        }
    }
    if vals.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(vals.len() * 3 / 4);
    for chunk in vals.chunks(4) {
        let mut n = 0u32;
        for (i, v) in chunk.iter().enumerate() {
            n |= u32::from(*v) << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

fn base64(input: &str) -> Result<Output, String> {
    let mut o = Output::default();
    let t = input.trim();
    if t.is_empty() {
        return Err("type or paste some text".into());
    }
    match b64_decode(t) {
        Some(bytes) if !bytes.is_empty() => match String::from_utf8(bytes.clone()) {
            Ok(text) => o.block("Decoded (text)", text),
            Err(_) => {
                let hex: String = bytes.iter().take(64).map(|b| format!("{b:02x}")).collect();
                o.block(
                    format!(
                        "Decoded ({} bytes, first {} as hex)",
                        bytes.len(),
                        bytes.len().min(64)
                    ),
                    hex,
                );
            }
        },
        _ => o.note("This is not valid Base64, so it is only shown encoded."),
    }
    o.block(
        "Encoded (standard)",
        b64_encode(input.as_bytes(), STD, true),
    );
    o.block(
        "Encoded (URL-safe, no padding)",
        b64_encode(input.as_bytes(), URL, false),
    );
    Ok(o)
}

// ---- JSON ----

/// Re-indent or minify JSON text that is already known to be valid, keeping the key order and every value exactly as written.
fn reformat(text: &str, indent: Option<usize>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    let (mut in_str, mut esc) = (false, false);
    let chars: Vec<char> = text.chars().collect();
    let newline = |out: &mut String, depth: usize| {
        if let Some(n) = indent {
            out.push('\n');
            out.extend(std::iter::repeat_n(' ', n * depth));
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
        } else {
            match c {
                '"' => {
                    in_str = true;
                    out.push(c);
                }
                '{' | '[' => {
                    out.push(c);
                    // an empty object or array stays on one line
                    let mut j = i + 1;
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                        out.push(chars[j]);
                        i = j;
                    } else {
                        depth += 1;
                        newline(&mut out, depth);
                    }
                }
                '}' | ']' => {
                    depth = depth.saturating_sub(1);
                    newline(&mut out, depth);
                    out.push(c);
                }
                ',' => {
                    out.push(c);
                    newline(&mut out, depth);
                }
                ':' => {
                    out.push(c);
                    if indent.is_some() {
                        out.push(' ');
                    }
                }
                c if c.is_whitespace() => {}
                c => out.push(c),
            }
        }
        i += 1;
    }
    out
}

fn json(input: &str) -> Result<Output, String> {
    let t = input.trim();
    if t.is_empty() {
        return Err("type or paste some JSON".into());
    }
    if let Err(e) = serde_json::from_str::<serde::de::IgnoredAny>(t) {
        // the position only: serde's own text can quote the input
        return Err(format!(
            "not valid JSON (line {}, column {})",
            e.line(),
            e.column()
        ));
    }
    let mut o = Output::default();
    o.block("Formatted", reformat(t, Some(2)));
    o.block("Minified", reformat(t, None));
    Ok(o)
}

// ---- URL ----

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn percent_decode(text: &str, plus_is_space: bool) -> String {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len()
                && b[i + 1].is_ascii_hexdigit()
                && b[i + 2].is_ascii_hexdigit() =>
            {
                out.push(u8::from_str_radix(&text[i + 1..i + 3], 16).unwrap_or(b'?'));
                i += 3;
            }
            b'+' if plus_is_space => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn url_tool(input: &str) -> Result<Output, String> {
    let t = input.trim();
    if t.is_empty() {
        return Err("type or paste an address or some text".into());
    }
    let mut o = Output::default();
    if let Some((scheme, rest)) = t.split_once("://").filter(|(s, _)| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    }) {
        o.field("Scheme", scheme);
        let (rest, fragment) = rest
            .split_once('#')
            .map_or((rest, None), |(a, b)| (a, Some(b)));
        let (rest, query) = rest
            .split_once('?')
            .map_or((rest, None), |(a, b)| (a, Some(b)));
        let (authority, path) = rest
            .find('/')
            .map_or((rest, ""), |i| (&rest[..i], &rest[i..]));
        let host_part = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        if authority.contains('@') {
            o.note("The address contains a user name (and perhaps a password) before the host; it is not shown here.");
        }
        let (host, port) = match host_part.rsplit_once(':') {
            Some((h, p)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => (h, Some(p)),
            _ => (host_part, None),
        };
        o.field("Host", host);
        if let Some(p) = port {
            o.field("Port", p);
        }
        o.field("Path", if path.is_empty() { "/" } else { path });
        if let Some(q) = query {
            for pair in q.split('&').filter(|p| !p.is_empty()) {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                o.field(
                    format!("? {}", percent_decode(k, true)),
                    percent_decode(v, true),
                );
            }
        }
        if let Some(f) = fragment {
            o.field("Fragment", percent_decode(f, false));
        }
    }
    o.block("Percent-decoded", percent_decode(t, false));
    o.block("Percent-encoded", percent_encode(t));
    Ok(o)
}

// ---- UUID ----

fn uuid() -> Output {
    use std::hash::{BuildHasher, Hasher};
    let mut bytes = [0u8; 16];
    for half in 0..2 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(half);
        if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            h.write_u128(d.as_nanos());
        }
        bytes[half as usize * 8..half as usize * 8 + 8].copy_from_slice(&h.finish().to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 10
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let dashed = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    let mut o = Output::default();
    o.field("UUID", dashed.clone());
    o.field("Uppercase", dashed.to_uppercase());
    o.field("No dashes", hex);
    o.field("In braces", format!("{{{dashed}}}"));
    o.note("Random, but not meant for secrets.");
    o
}

// ---- JWT ----

/// `1970-01-01 00:00:00 UTC` for a time in seconds since 1970 (the civil-from-days algorithm).
pub fn format_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn relative(delta: i64) -> String {
    let (n, past) = (delta.abs(), delta < 0);
    let text = match n {
        0..=59 => format!("{n} s"),
        60..=3599 => format!("{} min", n / 60),
        3600..=86_399 => format!("{} h", n / 3600),
        _ => format!("{} days", n / 86_400),
    };
    if past {
        format!("{text} ago")
    } else {
        format!("in {text}")
    }
}

fn claim_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn jwt_part(part: &str, what: &str) -> Result<(String, serde_json::Value), String> {
    let bytes = b64_decode(part).ok_or_else(|| format!("the {what} is not valid Base64"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("the {what} is not text"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| format!("the {what} is not JSON"))?;
    Ok((reformat(&text, Some(2)), value))
}

fn jwt(input: &str, now: i64) -> Result<Output, String> {
    let t = input.trim();
    let t = t
        .strip_prefix("Bearer ")
        .or_else(|| t.strip_prefix("bearer "))
        .unwrap_or(t)
        .trim();
    if t.is_empty() {
        return Err("paste a token".into());
    }
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err("a token has two or three parts separated by dots".into());
    }
    let (header_text, header) = jwt_part(parts[0], "header")?;
    let (payload_text, payload) = jwt_part(parts[1], "payload")?;
    let mut o = Output::default();
    o.block("Header", header_text);
    o.block("Payload", payload_text);
    if let Some(alg) = header.get("alg").and_then(|v| v.as_str()) {
        o.field("Algorithm", alg);
        if alg.eq_ignore_ascii_case("none") {
            o.note("The algorithm is \"none\": this token is not signed at all.");
        }
    }
    if parts.len() == 2 || parts[2].is_empty() {
        o.note("There is no signature part.");
    }
    if let Some(map) = payload.as_object() {
        // the registered claims first, in a fixed order, then the rest
        let standard = ["iss", "sub", "aud", "exp", "nbf", "iat", "jti"];
        let mut keys: Vec<&String> = standard
            .iter()
            .filter_map(|k| map.get_key_value(*k).map(|(k, _)| k))
            .collect();
        keys.extend(map.keys().filter(|k| !standard.contains(&k.as_str())));
        for k in keys {
            let v = &map[k];
            o.field(k.clone(), claim_text(v));
            if matches!(k.as_str(), "exp" | "nbf" | "iat") {
                if let Some(secs) = v.as_i64() {
                    let when = format_utc(secs);
                    let rel = relative(secs - now);
                    o.field(format!("{k} as a date"), format!("{when} ({rel})"));
                    if k == "exp" && secs < now {
                        o.note("This token has expired.");
                    }
                    if k == "nbf" && secs > now {
                        o.note("This token is not valid yet.");
                    }
                }
            }
        }
    }
    o.note("The signature is not verified: this only reads the token.");
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    // header {"alg":"HS256","typ":"JWT"}, payload {"sub":"alice","name":"Alice","iat":1700000000,"exp":1700003600}
    const TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhbGljZSIsIm5hbWUiOiJBbGljZSIsImlhdCI6MTcwMDAwMDAwMCwiZXhwIjoxNzAwMDAzNjAwfQ.c2lnbmF0dXJl";

    fn value(o: &Output, label: &str) -> String {
        o.fields
            .iter()
            .find(|f| f.label == label)
            .unwrap_or_else(|| panic!("no field {label}: {:?}", o.fields))
            .value
            .clone()
    }

    #[test]
    fn base64_round_trips_and_handles_both_alphabets() {
        for text in [
            "",
            "a",
            "ab",
            "abc",
            "abcd",
            "héllo wörld ✓",
            "\u{0}\u{1}\u{2}",
        ] {
            for (alpha, pad) in [(STD, true), (URL, false), (URL, true)] {
                let enc = b64_encode(text.as_bytes(), alpha, pad);
                assert_eq!(
                    b64_decode(&enc).unwrap(),
                    text.as_bytes(),
                    "{text:?} via {enc}"
                );
            }
        }
        assert_eq!(b64_encode(b"Man", STD, true), "TWFu");
        assert_eq!(b64_encode(b"Ma", STD, true), "TWE=");
        assert_eq!(b64_encode(b"M", STD, true), "TQ==");
        assert_eq!(b64_decode("a"), None);
        assert_eq!(b64_decode("a$b"), None);
    }

    #[test]
    fn the_base64_tool_decodes_or_says_it_cannot() {
        let o = run("base64", "SGVsbG8gd29ybGQ=", 0).unwrap();
        assert_eq!(value(&o, "Decoded (text)"), "Hello world");
        let o = run("base64", "not base64 !!", 0).unwrap();
        assert!(o.notes[0].contains("not valid Base64"));
        assert!(run("base64", "  ", 0).is_err());
        let o = run("base64", "/w==", 0).unwrap();
        assert!(o.fields[0].label.contains("1 bytes"), "{:?}", o.fields[0]);
    }

    #[test]
    fn json_keeps_order_and_values_and_reports_errors_without_the_input() {
        let o = run(
            "json",
            r#"{"b":1,"a":[1,2,{"x":"q,w:e"}],"e":{},"f":[],"s":"\" , { \\"}"#,
            0,
        )
        .unwrap();
        let pretty = value(&o, "Formatted");
        assert!(
            pretty.starts_with("{\n  \"b\": 1,\n  \"a\": [\n    1,\n    2,"),
            "{pretty}"
        );
        assert!(pretty.contains("\"e\": {},") && pretty.contains("\"f\": [],"));
        assert!(
            pretty.contains(r#""s": "\" , { \\""#),
            "strings are untouched: {pretty}"
        );
        assert_eq!(
            value(&o, "Minified"),
            r#"{"b":1,"a":[1,2,{"x":"q,w:e"}],"e":{},"f":[],"s":"\" , { \\"}"#
        );
        // formatting twice changes nothing
        assert_eq!(reformat(&pretty, Some(2)), pretty);
        let e = run("json", "{\"secret\": \"hunter2\",}", 0).unwrap_err();
        assert!(e.contains("line 1"), "{e}");
        assert!(!e.contains("hunter2"), "an error never repeats the input");
        assert!(run("json", "", 0).is_err());
    }

    #[test]
    fn the_jwt_decoder_reads_claims_and_times_and_never_claims_to_verify() {
        let o = run("jwt", &format!("Bearer {TOKEN}"), 1_700_000_100).unwrap();
        assert_eq!(value(&o, "Algorithm"), "HS256");
        assert_eq!(value(&o, "sub"), "alice");
        assert_eq!(
            value(&o, "iat as a date"),
            "2023-11-14 22:13:20 UTC (1 min ago)"
        );
        assert_eq!(
            value(&o, "exp as a date"),
            "2023-11-14 23:13:20 UTC (in 58 min)"
        );
        assert!(value(&o, "Payload").contains("\"name\": \"Alice\""));
        assert!(o.notes.iter().any(|n| n.contains("not verified")));
        assert!(!o.notes.iter().any(|n| n.contains("expired")));
        // standard claims come first, in a fixed order
        let order: Vec<&str> = o.fields.iter().map(|f| f.label.as_str()).collect();
        assert!(order.iter().position(|l| *l == "sub") < order.iter().position(|l| *l == "name"));
        // later, it has expired
        let late = run("jwt", TOKEN, 1_800_000_000).unwrap();
        assert!(late.notes.iter().any(|n| n.contains("expired")));
        assert!(value(&late, "exp as a date").contains("days ago"));
    }

    #[test]
    fn bad_tokens_are_explained_without_echoing_them() {
        for bad in [
            "",
            "abc",
            "a.b.c.d",
            "!!!.eyJ9.sig",
            "eyJhbGciOiJub25lIn0.not-json-at-all.",
            "eyJhbGciOiJIUzI1NiJ9.WzEsMl0.sig",
        ] {
            let r = run("jwt", bad, 0);
            if let Err(e) = r {
                assert!(!e.contains("not-json-at-all") && !e.contains("!!!"), "{e}");
            }
        }
        assert!(run("jwt", "a.b.c.d", 0).is_err());
        assert!(run("jwt", "!!!.eyJ9.sig", 0).is_err());
        // an unsigned token is called out
        let none = run("jwt", "eyJhbGciOiJub25lIn0.eyJzdWIiOiJ4In0.", 0).unwrap();
        assert!(none.notes.iter().any(|n| n.contains("not signed")));
        // a payload that is JSON but not an object is shown, with no claims
        let arr = run("jwt", "eyJhbGciOiJIUzI1NiJ9.WzEsMl0.sig", 0).unwrap();
        assert!(arr.fields.iter().any(|f| f.label == "Payload"));
    }

    #[test]
    fn dates_are_right_at_the_edges() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(format_utc(4_102_444_799), "2099-12-31 23:59:59 UTC");
        assert_eq!(format_utc(-1), "1969-12-31 23:59:59 UTC");
    }

    #[test]
    fn urls_are_taken_apart_and_text_is_percent_coded() {
        let o = run(
            "url",
            "https://user:pw@example.com:8443/a/b?x=1&y=hello%20world&z#frag%21",
            0,
        )
        .unwrap();
        assert_eq!(value(&o, "Scheme"), "https");
        assert_eq!(value(&o, "Host"), "example.com");
        assert_eq!(value(&o, "Port"), "8443");
        assert_eq!(value(&o, "Path"), "/a/b");
        assert_eq!(value(&o, "? y"), "hello world");
        assert_eq!(value(&o, "? z"), "");
        assert_eq!(value(&o, "Fragment"), "frag!");
        assert!(o.notes[0].contains("user name"));
        assert!(
            !o.fields
                .iter()
                .any(|f| f.value.contains("pw@") && !f.label.starts_with("Percent")),
            "the password is not shown as a part"
        );
        let t = run("url", "a b/ü?", 0).unwrap();
        assert_eq!(value(&t, "Percent-encoded"), "a%20b%2F%C3%BC%3F");
        assert_eq!(percent_decode("a%20b%2f%zz%", false), "a b/%zz%");
        assert_eq!(percent_decode("a+b", true), "a b");
    }

    #[test]
    fn uuids_are_version_4_and_differ() {
        let a = run("uuid", "", 0).unwrap();
        let b = run("uuid", "", 0).unwrap();
        let u = value(&a, "UUID");
        assert_eq!(u.len(), 36);
        assert_eq!(u.as_bytes()[14], b'4', "version 4: {u}");
        assert!(
            matches!(u.as_bytes()[19], b'8' | b'9' | b'a' | b'b'),
            "variant: {u}"
        );
        assert_ne!(u, value(&b, "UUID"));
        assert_eq!(value(&a, "No dashes").len(), 32);
    }

    #[test]
    fn suggestions_guess_from_the_text() {
        assert_eq!(suggest(TOKEN), vec!["jwt"]);
        assert_eq!(suggest(&format!("Bearer {TOKEN}")), vec!["jwt"]);
        assert_eq!(suggest("{\"a\": 1}"), vec!["json"]);
        assert_eq!(suggest("https://example.com/x?y=1"), vec!["url"]);
        assert!(suggest("just some words. and more. words").is_empty());
        assert!(suggest("").is_empty());
    }

    #[test]
    fn unknown_tools_and_huge_input_are_refused() {
        assert!(run("nope", "x", 0).is_err());
        assert!(run("json", &"[".repeat(MAX_INPUT_BYTES + 1), 0)
            .unwrap_err()
            .contains("too much"));
        assert!(info("jwt").unwrap().sensitive);
        assert!(!info("uuid").unwrap().takes_input);
    }

    /// S5, structurally: this module is a set of pure functions. It must not even name the things a tool must never reach.
    #[test]
    fn s5_the_tools_module_imports_nothing_it_could_attack_with() {
        let source = include_str!("tools.rs");
        let code: String = source.split("#[cfg(test)]").next().unwrap().to_string();
        for forbidden in [
            "rusqlite",
            "std::fs",
            "std::net",
            "std::process",
            "std::env",
            "crate::backend",
            "crate::data",
            "crate::settings",
            "crate::workspace",
            "crate::tab",
            "crate::history",
            "crate::bookmark",
            "crate::entry",
            "Connection",
        ] {
            assert!(
                !code.contains(forbidden),
                "tools.rs must not use {forbidden}"
            );
        }
    }
}
