//! Per-workspace bookmarks, in the workspace's own database. The launcher reads them from here; there is no second copy anywhere (the
//! spec: "no shadow bookmark copies"). One bookmark per address; `Ctrl+D` toggles it.

use crate::url;
use rusqlite::{params, Connection, OptionalExtension};

pub const MAX_ROWS: i64 = 5000;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub created_at: i64,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// May this address be bookmarked? Ordinary web pages only (the same rule as a tab's restorable address, and not the blank page).
pub fn bookmarkable(raw: &str) -> bool {
    url::is_restorable(raw) && !raw.eq_ignore_ascii_case("about:blank")
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

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Bookmark> {
    Ok(Bookmark {
        id: r.get(0)?,
        url: r.get(1)?,
        title: r.get(2)?,
        created_at: r.get(3)?,
    })
}

pub fn find(conn: &Connection, raw_url: &str) -> rusqlite::Result<Option<Bookmark>> {
    conn.query_row(
        "SELECT id, url, title, created_at FROM bookmarks WHERE url = ?1",
        [raw_url],
        row,
    )
    .optional()
}

/// Bookmark the address if it is not bookmarked, otherwise remove the bookmark. Returns whether it is bookmarked now.
/// An address that cannot be bookmarked, or a full list, is an error with a message for the user.
pub fn toggle(conn: &Connection, raw_url: &str, title: &str) -> Result<bool, String> {
    if !bookmarkable(raw_url) {
        return Err("this page cannot be bookmarked".into());
    }
    let sql = |e: rusqlite::Error| e.to_string();
    if let Some(b) = find(conn, raw_url).map_err(sql)? {
        conn.execute("DELETE FROM bookmarks WHERE id = ?1", [b.id])
            .map_err(sql)?;
        return Ok(false);
    }
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM bookmarks", [], |r| r.get(0))
        .map_err(sql)?;
    if count >= MAX_ROWS {
        return Err("there are too many bookmarks in this workspace".into());
    }
    conn.execute(
        "INSERT INTO bookmarks (url, title, created_at) VALUES (?1, ?2, ?3)",
        params![raw_url, clean(title), now()],
    )
    .map_err(sql)?;
    Ok(true)
}

/// All bookmarks, newest first.
pub fn all(conn: &Connection) -> rusqlite::Result<Vec<Bookmark>> {
    conn.prepare(
        "SELECT id, url, title, created_at FROM bookmarks ORDER BY created_at DESC, id DESC",
    )?
    .query_map([], row)?
    .collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Bookmark>> {
    conn.query_row(
        "SELECT id, url, title, created_at FROM bookmarks WHERE id = ?1",
        [id],
        row,
    )
    .optional()
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
    fn toggling_adds_then_removes() {
        let c = db();
        assert!(toggle(&c, "https://a.example/", "A\n").unwrap());
        assert_eq!(all(&c).unwrap().len(), 1);
        assert_eq!(all(&c).unwrap()[0].title, "A");
        assert!(!toggle(&c, "https://a.example/", "A").unwrap());
        assert!(all(&c).unwrap().is_empty());
    }

    #[test]
    fn only_web_pages_can_be_bookmarked() {
        let c = db();
        for no in [
            "about:blank",
            "javascript:alert(1)",
            "data:text/html,x",
            "file:///c:/x",
            "app://shell/",
            "",
            "https://a b/",
        ] {
            assert!(toggle(&c, no, "x").is_err(), "{no:?}");
        }
        assert!(all(&c).unwrap().is_empty());
    }

    #[test]
    fn the_list_is_bounded() {
        let c = db();
        for i in 0..MAX_ROWS {
            c.execute(
                "INSERT INTO bookmarks (url, title, created_at) VALUES (?1, '', 0)",
                [format!("https://x.example/{i}")],
            )
            .unwrap();
        }
        assert!(toggle(&c, "https://one-more.example/", "").is_err());
    }
}
