//! The host's mutable state. CEF calls every handler on the UI thread (the main thread), so one thread-local is enough and no locking is
//! needed. Everything that touches it goes through `with`, and a handler never holds the borrow while calling back into CEF.

use cef::*;
use dev_browser_core::data::DataDir;
use std::cell::RefCell;
use std::path::PathBuf;

/// What the binary tells the host before it starts.
#[derive(Clone, Debug)]
pub struct Config {
    pub data: DataDir,
    /// The workspace profile folder: must be a direct child of `data.chromium_root()` (checked in `profile`).
    pub profile_dir: PathBuf,
    /// The first page to open.
    pub start_url: String,
}

#[derive(Default)]
pub struct State {
    pub config: Option<Config>,
    pub browsers: Vec<Browser>,
    pub window: Option<Window>,
    pub panel: Option<Panel>,
    pub shell: Option<BrowserView>,
    pub page: Option<BrowserView>,
    pub context: Option<RequestContext>,
    /// Last layout key, so a layout change we cause ourselves does not loop.
    pub last_layout: (i32, i32),
    pub page_width: i32,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

pub fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn config() -> Config {
    with(|s| s.config.clone()).expect("host config is set before CEF starts")
}
