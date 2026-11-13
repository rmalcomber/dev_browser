//! Workspace pages: one request context and one page view per workspace, created the first time the workspace is shown, with exactly
//! one view visible at a time. The workspace list and what is active are decided in core; this module only carries out the effects
//! (`Activate`, `Delete`). M1.3 replaces "one page per workspace" with real tabs.
//!
//! Switching hides the old view **before** showing the new one, so a frame of the old workspace is never shown beside the new one
//! (the S8 rule; checked later with `tools\record.ps1`).

use crate::{
    handlers::{HostClient, Role},
    layout, profile, state,
    state::Page,
};
use cef::*;
use dev_browser_core::id::Id;

/// Show `id`'s page, creating its profile and view first if this is the first time.
pub fn show(id: &Id) {
    state::with(|s| s.active = Some(id.clone()));
    println!("[host] workspace {id}: shown");
    ensure(id);
    apply_visibility();
}

fn ensure(id: &Id) {
    if state::with(|s| s.pages.contains_key(id)) {
        return;
    }
    match profile::create_context(id) {
        Ok(context) => {
            println!("[host] workspace {id}: profile starting");
            state::with(|s| {
                s.pages.insert(
                    id.clone(),
                    Page {
                        context,
                        view: None,
                    },
                );
            });
        }
        Err(why) => eprintln!("[host] workspace {id}: cannot start its profile: {why}"),
    }
}

/// The profile for `id` is initialised: create its view (the gate described in `profile`) and put it in the window.
pub fn context_ready(id: &Id) {
    let (context, panel) =
        state::with(|s| (s.pages.get(id).map(|p| p.context.clone()), s.panel.clone()));
    // the workspace may have been deleted, or the window closed, while the profile was starting
    let (Some(mut context), Some(panel)) = (context, panel) else {
        return;
    };
    let url = state::config().start_url;
    let mut client = HostClient::new(Role::Page);
    let mut style = layout::ViewStyle::new(true);
    let Some(view) = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url.as_str())),
        Some(&layout::browser_settings()),
        None,
        Some(&mut context),
        Some(&mut style),
    ) else {
        eprintln!("[host] workspace {id}: could not create its view");
        return;
    };
    // Hidden until apply_visibility says it is the active one, so a background workspace never flashes on top.
    view.set_visible(0);
    let mut v = View::from(&view);
    panel.add_child_view(Some(&mut v));
    state::with(|s| {
        if let Some(p) = s.pages.get_mut(id) {
            p.view = Some(view);
        }
    });
    println!("[host] workspace {id}: page view created");
    apply_visibility();
}

/// Show the active workspace's view and hide every other one: hide first, then show.
fn apply_visibility() {
    let (views, active) = state::with(|s| {
        (
            s.pages
                .iter()
                .filter_map(|(id, p)| p.view.clone().map(|v| (id.clone(), v)))
                .collect::<Vec<_>>(),
            s.active.clone(),
        )
    });
    for (id, v) in &views {
        if Some(id) != active.as_ref() {
            v.set_visible(0);
        }
    }
    for (id, v) in &views {
        if Some(id) == active.as_ref() {
            v.set_visible(1);
            v.request_focus();
        }
    }
    layout::changed();
}

/// Close a deleted workspace's view and profile, then remove its folders (see `RemoveData`).
pub fn delete(id: &Id) {
    let page = state::with(|s| s.pages.remove(id));
    println!("[host] workspace {id}: deleting");
    if let Some(page) = page {
        if let Some(view) = page.view {
            // detach before closing the browser: closing a view that is still in the layout took the whole window with it in M0
            if let Some(panel) = state::with(|s| s.panel.clone()) {
                let mut v = View::from(&view);
                panel.remove_child_view(Some(&mut v));
            }
            if let Some(host) = view.browser().and_then(|b| b.host()) {
                host.close_browser(1);
            }
        }
        // dropping `page.context` lets CEF release the profile once its browser is gone
    }
    layout::changed();
    let mut task = RemoveData::new(id.clone(), 0);
    post_delayed_task(ThreadId::UI, Some(&mut task), 500);
}

/// Chromium keeps files in the profile open until its browser and context are gone, which takes a moment after the close. So try again
/// every half second for a while; if it still fails, the next start removes the folders as orphans (`Backend::open`).
const REMOVE_ATTEMPTS: u32 = 40;

wrap_task! {
    struct RemoveData { id: Id, attempt: u32 }
    impl Task {
        fn execute(&self) {
            let data = state::config().data;
            match data.remove_workspace_folders(&self.id) {
                Ok(()) => println!("[host] workspace {}: data removed", self.id),
                Err(_) if self.attempt < REMOVE_ATTEMPTS => {
                    let mut again = RemoveData::new(self.id.clone(), self.attempt + 1);
                    post_delayed_task(ThreadId::UI, Some(&mut again), 500);
                }
                Err(e) => eprintln!("[host] workspace {}: could not remove its data ({e}); it will be removed at the next start", self.id),
            }
        }
    }
}
