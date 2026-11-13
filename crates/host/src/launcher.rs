//! The launcher overlay: a second trusted view (`app://shell/launcher.html`) above the pages, shown by `Ctrl+P`. A page never sees it
//! and it never shares a profile with a page. What it shows and searches is decided in core (`backend::launcher`); this module only
//! shows, hides, places and moves it.

use crate::{
    handlers::{HostClient, Role},
    layout, pages, state,
};
use cef::*;

pub const URL: &str = "app://shell/launcher.html";
pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 440;
const TOP: i32 = 24;

/// The launcher's browser view, made before the window exists (the window delegate adds it as an overlay).
pub fn create_view() -> Option<BrowserView> {
    let mut client = HostClient::new(Role::Launcher, String::new(), String::new());
    let mut style = layout::ViewStyle::new(false);
    // no request context: like the shell it shares no cookies or storage with any workspace
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(URL)),
        Some(&layout::browser_settings()),
        None,
        None,
        Some(&mut style),
    )
}

/// Add the view to the window as a hidden overlay.
pub fn attach(window: &Window, view: BrowserView) {
    let mut v = View::from(&view);
    let ctl = window.add_overlay_view(Some(&mut v), DockingMode::CUSTOM, 1);
    if let Some(c) = ctl.as_ref() {
        c.set_bounds(Some(&Rect {
            x: 0,
            y: TOP,
            width: WIDTH,
            height: HEIGHT,
        }));
        c.set_visible(0);
    }
    view.set_visible(0);
    state::with(|s| {
        s.launcher = Some(view);
        s.launcher_ctl = ctl;
    });
}

/// Where the top-left corner may be: the whole launcher stays inside the window.
pub fn clamp_pos(x: i32, y: i32, window: (i32, i32)) -> (i32, i32) {
    (
        x.clamp(0, (window.0 - WIDTH).max(0)),
        y.clamp(0, (window.1 - HEIGHT).max(0)),
    )
}

/// The top-centre default.
pub fn default_pos(window_width: i32) -> (i32, i32) {
    (((window_width - WIDTH) / 2).max(0), TOP)
}

fn window_size() -> Option<(i32, i32)> {
    state::with(|s| s.window.clone()).map(|w| {
        let z = w.size();
        (z.width, z.height)
    })
}

fn place(x: i32, y: i32) {
    let (Some(ctl), Some(win)) = (state::with(|s| s.launcher_ctl.clone()), window_size()) else {
        return;
    };
    let (x, y) = clamp_pos(x, y, win);
    ctl.set_bounds(Some(&Rect {
        x,
        y,
        width: WIDTH.min(win.0),
        height: HEIGHT.min(win.1),
    }));
}

/// Show the launcher where it was last left (or top-centre), give it the keyboard focus and tell its page to start afresh.
pub fn show() {
    let (ctl, view, win) = (
        state::with(|s| s.launcher_ctl.clone()),
        state::with(|s| s.launcher.clone()),
        window_size(),
    );
    let (Some(ctl), Some(view), Some(win)) = (ctl, view, win) else {
        return;
    };
    let saved = state::with(|s| s.backend.as_ref().and_then(|b| b.launcher_pos()));
    let (x, y) = saved.unwrap_or_else(|| default_pos(win.0));
    place(x, y);
    ctl.set_visible(1);
    view.set_visible(1);
    view.request_focus();
    println!("[host] launcher shown");
    crate::ipc::push_to_launcher("launcher.opened", "{}");
}

/// Hide it, remember where it is, and give the page the focus back.
pub fn hide() {
    let (ctl, view) = (
        state::with(|s| s.launcher_ctl.clone()),
        state::with(|s| s.launcher.clone()),
    );
    if let Some(c) = ctl.as_ref() {
        let p = c.position();
        println!("[host] launcher at ({}, {})", p.x, p.y);
        state::with(|s| {
            if let Some(b) = s.backend.as_ref() {
                b.set_launcher_pos(p.x, p.y)
            }
        });
        c.set_visible(0);
    }
    if let Some(v) = view {
        v.set_visible(0);
    }
    println!("[host] launcher hidden");
    pages::focus_page();
}

pub fn move_by(dx: i32, dy: i32) {
    let Some(ctl) = state::with(|s| s.launcher_ctl.clone()) else {
        return;
    };
    let p = ctl.position();
    place(p.x + dx, p.y + dy);
}

/// The window changed size: keep the launcher inside it.
pub fn keep_inside() {
    let Some(ctl) = state::with(|s| s.launcher_ctl.clone()) else {
        return;
    };
    let p = ctl.position();
    place(p.x, p.y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_never_leaves_the_window() {
        assert_eq!(clamp_pos(-50, -50, (1200, 720)), (0, 0));
        assert_eq!(
            clamp_pos(5000, 5000, (1200, 720)),
            (1200 - WIDTH, 720 - HEIGHT)
        );
        assert_eq!(
            clamp_pos(100, 100, (300, 200)),
            (0, 0),
            "a window smaller than the launcher"
        );
        assert_eq!(clamp_pos(100, 50, (1200, 720)), (100, 50));
    }

    #[test]
    fn it_starts_top_centre() {
        assert_eq!(default_pos(1200), ((1200 - WIDTH) / 2, TOP));
        assert_eq!(default_pos(100), (0, TOP));
    }
}
