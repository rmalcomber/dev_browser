//! The host's mutable state. CEF calls every handler on the UI thread (the main thread), so one thread-local is enough and no locking is
//! needed. Everything that touches it goes through `with`, and a handler never holds the borrow while calling back into CEF.

use cef::*;
use dev_browser_core::{backend::Backend, data::DataDir, id::Id};
use std::cell::RefCell;
use std::collections::HashMap;

/// What the binary tells the host before it starts.
#[derive(Clone, Debug)]
pub struct Config {
    pub data: DataDir,
    /// The first page every workspace opens (M1.3 replaces this with each workspace's restored tab).
    pub start_url: String,
}

/// One workspace's browsing: its profile (request context) and, once the context is ready, its page view.
pub struct Page {
    pub context: RequestContext,
    pub view: Option<BrowserView>,
}

#[derive(Default)]
pub struct State {
    pub config: Option<Config>,
    /// Decides what the shell may do; owns the global database. Opened before CEF starts, in the browser process only.
    pub backend: Option<Backend>,
    pub browsers: Vec<Browser>,
    pub window: Option<Window>,
    pub panel: Option<Panel>,
    pub shell: Option<BrowserView>,
    /// One entry per workspace that has been shown at least once in this run.
    pub pages: HashMap<Id, Page>,
    pub active: Option<Id>,
    /// Last layout key (width, height, generation), so a layout change we cause ourselves does not loop.
    pub last_layout: (i32, i32, u32),
    /// Bumped whenever the set of visible views changes, to force the box layout to be re-installed.
    pub layout_gen: u32,
    pub page_width: i32,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

pub fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn config() -> Config {
    with(|s| s.config.clone()).expect("host config is set before CEF starts")
}
