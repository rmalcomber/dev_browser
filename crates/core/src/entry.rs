//! Home Screen entries and launch counts: the contents of a workspace's own `homescreen.db` (the spec keeps these apart from the browser's
//! own records: history and bookmarks are in `browser.db`).
//!
//! An **entry** is a saved, named link that may belong to an environment (`dev`, `stage`, `prod`), like the links of a Home Screen
//! developer tile ("Billing prod", always shown in red so the wrong one is not clicked by accident). **Launch counts** record how often and
//! when the user opened a launcher result *from this workspace*; they only nudge ranking inside the scope that is already allowed, and are
//! never listed anywhere (an empty search shows no "recent" items).

use crate::url;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

pub const MAX_ENTRIES: i64 = 2000;
const MAX_TITLE_CHARS: usize = 200;
/// Launch counts kept (least recently used dropped first).
pub const MAX_USAGE: i64 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    Dev,
    Stage,
    Prod,
}

impl Env {
    pub fn as_str(self) -> &'static str {
        match self {
            Env::Dev => "dev",
            Env::Stage => "stage",
            Env::Prod => "prod",
        }
    }

    pub fn parse(s: &str) -> Option<Env> {
        match s {
            "dev" => Some(Env::Dev),
            "stage" => Some(Env::Stage),
            "prod" => Some(Env::Prod),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub env: Option<Env>,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let env: Option<String> = r.get(3)?;
    Ok(Entry {
        id: r.get(0)?,
        url: r.get(1)?,
        title: r.get(2)?,
        env: env.as_deref().and_then(Env::parse),
    })
}

/// Save an address as an entry, or update its name and environment if it already is one. Web pages only.
pub fn save(
    conn: &Connection,
    raw_url: &str,
    title: &str,
    env: Option<Env>,
) -> Result<Entry, String> {
    if !url::is_restorable(raw_url) || raw_url.eq_ignore_ascii_case("about:blank") {
        return Err("this page cannot be saved as an entry".into());
    }
    let sql = |e: rusqlite::Error| e.to_string();
    let exists = find(conn, raw_url).map_err(sql)?.is_some();
    if !exists {
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
            .map_err(sql)?;
        if n >= MAX_ENTRIES {
            return Err("there are too many entries in this workspace".into());
        }
    }
    let name = clean(title);
    let name = if name.is_empty() {
        raw_url.to_string()
    } else {
        name
    };
    conn.execute(
        "INSERT INTO entries (url, title, env, created_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(url) DO UPDATE SET title = excluded.title, env = excluded.env",
        params![raw_url, name, env.map(Env::as_str), now()],
    )
    .map_err(sql)?;
    Ok(find(conn, raw_url).map_err(sql)?.expect("just saved"))
}

pub fn find(conn: &Connection, raw_url: &str) -> rusqlite::Result<Option<Entry>> {
    conn.query_row(
        "SELECT id, url, title, env FROM entries WHERE url = ?1",
        [raw_url],
        row,
    )
    .optional()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Entry>> {
    conn.query_row(
        "SELECT id, url, title, env FROM entries WHERE id = ?1",
        [id],
        row,
    )
    .optional()
}

pub fn all(conn: &Connection) -> rusqlite::Result<Vec<Entry>> {
    conn.prepare("SELECT id, url, title, env FROM entries ORDER BY id")?
        .query_map([], row)?
        .collect()
}

/// Remove the entry for an address. Returns whether there was one.
pub fn remove(conn: &Connection, raw_url: &str) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM entries WHERE url = ?1", [raw_url])? > 0)
}

// ---- launch counts ----

/// The user opened the result with this key (`entry:3`, `bm:7`, `cmd:nav.reload` ...).
pub fn record_use(conn: &Connection, key: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO launches (key, count, last_used) VALUES (?1, 1, ?2)
         ON CONFLICT(key) DO UPDATE SET count = count + 1, last_used = excluded.last_used",
        params![key, now()],
    )?;
    conn.execute(
        "DELETE FROM launches WHERE key NOT IN (SELECT key FROM launches ORDER BY last_used DESC, key LIMIT ?1)",
        [MAX_USAGE],
    )?;
    Ok(())
}

/// Every key with how many times it was opened and when, for ranking only.
pub fn usage(conn: &Connection) -> rusqlite::Result<HashMap<String, (i64, i64)>> {
    conn.prepare("SELECT key, count, last_used FROM launches")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))?
        .collect()
}

pub fn clear_usage(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM launches", [])
}

/// The current time, for ranking.
pub fn clock() -> i64 {
    now()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate::migrate, schema};

    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, schema::HOMESCREEN).unwrap();
        c
    }

    #[test]
    fn saving_twice_updates_the_same_entry() {
        let c = db();
        let a = save(&c, "https://billing.example/", "Billing\n", None).unwrap();
        assert_eq!((a.title.as_str(), a.env), ("Billing", None));
        let b = save(
            &c,
            "https://billing.example/",
            "Billing prod",
            Some(Env::Prod),
        )
        .unwrap();
        assert_eq!(b.id, a.id);
        assert_eq!(b.env, Some(Env::Prod));
        assert_eq!(all(&c).unwrap().len(), 1);
        assert!(remove(&c, "https://billing.example/").unwrap());
        assert!(!remove(&c, "https://billing.example/").unwrap());
    }

    #[test]
    fn an_empty_title_falls_back_to_the_address_and_odd_pages_are_refused() {
        let c = db();
        assert_eq!(
            save(&c, "https://a.example/", "  ", None).unwrap().title,
            "https://a.example/"
        );
        for no in [
            "about:blank",
            "javascript:alert(1)",
            "data:text/html,x",
            "file:///c:/x",
            "",
            "https://a b/",
        ] {
            assert!(save(&c, no, "x", None).is_err(), "{no:?}");
        }
    }

    #[test]
    fn entries_are_bounded() {
        let c = db();
        for i in 0..MAX_ENTRIES {
            c.execute(
                "INSERT INTO entries (url, title, created_at) VALUES (?1, '', 0)",
                [format!("https://x.example/{i}")],
            )
            .unwrap();
        }
        assert!(save(&c, "https://one-more.example/", "", None).is_err());
        assert!(
            save(&c, "https://x.example/0", "renamed", None).is_ok(),
            "updating an existing one is still fine"
        );
    }

    #[test]
    fn launch_counts_accumulate_are_bounded_and_clear() {
        let c = db();
        record_use(&c, "entry:1").unwrap();
        record_use(&c, "entry:1").unwrap();
        record_use(&c, "bm:2").unwrap();
        let u = usage(&c).unwrap();
        assert_eq!(u["entry:1"].0, 2);
        assert_eq!(u["bm:2"].0, 1);
        for i in 0..(MAX_USAGE + 20) {
            c.execute(
                "INSERT INTO launches (key, count, last_used) VALUES (?1, 1, ?2)",
                params![format!("k{i}"), i],
            )
            .unwrap();
        }
        record_use(&c, "fresh").unwrap();
        assert_eq!(usage(&c).unwrap().len() as i64, MAX_USAGE);
        assert!(usage(&c).unwrap().contains_key("fresh"));
        clear_usage(&c).unwrap();
        assert!(usage(&c).unwrap().is_empty());
    }

    #[test]
    fn environments_parse_only_known_names() {
        for e in [Env::Dev, Env::Stage, Env::Prod] {
            assert_eq!(Env::parse(e.as_str()), Some(e));
        }
        assert_eq!(Env::parse("production"), None);
    }
}
