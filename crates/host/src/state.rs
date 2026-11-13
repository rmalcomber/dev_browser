//! The host's mutable state. CEF calls every handler on the UI thread (the main thread), so one thread-local is enough and no locking is
//! needed. Everything that touches it goes through `with`, and a handler never holds the borrow while calling back into CEF.

use cef::*;
use dev_browser_core::{backend::Backend, data::DataDir, id::Id};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// What the binary tells the host before it starts.
#[derive(Clone, Debug)]
pub struct Config {
    pub data: DataDir,
    /// The page a new tab opens (until the launcher and address bar exist).
    pub start_url: String,
}

/// A tab that has a view right now. The tab's record lives in core; this is only the CEF side.
pub struct TabView {
    pub workspace: Id,
    /// The panel that holds the view. It is what is shown, hidden and detached: a browser view with no parent cannot finish closing
    /// (M0 always removed a wrapper, never the view itself), and M1.4 draws the workspace colour frame in it.
    pub wrapper: Panel,
    pub view: BrowserView,
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
    /// A request context per workspace that has been shown in this run, and which of them are initialised (views may be created).
    pub contexts: HashMap<Id, RequestContext>,
    pub ready: HashSet<Id>,
    /// Tabs with a view.
    pub tabs: HashMap<Id, TabView>,
    /// Tabs core asked to show whose workspace profile is still starting: (workspace, tab, url).
    pub waiting: Vec<(Id, Id, String)>,
    /// The tab that should be visible.
    pub shown: Option<Id>,
    /// Tabs whose view the app itself is closing, so the browser's close event is not mistaken for the page closing itself.
    pub closing: HashSet<Id>,
    /// Focus the page as soon as the shown tab's view exists.
    pub want_focus: bool,
    /// The window is closing: browsers going away now are not tabs being closed.
    pub shutting_down: bool,
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
