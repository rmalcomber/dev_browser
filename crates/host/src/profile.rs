//! A workspace profile is a CEF request context with its own cache folder.
//!
//! Two rules learned in M0:
//! - the cache folder must be a **direct child** of `root_cache_path`, in the same separator form (backslashes on Windows), or CEF
//!   quietly falls back to an in-memory profile;
//! - the context is created asynchronously, and **no view may be created until `on_request_context_initialized` fires** (otherwise
//!   `browser_view_create` returns nothing).

use crate::{state, state::Config, window};
use cef::*;
use std::path::Path;

/// `Err` explains why the folder is not an acceptable profile folder.
pub fn check_profile_dir(root: &Path, dir: &Path) -> Result<(), String> {
    if dir.parent() != Some(root) {
        return Err(format!(
            "profile folder {} must be a direct child of {}",
            dir.display(),
            root.display()
        ));
    }
    if dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| dev_browser_core::id::Id::parse(n).is_ok())
        != Some(true)
    {
        return Err(format!(
            "profile folder name in {} is not a valid workspace id",
            dir.display()
        ));
    }
    Ok(())
}

pub fn create(config: &Config) {
    if let Err(why) = check_profile_dir(&config.data.chromium_root(), &config.profile_dir) {
        eprintln!("[host] refusing to start: {why}");
        quit_message_loop();
        return;
    }
    let settings = RequestContextSettings {
        cache_path: CefString::from(config.profile_dir.to_string_lossy().as_ref()),
        persist_session_cookies: 1,
        ..Default::default()
    };
    let mut handler = ProfileReady::new();
    let ctx = request_context_create_context(Some(&settings), Some(&mut handler));
    state::with(|s| s.context = ctx);
}

wrap_request_context_handler! {
    struct ProfileReady {}
    impl RequestContextHandler {
        fn on_request_context_initialized(&self, _c: Option<&mut RequestContext>) {
            println!("[host] profile ready");
            window::open();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dev_browser_core::id::Id;

    #[test]
    fn only_a_direct_child_with_an_id_name_is_accepted() {
        let root = Path::new("C:\\data\\chromium-root");
        let id = Id::new("ws");
        assert!(check_profile_dir(root, &root.join(id.as_str())).is_ok());
        assert!(
            check_profile_dir(root, &root.join("a").join(id.as_str())).is_err(),
            "deeper"
        );
        assert!(
            check_profile_dir(root, Path::new("C:\\data").join(id.as_str()).as_path()).is_err(),
            "elsewhere"
        );
        assert!(
            check_profile_dir(root, &root.join("Default")).is_err(),
            "not an id"
        );
        assert!(check_profile_dir(root, root).is_err(), "the root itself");
    }
}
