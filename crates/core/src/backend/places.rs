//! `Ctrl+D` and clearing history: the two things the user does to a workspace's places. Both act on the **active** workspace only; the
//! launcher is how places are found again (`launcher.rs`).

use super::*;

impl Backend {
    /// Bookmark the shown page of the active workspace, or remove its bookmark. The answer is a short notice.
    pub(super) fn bookmark_toggle(&mut self) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let tab = self.current(&ws)?;
        let added = self.with_db(&ws, |c| {
            bookmark::toggle(c, &tab.url, &tab.title).map_err(tab::TabError::Message)
        })?;
        let name = if tab.title.trim().is_empty() {
            tab.url.as_str()
        } else {
            tab.title.as_str()
        };
        let text = if added {
            format!("Bookmarked: {name}")
        } else {
            format!("Bookmark removed: {name}")
        };
        Ok(Outcome {
            replies: vec![Self::notice_reply(&text)],
            effects: vec![],
        })
    }

    /// Save the shown page of the active workspace as an entry (a named link, optionally for an environment), or update it.
    pub(super) fn entry_save(&mut self, env: Option<entry::Env>) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let tab = self.current(&ws)?;
        let saved = self.with_home(&ws, |c| {
            entry::save(c, &tab.url, &tab.title, env).map_err(tab::TabError::Message)
        })?;
        let tag = saved
            .env
            .map(|e| format!(" ({})", e.as_str()))
            .unwrap_or_default();
        Ok(Outcome {
            replies: vec![Self::notice_reply(&format!(
                "Saved as an entry{tag}: {}",
                saved.title
            ))],
            effects: vec![],
        })
    }

    /// Remove the entry for the shown page of the active workspace.
    pub(super) fn entry_remove(&mut self) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let tab = self.current(&ws)?;
        let removed = self.with_home(&ws, |c| Ok(entry::remove(c, &tab.url)?))?;
        let text = if removed {
            "Entry removed"
        } else {
            "This page is not an entry"
        };
        Ok(Outcome {
            replies: vec![Self::notice_reply(text)],
            effects: vec![],
        })
    }

    /// Forget the active workspace's history. Bookmarks and open tabs stay.
    pub(super) fn history_clear(&mut self) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let n = self.with_db(&ws, |c| Ok(history::clear(c)?))?;
        // what was opened how often is browsing history too
        self.with_home(&ws, |c| Ok(entry::clear_usage(c)?))?;
        Ok(Outcome {
            replies: vec![Self::notice_reply(&format!(
                "History cleared ({n} page{})",
                if n == 1 { "" } else { "s" }
            ))],
            effects: vec![],
        })
    }
}
