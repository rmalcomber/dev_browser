//! The trusted shell is served from `app://shell/`, a custom scheme the app owns. Files come from the `ui` folder next to the executable
//! (a copy of `ui/dist`, made by `tools/build.ps1`); embedding them in the binary can replace that without changing anything here.
//!
//! Safety rules, all in `resolve`: the URL path can only name a file inside that folder (no `..`, no backslashes, no drive letters, no
//! encoded tricks), and anything else is a 404. The scheme is registered `DISPLAY_ISOLATED`, so only app:// content can embed or link
//! to it, and the shell is created without a workspace request context, so it never shares cookies or storage with a page.

use cef::wrapper::stream_resource_handler::StreamResourceHandler;
use cef::*;
use std::path::{Component, Path, PathBuf};

pub const SCHEME: &str = "app";
pub const HOST: &str = "shell";
pub const ORIGIN: &str = "app://shell/";
pub const START_URL: &str = "app://shell/index.html";

/// Register the scheme. Runs in every process (CEF calls it from `execute_process` and `initialize`).
pub fn register_scheme(registrar: &mut SchemeRegistrar) {
    let options = SchemeOptions::STANDARD.get_raw()
        | SchemeOptions::SECURE.get_raw()
        | SchemeOptions::CORS_ENABLED.get_raw()
        | SchemeOptions::FETCH_ENABLED.get_raw()
        | SchemeOptions::DISPLAY_ISOLATED.get_raw();
    registrar.add_custom_scheme(Some(&CefString::from(SCHEME)), options);
}

/// Where the shell files live: `<folder of the executable>/ui`.
pub fn dist_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("ui")))
        .unwrap_or_else(|| PathBuf::from("ui"))
}

/// Is this URL a page of the trusted shell? The browser process checks this before it believes a message.
pub fn is_shell_url(url: &str) -> bool {
    url.starts_with(ORIGIN)
}

/// Turn the path part of an `app://shell/...` URL into a file under `root`, or `None` if it is not an acceptable name.
pub fn resolve(root: &Path, url_path: &str) -> Option<PathBuf> {
    let path = url_path.split(['?', '#']).next().unwrap_or("");
    let path = path.strip_prefix('/').unwrap_or(path);
    let path = if path.is_empty() { "index.html" } else { path };
    // every segment must be a real name: no empty ones (`//`, trailing `/`), no `.` and no `..`
    if path
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return None;
    }
    // Refuse anything that is not plain forward-slash relative names. Percent-encoding is not decoded, so `%2e%2e` is just a name
    // that does not exist, and a backslash or colon (drive letters, streams) is never part of a file name here.
    if path.contains(['\\', ':', '\0']) {
        return None;
    }
    let rel = Path::new(path);
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(root.join(rel))
}

pub fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") => "text/html",
        Some("js") | Some("mjs") => "text/javascript",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

fn not_found() -> ResourceHandler {
    StreamResourceHandler::new(
        404,
        "Not Found".to_string(),
        "text/plain".to_string(),
        None,
        None,
    )
}

fn serve(url: &str) -> ResourceHandler {
    // url is app://shell/<path>
    let path = url
        .strip_prefix(ORIGIN)
        .map(|p| format!("/{p}"))
        .unwrap_or_default();
    let root = dist_dir();
    let Some(file) = resolve(&root, &path) else {
        eprintln!("[host] app://shell refused a bad path: {path:?}");
        return not_found();
    };
    match std::fs::read(&file) {
        Ok(mut bytes) => match stream_reader_create_for_data(bytes.as_mut_ptr(), bytes.len()) {
            // CEF copies the bytes into the reader, so `bytes` can go away.
            Some(stream) => {
                StreamResourceHandler::new_with_stream(mime_type(&file).to_string(), stream)
            }
            None => not_found(),
        },
        Err(_) => not_found(),
    }
}

wrap_scheme_handler_factory! {
    struct ShellFiles {}
    impl SchemeHandlerFactory {
        fn create(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame>, _scheme: Option<&CefString>, request: Option<&mut Request>) -> Option<ResourceHandler> {
            let url = request.map(|r| CefString::from(&r.url()).to_string())?;
            Some(serve(&url))
        }
    }
}

/// Register the file factory for `app://shell`. Browser process, after CEF is initialised.
pub fn register_factory() {
    let mut factory = ShellFiles::new();
    let ok = register_scheme_handler_factory(
        Some(&CefString::from(SCHEME)),
        Some(&CefString::from(HOST)),
        Some(&mut factory),
    );
    if ok != 1 {
        eprintln!("[host] could not register the app://shell handler");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("C:\\app\\ui")
    }

    #[test]
    fn plain_names_resolve_inside_the_root() {
        assert_eq!(
            resolve(&root(), "/index.html"),
            Some(root().join("index.html"))
        );
        assert_eq!(
            resolve(&root(), "/assets/index-abc.js"),
            Some(root().join("assets").join("index-abc.js"))
        );
        assert_eq!(
            resolve(&root(), "/"),
            Some(root().join("index.html")),
            "the bare origin means index.html"
        );
        assert_eq!(
            resolve(&root(), "/a.js?x=1#y"),
            Some(root().join("a.js")),
            "query and fragment are not part of the name"
        );
    }

    #[test]
    fn path_tricks_are_refused() {
        for bad in [
            "/../secret",
            "/a/../../secret",
            "/..",
            "/a/./b",
            "//host/share",
            "/C:/Windows/win.ini",
            "/a\\b",
            "/a:stream",
            "/a\0b",
            "/..\\x",
        ] {
            assert_eq!(resolve(&root(), bad), None, "{bad:?} must be refused");
        }
    }

    #[test]
    fn encoded_dots_are_just_a_name_that_does_not_exist() {
        let p = resolve(&root(), "/%2e%2e/secret").expect("a plain (odd) name");
        assert!(p.starts_with(root()), "{p:?}");
    }

    #[test]
    fn only_the_shell_origin_counts_as_the_shell() {
        assert!(is_shell_url("app://shell/index.html"));
        assert!(!is_shell_url("app://shell.evil.com/index.html"));
        assert!(!is_shell_url("https://app://shell/"));
        assert!(!is_shell_url("https://example.com/app://shell/"));
        assert!(!is_shell_url("app://other/index.html"));
        assert!(!is_shell_url(""));
    }

    #[test]
    fn mime_types() {
        assert_eq!(mime_type(Path::new("a/b.JS")), "text/javascript");
        assert_eq!(mime_type(Path::new("x.html")), "text/html");
        assert_eq!(
            mime_type(Path::new("x.unknown")),
            "application/octet-stream"
        );
    }
}
