//! The window's content: a Panel with a horizontal BoxLayout holding the trusted shell (fixed width) and the page (the rest).
//!
//! Findings from M0 that shape this file:
//! - hand-set `set_bounds` painted wrongly; a BoxLayout on a Panel keeps the native browser windows in step with the views;
//! - flex weights were ignored for browser views, so each view reports its wanted width as its preferred size and the layout is
//!   **re-installed** when the panel size changes;
//! - views are added to the panel in layout order.

use crate::state;
use cef::*;

pub const SHELL_WIDTH: i32 = 220;

/// A dark browser background (matches the app) turns the first unpainted frames of a new view dark instead of white.
pub fn browser_settings() -> BrowserSettings {
    BrowserSettings {
        background_color: 0xFF1E1E1E,
        ..Default::default()
    }
}

// Alloy-style views inside the Chrome bootstrap (M0 decision; CEF issue 3681).
wrap_browser_view_delegate! {
    pub struct ViewStyle { page: bool }
    impl ViewDelegate {
        fn preferred_size(&self, _v: Option<&mut View>) -> Size {
            let w = if self.page { state::with(|s| s.page_width).max(1) } else { SHELL_WIDTH };
            Size { width: w, height: 100 }
        }
    }
    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
    }
}

wrap_panel_delegate! {
    pub struct PanelEvents {}
    impl ViewDelegate {
        fn on_layout_changed(&self, _v: Option<&mut View>, _b: Option<&Rect>) { relayout(); }
    }
    impl PanelDelegate {}
}

/// The page width for a given panel width.
pub fn page_width(panel_width: i32) -> i32 {
    (panel_width - SHELL_WIDTH).max(1)
}

pub fn relayout() {
    let (panel, shell, page, last) = state::with(|s| {
        (
            s.panel.clone(),
            s.shell.clone(),
            s.page.clone(),
            s.last_layout,
        )
    });
    let (Some(panel), Some(shell), Some(page)) = (panel, shell, page) else {
        return;
    };
    let size = panel.size();
    if size.width <= 0 || size.height <= 0 || (size.width, size.height) == last {
        return;
    }
    state::with(|s| {
        s.last_layout = (size.width, size.height);
        s.page_width = page_width(size.width);
    });
    let Some(layout) = panel.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    })) else {
        return;
    };
    let (mut vs, mut vp) = (View::from(&shell), View::from(&page));
    layout.set_flex_for_view(Some(&mut vs), 0);
    layout.set_flex_for_view(Some(&mut vp), 0);
    shell.invalidate_layout();
    page.invalidate_layout();
    panel.invalidate_layout();
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_page_takes_what_the_shell_leaves() {
        assert_eq!(super::page_width(1200), 1200 - super::SHELL_WIDTH);
        assert_eq!(super::page_width(100), 1, "never zero or negative");
    }
}
