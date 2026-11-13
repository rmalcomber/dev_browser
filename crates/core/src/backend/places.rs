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

    /// Forget the active workspace's history. Bookmarks and open tabs stay.
    pub(super) fn history_clear(&mut self) -> Result<Outcome, Failure> {
        let ws = self.active.clone();
        let n = self.with_db(&ws, |c| Ok(history::clear(c)?))?;
        Ok(Outcome {
            replies: vec![Self::notice_reply(&format!(
                "History cleared ({n} page{})",
                if n == 1 { "" } else { "s" }
            ))],
            effects: vec![],
        })
    }
}
