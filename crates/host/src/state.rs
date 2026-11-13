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
    /// Where saved downloads go; `None` is the user's Downloads folder. (`--downloads-dir=PATH`, used by the tests.)
    pub downloads_dir: Option<std::path::PathBuf>,
}

/// What the shell is told about a tab (the shown one): see `navigation`.
#[derive(Clone, Debug, Default)]
pub struct NavInfo {
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub can_back: bool,
    pub can_forward: bool,
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
    /// The launcher overlay: its view and the controller that places it above the pages.
    pub pending_launcher: Option<BrowserView>,
    /// The right-hand widget panel: its view, the overlay that places it, and whether it is open and how wide (set by core).
    pub pending_side: Option<BrowserView>,
    pub side_view: Option<BrowserView>,
    pub side_ctl: Option<OverlayController>,
    pub side_open: bool,
    /// The panel's overlay has been shown (it stays shown; only its size changes).
    pub side_shown: bool,
    /// The sidebar is collapsed to a rail (set by core).
    pub shell_collapsed: bool,
    /// A panel that core asked to open while the launcher was open: shown when the launcher closes.
    pub side_deferred: Option<(bool, i32)>,
    pub side_width: i32,
    pub launcher: Option<BrowserView>,
    pub launcher_ctl: Option<OverlayController>,
    /// The launcher is showing (set by `launcher::show` and `hide`).
    pub launcher_open: bool,
    /// A request context per workspace that has been shown in this run, and which of them are initialised (views may be created).
    pub contexts: HashMap<Id, RequestContext>,
    pub ready: HashSet<Id>,
    /// Tabs with a view.
    pub tabs: HashMap<Id, TabView>,
    /// Navigation state per tab that has a view.
    pub nav: HashMap<Id, NavInfo>,
    /// The favicon address last fetched for each tab, so the same icon is not downloaded again and again.
    pub favicon_urls: HashMap<Id, String>,
    /// Each workspace's accent colour (0xRRGGBB), drawn as a frame around its pages. Set from core's `Show` and `SetAccent` effects.
    pub accents: HashMap<Id, u32>,
    /// Tabs core asked to show whose workspace profile is still starting: (workspace, tab, url).
    pub waiting: Vec<(Id, Id, String)>,
    /// Tabs showing one of the app's own error pages, with the address that failed (Reload retries it).
    pub errors: HashMap<Id, String>,
    /// Downloads waiting for the user: our id, the tab, the safe file name, and CEF's callback. Dropping the callback cancels the download.
    pub downloads: HashMap<u32, (Id, String, BeforeDownloadCallback)>,
    pub next_download: u32,
    /// How many times in a row the shell's renderer has died and been reloaded.
    pub shell_crashes: u32,
    /// A selection request in flight: its one-time id and the tab it was sent to (see `selection`).
    pub selection_pending: Option<(u32, Id)>,
    pub selection_seq: u32,
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
