//! Navigation of the shown tab: carrying out core's navigation effects (load an address, back, forward, reload, stop), and keeping the shell
//! told about the shown tab (its address, title, loading state and whether it can go back or forward) over IPC.
//!
//! What is loaded is decided in core (`address::resolve` turns what was typed into an address). The shell never names a tab: it can only
//! navigate the shown one. Favicons are fetched here, **in the tab's own browser** (so in its workspace's request context, with its cookies
//! and not another workspace's), made into a PNG `data:` address, and handed to core, which decides who may receive it.

use crate::{b64, ipc, state, state::NavInfo};
use cef::*;
use dev_browser_core::{id::Id, ipc::Reply, nav, url};

fn reply_for(tab: &Id, n: &NavInfo) -> Reply {
    nav::reply(tab, &n.url, &n.title, n.loading, n.can_back, n.can_forward)
}

/// Change what is known about a tab, and tell the shell if it is the shown one.
pub fn update(tab: &Id, f: impl FnOnce(&mut NavInfo)) {
    let reply = state::with(|s| {
        let n = s.nav.entry(tab.clone()).or_default();
        f(n);
        (s.shown.as_ref() == Some(tab)).then(|| reply_for(tab, n))
    });
    if let Some(r) = reply {
        ipc::push_to_shell(r.name, &r.payload);
    }
}

/// A tab became the shown one: make sure something is known about it (a fresh view starts out loading its address) and tell the shell.
pub fn shown(tab: &Id, url: &str) {
    let reply = state::with(|s| {
        let n = s.nav.entry(tab.clone()).or_insert_with(|| NavInfo {
            url: url.to_string(),
            loading: true,
            ..Default::default()
        });
        reply_for(tab, n)
    });
    ipc::push_to_shell(reply.name, &reply.payload);
}

/// A tab's view is gone: forget what was known about it.
pub fn forget(tab: &Id) {
    state::with(|s| {
        s.nav.remove(tab);
        s.favicon_urls.remove(tab);
    });
}

/// The browser of the shown tab, if it has a view.
fn shown_browser() -> Option<Browser> {
    state::with(|s| {
        s.shown
            .as_ref()
            .and_then(|t| s.tabs.get(t))
            .and_then(|tv| tv.view.browser())
    })
}

/// Load an address in a tab. Core made the address; checked once more here, because this is the call that actually loads something.
pub fn navigate(tab: &Id, address: &str) {
    if !url::is_restorable(address) {
        eprintln!("[host] refused to load an address core should not have sent");
        return;
    }
    let frame = state::with(|s| s.tabs.get(tab).and_then(|tv| tv.view.browser()))
        .and_then(|b| b.main_frame());
    match frame {
        Some(f) => {
            println!("[host] tab {tab}: navigating");
            f.load_url(Some(&CefString::from(address)));
        }
        // the view is still being made: it will open this address when it is created
        None => state::with(|s| {
            for w in s.waiting.iter_mut().filter(|(_, t, _)| t == tab) {
                w.2 = address.to_string();
            }
        }),
    }
}

pub fn back() {
    if let Some(b) = shown_browser() {
        b.go_back();
    }
}

pub fn forward() {
    if let Some(b) = shown_browser() {
        b.go_forward();
    }
}

pub fn reload(hard: bool) {
    if let Some(b) = shown_browser() {
        if hard {
            b.reload_ignore_cache();
        } else {
            b.reload();
        }
    }
}

pub fn stop() {
    if let Some(b) = shown_browser() {
        b.stop_load();
    }
}

// ---- favicons ----

/// The largest favicon image asked for, in pixels. The sidebar shows 16, so 32 covers a high-DPI screen.
const FAVICON_PIXELS: u32 = 32;

/// The strings of a CEF string list that CEF lent us for a callback. (Cloning such a list gives an empty one, so read it through its pointer.)
pub fn string_list(list: &mut CefStringList) -> Vec<String> {
    let raw: *mut sys::_cef_string_list_t = list.into();
    // SAFETY: CEF passes a valid list to the callback and it stays valid for its duration; each value is copied out and then cleared.
    unsafe {
        let Some(raw) = raw.as_mut() else {
            return Vec::new();
        };
        let count = sys::cef_string_list_size(raw);
        (0..count)
            .filter_map(|i| {
                let mut value: sys::_cef_string_utf16_t = std::mem::zeroed();
                let got = sys::cef_string_list_value(raw, i, &mut value) > 0;
                let text = got.then(|| CefString::from(std::ptr::from_ref(&value)).to_string());
                sys::cef_string_utf16_clear(&mut value);
                text
            })
            .collect()
    }
}

/// A page announced its icon addresses. Fetch the first web one (once per address) in this tab's own browser.
pub fn favicon_urls_changed(workspace: &str, tab: &str, browser: &Browser, urls: Vec<String>) {
    let (Ok(ws), Ok(tab_id)) = (Id::parse(workspace), Id::parse(tab)) else {
        return;
    };
    let Some(address) = urls
        .into_iter()
        .find(|u| u.starts_with("https://") || u.starts_with("http://"))
    else {
        return;
    };
    let fresh = state::with(|s| {
        s.favicon_urls
            .insert(tab_id.clone(), address.clone())
            .as_deref()
            != Some(address.as_str())
    });
    if !fresh {
        return;
    }
    let Some(host) = browser.host() else { return };
    let mut done = FaviconDone::new(ws, tab_id);
    // downloaded through this browser, hence through its workspace's request context; not cached beyond the session
    host.download_image(
        Some(&CefString::from(address.as_str())),
        1,
        FAVICON_PIXELS,
        0,
        Some(&mut done),
    );
}

wrap_download_image_callback! {
    struct FaviconDone { workspace: Id, tab: Id }
    impl DownloadImageCallback {
        fn on_download_image_finished(&self, _url: Option<&CefString>, status: i32, image: Option<&mut Image>) {
            if status != 200 { return }
            let Some(image) = image else { return };
            // CEF needs the size out-parameters: with None it returns nothing at all
            let (mut w, mut h) = (0, 0);
            let Some(png) = image.as_png(1.0, 1, Some(&mut w), Some(&mut h)) else { return };
            let size = png.size();
            if size == 0 || size > 48 * 1024 { return }
            // the bytes are CEF's own PNG encoding of whatever the page's icon decoded to, not the page's file
            let bytes = unsafe { std::slice::from_raw_parts(png.raw_data() as *const u8, size) };
            let data = format!("data:image/png;base64,{}", b64::encode(bytes));
            println!("[host] tab {}: favicon ready ({size} bytes)", self.tab);
            let reply = state::with(|s| s.backend.as_mut().and_then(|b| b.tab_favicon(&self.workspace, &self.tab, &data)));
            if let Some(r) = reply { ipc::push_to_shell(r.name, &r.payload); }
        }
    }
}
