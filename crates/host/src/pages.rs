//! Tab pages: carrying out core's effects. Core decides which tab is shown and which views live (planning/m1-plan.md, section 4); this
//! module creates a tab's view when asked to show it (after its workspace's profile is ready), hides every other view, destroys views
//! on request, and moves focus. A tab is a record in core; its view here is cheap to destroy and recreate from the record's address.
//!
//! Switching hides the old view **before** showing the new one, so a frame of the old tab is never shown beside the new one (the S8
//! rule; checked with `tools\record.ps1`).

use crate::{
    handlers::{HostClient, Role},
    ipc, layout, profile, state,
    state::TabView,
};
use cef::*;
use dev_browser_core::backend::{Effect, Outcome};
use dev_browser_core::id::Id;

/// Carry out one effect from core.
pub fn execute(effect: Effect) {
    match effect {
        Effect::Show {
            workspace,
            tab,
            url,
        } => show(&workspace, &tab, &url),
        Effect::DestroyView { tab, .. } => destroy_view(&tab),
        Effect::DeleteWorkspace(ws) => delete_workspace(&ws),
        Effect::FocusPage => focus_page(),
        Effect::FocusShell => focus_shell(),
    }
}

/// Carry out an outcome: effects first (in order), then push its replies to the shell.
pub fn deliver(outcome: Outcome) {
    for effect in outcome.effects {
        execute(effect);
    }
    for reply in outcome.replies {
        ipc::push_to_shell(reply.name, &reply.payload);
    }
}

/// Show a tab, creating its view first if it has none (and its workspace's profile before that, if this is the first tab shown from it).
fn show(ws: &Id, tab: &Id, url: &str) {
    println!("[host] tab {tab}: shown (workspace {ws})");
    let (has_view, ready) = state::with(|s| {
        s.shown = Some(tab.clone());
        (s.tabs.contains_key(tab), s.ready.contains(ws))
    });
    if has_view {
        apply_visibility();
        return;
    }
    state::with(|s| {
        s.waiting.retain(|(_, t, _)| t != tab);
        s.waiting.push((ws.clone(), tab.clone(), url.to_string()));
    });
    if ready {
        flush(ws);
    } else {
        ensure_context(ws);
    }
}

fn ensure_context(ws: &Id) {
    if state::with(|s| s.contexts.contains_key(ws)) {
        return; // already starting; context_ready will flush
    }
    match profile::create_context(ws) {
        Ok(context) => {
            println!("[host] workspace {ws}: profile starting");
            state::with(|s| {
                s.contexts.insert(ws.clone(), context);
            });
        }
        Err(why) => eprintln!("[host] workspace {ws}: cannot start its profile: {why}"),
    }
}

/// The profile for `ws` is initialised: views for its waiting tabs may now be created (the gate described in `profile`).
pub fn context_ready(ws: &Id) {
    state::with(|s| {
        if s.contexts.contains_key(ws) {
            s.ready.insert(ws.clone());
        }
    });
    flush(ws);
}

/// Create the views waiting on `ws`'s profile. Only the tab that is meant to be visible gets one; a tab that was shown and left again
/// before its profile was ready is simply dropped (it gets its view when it is shown again).
fn flush(ws: &Id) {
    let (mine, shown) = state::with(|s| {
        let all = std::mem::take(&mut s.waiting);
        let (mine, others): (Vec<_>, Vec<_>) = all.into_iter().partition(|(w, _, _)| w == ws);
        s.waiting = others;
        (mine, s.shown.clone())
    });
    for (w, tab, url) in mine {
        if Some(&tab) == shown.as_ref() {
            create_view(&w, &tab, &url);
        }
    }
}

fn create_view(ws: &Id, tab: &Id, url: &str) {
    let (context, panel) = state::with(|s| (s.contexts.get(ws).cloned(), s.panel.clone()));
    // the workspace may have been deleted, or the window closed, while the profile was starting
    let (Some(mut context), Some(panel)) = (context, panel) else {
        return;
    };
    let mut client = HostClient::new(Role::Page, ws.to_string(), tab.to_string());
    let mut style = layout::ViewStyle::new(true);
    let Some(view) = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&layout::browser_settings()),
        None,
        Some(&mut context),
        Some(&mut style),
    ) else {
        eprintln!("[host] tab {tab}: could not create its view");
        return;
    };
    // The view lives in a wrapper panel, which is what the window shows, hides and detaches. The wrapper goes into the window first so the
    // browser view's native window is created inside it. Hidden until apply_visibility says it is the shown one.
    let mut wd = layout::WrapEvents::new();
    let Some(wrapper) = panel_create(Some(&mut wd)) else {
        eprintln!("[host] tab {tab}: could not create its wrapper");
        return;
    };
    wrapper.set_visible(0);
    let mut wv = View::from(&wrapper);
    panel.add_child_view(Some(&mut wv));
    let inner = wrapper.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        inside_border_insets: Insets {
            top: layout::FRAME_PX,
            left: layout::FRAME_PX,
            bottom: layout::FRAME_PX,
            right: layout::FRAME_PX,
        },
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    let mut v = View::from(&view);
    wrapper.add_child_view(Some(&mut v));
    if let Some(l) = inner.as_ref() {
        l.set_flex_for_view(Some(&mut v), 0);
    }
    state::with(|s| {
        s.tabs.insert(
            tab.clone(),
            TabView {
                workspace: ws.clone(),
                wrapper,
                view,
            },
        );
    });
    println!("[host] tab {tab}: view created");
    apply_visibility();
}

