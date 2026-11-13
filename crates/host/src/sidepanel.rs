//! The right-hand widget panel: a trusted view (`app://shell/panel.html`) docked as an overlay in the strip at the window's right edge,
//! which the pages leave free when the panel is open (`layout::page_width`). What it shows (which tools are pinned) and what a widget
//! does is decided in core (`backend/widgets.rs`); this module shows, hides, places and focuses it.
//!
//! It is an overlay, not a third column of the box layout, because pages are added to that layout later than the shell and a column
//! added now would end up in the middle of them. The pages are narrowed by exactly its width instead, so nothing is ever covered.

use crate::{
    handlers::{HostClient, Role},
    ipc, layout, state,
};
use cef::*;

pub const URL: &str = "app://shell/panel.html";

/// The panel's browser view, made before the window exists (the window delegate adds it as an overlay).
pub fn create_view() -> Option<BrowserView> {
    let mut client = HostClient::new(Role::Panel, String::new(), String::new());
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
        c.set_visible(0);
    }
    view.set_visible(0);
    state::with(|s| {
        s.side_view = Some(view);
        s.side_ctl = ctl;
    });
}

/// Where the panel sits: the right strip of the window, full height, `width` wide (never wider than the window).
pub fn bounds(window: (i32, i32), width: i32) -> (i32, i32, i32, i32) {
    let w = width.clamp(0, window.0.max(0));
    (window.0 - w, 0, w, window.1.max(0))
}

/// Core says the panel is now open or closed, this wide; put it there, re-flow the pages, and maybe give it the keyboard focus.
pub fn apply(open: bool, width: i32, focus: bool) {
    // Showing the panel moves the keyboard into it. While the launcher is open (a tool was just pinned) that would pull the keyboard away from
    // the user, so the panel is shown the moment the launcher closes instead.
    if open && !focus && crate::state::with(|s| s.launcher_open && !s.side_open) {
        state::with(|s| s.side_deferred = Some((open, width)));
        return;
    }
    state::with(|s| s.side_deferred = None);
    let was = state::with(|s| (s.side_open, s.side_width));
    state::with(|s| {
        s.side_open = open;
        s.side_width = width;
    });
    if was != (open, width) {
        println!(
            "[host] panel {} (width {width})",
            if open { "open" } else { "closed" }
        );
    }
    place();
    layout::changed();
    let (view, ctl) = state::with(|s| (s.side_view.clone(), s.side_ctl.clone()));
    let (Some(view), Some(ctl)) = (view, ctl) else {
        return;
    };
    ctl.set_visible(i32::from(open));
    view.set_visible(i32::from(open));
    if open {
        // showing a view can move the keyboard into it: give it back to where it was (the launcher if that is open, else the page)
        // (a moment later: the window finishes activating the new view after this call returns)
        if !focus && !was.0 {
            let mut task = Refocus::new();
            post_delayed_task(ThreadId::UI, Some(&mut task), 150);
        }
    }
    if open && focus {
        view.request_focus();
        ipc::push_to_panel("panel.focus", "{}");
    } else if !open && was.0 {
        // closing hands the keyboard back to the page, and the panel forgets what was typed into it
        ipc::push_to_panel("panel.closed", "{}");
        crate::pages::focus_page();
    }
}

wrap_task! {
    struct Refocus {}
    impl Task {
        fn execute(&self) {
            if !crate::launcher::refocus() {
                crate::pages::focus_page();
            }
        }
    }
}

/// The launcher closed: show a panel that was waiting for it.
pub fn apply_deferred() {
    if let Some((open, width)) = state::with(|s| s.side_deferred.take()) {
        apply(open, width, false);
    }
}

/// Put the overlay in the strip the pages leave free. Called when the panel changes and when the window is resized.
pub fn place() {
    // the size of the window's content (what the pages are laid out in), not of the window with its frame: the frame is not drawn on
    let (ctl, open, width, window) = state::with(|s| {
        (
            s.side_ctl.clone(),
            s.side_open,
            s.side_width,
            s.panel.clone(),
        )
    });
    let (Some(ctl), Some(window)) = (ctl, window) else {
        return;
    };
    if !open {
        return;
    }
    let z = window.size();
    let (x, y, w, h) = bounds((z.width, z.height), width);
    ctl.set_bounds(Some(&Rect {
        x,
        y,
        width: w,
        height: h,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_is_the_right_strip_of_the_window() {
        assert_eq!(bounds((1200, 720), 320), (880, 0, 320, 720));
        assert_eq!(
            bounds((300, 400), 320),
            (0, 0, 300, 400),
            "never wider than the window"
        );
        assert_eq!(bounds((1200, 720), 0), (1200, 0, 0, 720));
    }
}
