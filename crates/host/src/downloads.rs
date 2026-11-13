//! Downloads, minimal (M1.6): a page's download is held until the user answers in the shell (Save or Cancel). The shell never names a
//! path: the folder is the app's (the user's Downloads folder, or `--downloads-dir` in tests) and the file name is made safe by core.
//! A cancelled download, or one whose tab closed, is cancelled by dropping CEF's callback without calling it.

use crate::{ipc, state};
use cef::*;
use dev_browser_core::{downloads, id::Id};
use std::path::PathBuf;

fn folder() -> PathBuf {
    state::config().downloads_dir.unwrap_or_else(|| {
        std::env::var_os("USERPROFILE")
            .map(|h| PathBuf::from(h).join("Downloads"))
            .unwrap_or_else(std::env::temp_dir)
    })
}

/// A page started a download. Returns whether the app took it (1) or it is cancelled right away (0).
pub fn offer(
    workspace: &str,
    tab: &str,
    suggested: &str,
    callback: &BeforeDownloadCallback,
) -> i32 {
    let (Ok(ws), Ok(tab)) = (Id::parse(workspace), Id::parse(tab)) else {
        return 0; // not a page of a tab (the shell): no downloads
    };
    let id = state::with(|s| {
        s.next_download += 1;
        s.next_download
    });
    let reply = state::with(|s| {
        s.backend
            .as_mut()
            .and_then(|b| b.download_offer(&ws, id, suggested))
    });
    let Some(reply) = reply else {
        println!("[host] download refused (not the active workspace, or too many waiting)");
        return 0;
    };
    let name = downloads::safe_name(suggested);
    println!("[host] download {id} offered: {name}");
    state::with(|s| s.downloads.insert(id, (tab, name, callback.clone())));
    ipc::push_to_shell(reply.name, &reply.payload);
    1
}

/// The user's answer, ordered by core.
pub fn decide(id: u32, save: bool) {
    let Some((_, name, callback)) = state::with(|s| s.downloads.remove(&id)) else {
        return;
    };
    ipc::push_to_shell("download.done", &format!(r#"{{"id":{id}}}"#));
    if !save {
        println!("[host] download {id} cancelled");
        return; // dropping the callback cancels it
    }
    let dir = folder();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[host] download {id}: cannot make the downloads folder: {e}");
        return;
    }
    let file = downloads::unique_name(&name, |n| dir.join(n).exists());
    let path = dir.join(&file);
    println!("[host] download {id} saving as {file}");
    callback.cont(Some(&CefString::from(path.to_string_lossy().as_ref())), 0);
}

/// A tab's view is closing: its waiting downloads are cancelled and withdrawn.
pub fn tab_closed(tab: &Id) {
    let ids: Vec<u32> = state::with(|s| {
        let ids: Vec<u32> = s
            .downloads
            .iter()
            .filter(|(_, (t, _, _))| t == tab)
            .map(|(i, _)| *i)
            .collect();
        for i in &ids {
            s.downloads.remove(i);
        }
        ids
    });
    for id in ids {
        state::with(|s| s.backend.as_mut().map(|b| b.download_gone(id)));
        ipc::push_to_shell("download.done", &format!(r#"{{"id":{id}}}"#));
    }
}