/// Show the shown tab's view and hide every other one: hide first, then show. Honour a pending focus request.
fn apply_visibility() {
    let (views, shown, want_focus) = state::with(|s| {
        (
            s.tabs
                .iter()
                .map(|(t, tv)| (t.clone(), tv.wrapper.clone(), tv.view.clone()))
                .collect::<Vec<_>>(),
            s.shown.clone(),
            s.want_focus,
        )
    });
    for (t, w, _) in &views {
        if Some(t) != shown.as_ref() {
            w.set_visible(0);
        }
    }
    for (t, w, v) in &views {
        if Some(t) == shown.as_ref() {
            w.set_visible(1);
            if want_focus {
                v.request_focus();
                state::with(|s| s.want_focus = false);
            }
        }
    }
    layout::changed();
}

fn focus_page() {
    state::with(|s| s.want_focus = true);
    apply_visibility(); // focuses now if the view exists, otherwise when it is created
                        // the shell abandons any form it had open (Esc and F6 leave it half-typed otherwise)
    ipc::push_to_shell("focus", r#"{"target":"page"}"#);
}

fn focus_shell() {
    if let Some(shell) = state::with(|s| s.shell.clone()) {
        shell.request_focus();
        // the shell's page decides which control inside it takes the focus
        ipc::push_to_shell("focus", r#"{"target":"sidebar"}"#);
    }
}

/// Close a tab's view (a discard or a close ordered by core). The record is core's business.
fn destroy_view(tab: &Id) {
    let removed = state::with(|s| {
        s.waiting.retain(|(_, t, _)| t != tab);
        let tv = s.tabs.remove(tab);
        if tv.is_some() {
            s.closing.insert(tab.clone());
        }
        tv
    });
    println!("[host] tab {tab}: view destroyed");
    if let Some(tv) = removed {
        close_view(&tv.wrapper, &tv.view);
    }
    layout::changed();
}

/// Detach a tab's wrapper from the window, then close its browser. Detach first: closing a view that is still in the window took the whole
/// window with it in M0. Detach the *wrapper*: the browser view stays inside it, as it did in M0, which is what lets the browser finish
/// closing (a browser view removed from every parent never did: its renderer process lived on).
fn close_view(wrapper: &Panel, view: &BrowserView) {
    if let Some(panel) = state::with(|s| s.panel.clone()) {
        let mut w = View::from(wrapper);
        panel.remove_child_view(Some(&mut w));
    }
    if let Some(host) = view.browser().and_then(|b| b.host()) {
        host.close_browser(1);
    }
}

/// Close all of a deleted workspace's views and its profile, then remove its folders (see `RemoveData`).
fn delete_workspace(ws: &Id) {
    println!("[host] workspace {ws}: deleting");
    let tabs: Vec<Id> = state::with(|s| {
        s.tabs
            .iter()
            .filter(|(_, tv)| &tv.workspace == ws)
            .map(|(t, _)| t.clone())
            .collect()
    });
    for t in &tabs {
        destroy_view(t);
    }
    state::with(|s| {
        s.waiting.retain(|(w, _, _)| w != ws);
        s.ready.remove(ws);
        // dropping the context lets CEF release the profile once its browsers are gone
        s.contexts.remove(ws);
    });
    let mut task = RemoveData::new(ws.clone(), 0);
    post_delayed_task(ThreadId::UI, Some(&mut task), 500);
}

/// A tab's browser has closed. If the app did not order it (the page closed itself, `window.close()`), core is told, because that is a
/// tab close and something else must be shown. Not while the window is closing: then every browser goes, and no tab is being closed.
pub fn browser_closed(ws: &str, tab: &str) {
    let (Ok(ws), Ok(tab)) = (Id::parse(ws), Id::parse(tab)) else {
        return;
    };
    let by_page = state::with(|s| {
        if s.closing.remove(&tab) || s.shutting_down {
            return false;
        }
        s.tabs.remove(&tab).is_some()
    });
    if !by_page {
        return;
    }
    println!("[host] tab {tab}: closed by its page");
    let outcome = state::with(|s| {
        s.backend
            .as_mut()
            .and_then(|b| b.view_closed_by_page(&ws, &tab))
    });
    layout::changed();
    if let Some(outcome) = outcome {
        deliver(outcome);
    }
}

/// Chromium keeps files in the profile open until its browser and context are gone, which takes a moment after the close. So try again
/// every half second for a while; if it still fails, the exit sweep and the next start remove the folders as orphans.
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
                Err(e) => eprintln!("[host] workspace {}: could not remove its data ({e}); it will be removed at exit or the next start", self.id),
            }
        }
    }
}
