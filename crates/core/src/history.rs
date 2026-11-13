//! Per-workspace browsing history: one row per page address (a visit count and the time of the last visit), kept in the workspace's own
//! database so it can never be read through another workspace's scope and disappears with the workspace.
//!
//! What is recorded: ordinary web pages only (`http` and `https`), **without the fragment** (`#...`), because a fragment can carry a
//! token (an OAuth redirect, a one-time link) that has no business in a history list. The query string is kept; it is what tells two
//! pages of one site apart. History is capped (`MAX_ROWS`, oldest dropped first) and can be cleared.

use crate::url;
use rusqlite::{params, Connection};

pub const MAX_ROWS: i64 = 5000;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visit {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visits: i64,
    pub last_visit: i64,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The address as it is recorded, or `None` if this page is not history (not a web page, not a restorable address). Without fragment.
pub fn page_url(raw: &str) -> Option<String> {
    if !url::is_restorable(raw) {
        return None;
    }
    let lower = raw.get(..8).unwrap_or(raw).to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return None; // about:blank is restorable but is not a visit
    }
    Some(raw.split('#').next().unwrap_or(raw).to_string())
}

fn clean(title: &str) -> String {
    title
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect()
}

/// A page was visited. A repeat visit counts and moves it to the top; a title is only ever replaced by a non-empty one.
pub fn record(conn: &Connection, raw_url: &str, title: &str) -> rusqlite::Result<bool> {
    let Some(page) = page_url(raw_url) else {
        return Ok(false);
    };
    conn.execute(
        "INSERT INTO history (url, title, visits, last_visit) VALUES (?1, ?2, 1, ?3)
         ON CONFLICT(url) DO UPDATE SET visits = visits + 1, last_visit = excluded.last_visit,
             title = CASE WHEN excluded.title <> '' THEN excluded.title ELSE title END",
        params![page, clean(title), now()],
    )?;
    conn.execute(
        "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY last_visit DESC, id DESC LIMIT ?1)",
        [MAX_ROWS],
    )?;
    Ok(true)
}

/// The page's title arrived (after its address): remember it for that page, if the page is in history.
pub fn set_title(conn: &Connection, raw_url: &str, title: &str) -> rusqlite::Result<()> {
    if let Some(page) = page_url(raw_url) {
        conn.execute(
            "UPDATE history SET title = ?2 WHERE url = ?1 AND ?2 <> ''",
            params![page, clean(title)],
        )?;
    }
    Ok(())
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Visit> {
    Ok(Visit {
        id: r.get(0)?,
        url: r.get(1)?,
        title: r.get(2)?,
        visits: r.get(3)?,
        last_visit: r.get(4)?,
    })
}

/// Everything, most recent first (at most `MAX_ROWS`).
pub fn all(conn: &Connection) -> rusqlite::Result<Vec<Visit>> {
    conn.prepare(
        "SELECT id, url, title, visits, last_visit FROM history ORDER BY last_visit DESC, id DESC",
    )?
    .query_map([], row)?
    .collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Visit>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT id, url, title, visits, last_visit FROM history WHERE id = ?1",
        [id],
        row,
    )
    .optional()
}

/// Forget everything. Returns how many pages were removed.
pub fn clear(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM history", [])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate::migrate, schema};

    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, schema::WORKSPACE).unwrap();
        c
    }

    #[test]
    fn only_web_pages_without_a_fragment_are_history() {
        assert_eq!(
            page_url("https://a.example/x?q=1#token=secret").as_deref(),
            Some("https://a.example/x?q=1")
        );
        assert_eq!(
            page_url("HTTP://a.example/").as_deref(),
            Some("HTTP://a.example/")
        );
        for no in [
            "about:blank",
            "javascript:alert(1)",
            "data:text/html,x",
            "file:///c:/x",
            "app://shell/",
            "",
            "http://",
            "https://a b/",
        ] {
            assert_eq!(page_url(no), None, "{no:?}");
        }
    }

    #[test]
    fn a_visit_is_recorded_counted_and_titled() {
        let c = db();
        assert!(record(&c, "https://a.example/", "").unwrap());
        record(&c, "https://a.example/#frag", "A page").unwrap();
        record(&c, "https://a.example/", "").unwrap();
        let v = all(&c).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(
            (v[0].visits, v[0].title.as_str()),
            (3, "A page"),
            "an empty title never erases a known one"
        );
        assert!(!record(&c, "about:blank", "x").unwrap());
        assert_eq!(all(&c).unwrap().len(), 1);
    }

    #[test]
    fn a_late_title_goes_to_its_page_only() {
        let c = db();
        record(&c, "https://a.example/", "").unwrap();
        record(&c, "https://b.example/", "B").unwrap();
        set_title(&c, "https://a.example/#x", "Late title\n").unwrap();
        set_title(&c, "https://never-visited.example/", "Ghost").unwrap();
        let v = all(&c).unwrap();
        assert_eq!(v.len(), 2, "a title for an unvisited page makes no row");
        assert_eq!(
            v.iter()
                .find(|x| x.url == "https://a.example/")
                .unwrap()
                .title,
            "Late title"
        );
    }

    #[test]
    fn history_is_capped_and_can_be_cleared() {
        let c = db();
        for i in 0..(MAX_ROWS + 25) {
            c.execute(
                "INSERT INTO history (url, title, visits, last_visit) VALUES (?1, '', 1, ?2)",
                params![format!("https://x.example/{i}"), i],
            )
            .unwrap();
        }
        record(&c, "https://new.example/", "").unwrap();
        let v = all(&c).unwrap();
        assert_eq!(v.len() as i64, MAX_ROWS);
        assert_eq!(v[0].url, "https://new.example/");
        assert!(
            !v.iter().any(|x| x.url == "https://x.example/0"),
            "the oldest went first"
        );
        assert_eq!(clear(&c).unwrap() as i64, MAX_ROWS);
        assert!(all(&c).unwrap().is_empty());
    }
}
