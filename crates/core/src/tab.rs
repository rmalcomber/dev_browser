//! Tab records, in a workspace's own database. A tab is a record, not a view: its page view exists only while needed and can be
//! destroyed and recreated from the record (planning/m1-plan.md, section 4). Positions are dense (0, 1, 2, ...) within the workspace, and
//! they are the **visual order** the sidebar shows and `Ctrl+Tab` follows: ungrouped tabs first, then each group (in group order) with its
//! tabs together. `normalize` keeps that true after every change, so a tab can never sit inside another group's block.
//!
//! Titles and URLs come from pages, so they are untrusted: titles are cleaned and cut short here, and a URL is only ever stored if
//! `url::is_restorable` says it may be loaded again.

use crate::id::Id;
use crate::url;
use rusqlite::{params, Connection, OptionalExtension};
use std::fmt;

pub const MAX_TITLE_CHARS: usize = 200;
const SELECTED: &str = "last_tab_id";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    pub id: Id,
    pub url: String,
    pub title: String,
    pub position: u32,
    /// The group this tab is in, if any (see `group`).
    pub group: Option<Id>,
}

#[derive(Debug)]
pub enum TabError {
    NotFound(Id),
    /// The URL may not be stored (see `url::is_restorable`).
    BadUrl,
    BadPosition,
    /// The group does not exist in this workspace.
    NoSuchGroup(Id),
    /// A group name that is empty, too long or has control characters.
    InvalidName(&'static str),
    /// A message for the user from another module that runs inside a workspace database call (a bookmark that cannot be made).
    Message(String),
    Sqlite(rusqlite::Error),
}

impl fmt::Display for TabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TabError::NotFound(_) => write!(f, "there is no such tab"),
            TabError::BadUrl => write!(f, "that address cannot be opened in a tab"),
            TabError::BadPosition => write!(f, "that position is outside the tab list"),
            TabError::NoSuchGroup(_) => write!(f, "there is no such group"),
            TabError::InvalidName(why) => write!(f, "invalid group name: {why}"),
            TabError::Message(m) => write!(f, "{m}"),
            TabError::Sqlite(e) => write!(f, "sqlite error: {e}"),
        }
    }
}

impl std::error::Error for TabError {}

impl From<rusqlite::Error> for TabError {
    fn from(e: rusqlite::Error) -> Self {
        TabError::Sqlite(e)
    }
}

/// A page's title made safe to store and show: control characters become spaces, ends trimmed, at most 200 characters.
pub fn clean_title(raw: &str) -> String {
    let t: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    t.trim().chars().take(MAX_TITLE_CHARS).collect()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Tab> {
    let id: String = r.get(0)?;
    Ok(Tab {
        // a bad id in the database means the file was edited by hand: fail loudly rather than build anything from it
        id: Id::parse(&id).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        url: r.get(1)?,
        title: r.get(2)?,
        position: r.get(3)?,
        group: match r.get::<_, Option<String>>(4)? {
            None => None,
            Some(g) => Some(Id::parse(&g).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?),
        },
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Tab>, TabError> {
    let mut s =
        conn.prepare("SELECT id, url, title, position, group_id FROM tabs ORDER BY position")?;
    let tabs = s.query_map([], row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(tabs)
}

pub fn get(conn: &Connection, id: &Id) -> Result<Tab, TabError> {
    conn.query_row(
        "SELECT id, url, title, position, group_id FROM tabs WHERE id = ?1",
        [id.as_str()],
        row,
    )
    .optional()?
    .ok_or_else(|| TabError::NotFound(id.clone()))
}

/// Add a tab at `position` (the end if `None` or past the end), shifting the ones after it, in `group` if given. The position is a
/// request: `normalize` then puts the tab where its group says it belongs.
pub fn create(
    conn: &Connection,
    url: &str,
    position: Option<u32>,
    group: Option<&Id>,
) -> Result<Tab, TabError> {
    if !url::is_restorable(url) {
        return Err(TabError::BadUrl);
    }
    if let Some(g) = group {
        crate::group::get(conn, g)?;
    }
    let tx = conn.unchecked_transaction()?;
    let count: u32 = tx.query_row("SELECT COUNT(*) FROM tabs", [], |r| r.get(0))?;
    let position = position.unwrap_or(count).min(count);
    tx.execute(
        "UPDATE tabs SET position = position + 1 WHERE position >= ?1",
        [position],
    )?;
    let id = Id::new("tab");
    tx.execute(
        "INSERT INTO tabs (id, url, title, position, created_at, last_active, group_id) VALUES (?1, ?2, '', ?3, ?4, ?4, ?5)",
        params![id.as_str(), url, position, now(), group.map(|g| g.as_str())],
    )?;
    normalize(&tx)?;
    tx.commit()?;
    get(conn, &id)
}

/// Remove a tab and close the gap. If it was the selected tab the selection is cleared (the caller picks the next one).
pub fn close(conn: &Connection, id: &Id) -> Result<(), TabError> {
    let tx = conn.unchecked_transaction()?;
    let position: u32 = tx
        .query_row(
            "SELECT position FROM tabs WHERE id = ?1",
            [id.as_str()],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| TabError::NotFound(id.clone()))?;
    tx.execute("DELETE FROM tabs WHERE id = ?1", [id.as_str()])?;
    tx.execute(
        "UPDATE tabs SET position = position - 1 WHERE position > ?1",
        [position],
    )?;
    tx.execute(
        "DELETE FROM workspace_state WHERE key = ?1 AND value = ?2",
        params![SELECTED, id.as_str()],
    )?;
    crate::group::prune_empty(&tx)?;
    normalize(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Move a tab to `position` (0 to count - 1), shifting the others.
pub fn move_to(conn: &Connection, id: &Id, position: u32) -> Result<(), TabError> {
    let tx = conn.unchecked_transaction()?;
    let count: u32 = tx.query_row("SELECT COUNT(*) FROM tabs", [], |r| r.get(0))?;
    if position >= count {
        return Err(TabError::BadPosition);
    }
    let from: u32 = tx
        .query_row(
            "SELECT position FROM tabs WHERE id = ?1",
            [id.as_str()],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| TabError::NotFound(id.clone()))?;
    if from != position {
        if from < position {
            tx.execute(
                "UPDATE tabs SET position = position - 1 WHERE position > ?1 AND position <= ?2",
                [from, position],
            )?;
        } else {
            tx.execute(
                "UPDATE tabs SET position = position + 1 WHERE position >= ?1 AND position < ?2",
                [position, from],
            )?;
        }
        tx.execute(
            "UPDATE tabs SET position = ?2 WHERE id = ?1",
            params![id.as_str(), position],
        )?;
    }
    // a tab can only be reordered within its own block: normalize puts it back if the move crossed into another group
    normalize(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Put the tabs in visual order: ungrouped first, then each group in group order, each group's tabs together, and renumber 0, 1, 2, ...
/// Within a block the existing relative order is kept. Safe to call at any time; does nothing if the order is already right.
pub(crate) fn normalize(conn: &Connection) -> Result<(), TabError> {
    let mut s = conn.prepare(
        "SELECT t.id, t.position FROM tabs t LEFT JOIN tab_groups g ON g.id = t.group_id \
         ORDER BY (CASE WHEN t.group_id IS NULL THEN 0 ELSE 1 END), COALESCE(g.position, 0), t.position, t.id",
    )?;
    let rows: Vec<(String, u32)> = s
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (i, (id, position)) in rows.iter().enumerate() {
        if *position != i as u32 {
            conn.execute(
                "UPDATE tabs SET position = ?2 WHERE id = ?1",
                params![id, i as u32],
            )?;
        }
    }
    Ok(())
}

/// Record what the page says its title is. Untrusted: cleaned and cut short.
pub fn set_title(conn: &Connection, id: &Id, title: &str) -> Result<(), TabError> {
    let n = conn.execute(
        "UPDATE tabs SET title = ?2 WHERE id = ?1",
        params![id.as_str(), clean_title(title)],
    )?;
    if n == 0 {
        return Err(TabError::NotFound(id.clone()));
    }
    Ok(())
}

/// Record where the page navigated to, if (and only if) that address may be re-opened later. Returns whether it was stored.
pub fn set_url(conn: &Connection, id: &Id, new_url: &str) -> Result<bool, TabError> {
    if !url::is_restorable(new_url) {
        return Ok(false);
    }
    let n = conn.execute(
        "UPDATE tabs SET url = ?2 WHERE id = ?1",
        params![id.as_str(), new_url],
    )?;
    if n == 0 {
        return Err(TabError::NotFound(id.clone()));
    }
    Ok(true)
}

/// Remember this tab as the selected one (restored on the next start) and note when it was last active.
pub fn select(conn: &Connection, id: &Id) -> Result<(), TabError> {
    get(conn, id)?;
    conn.execute(
        "UPDATE tabs SET last_active = ?2 WHERE id = ?1",
        params![id.as_str(), now()],
    )?;
    conn.execute(
        "INSERT INTO workspace_state (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SELECTED, id.as_str()],
    )?;
    Ok(())
}

/// The selected tab, if one is remembered and still exists.
pub fn selected(conn: &Connection) -> Result<Option<Tab>, TabError> {
    let id: Option<String> = conn
        .query_row(
            "SELECT value FROM workspace_state WHERE key = ?1",
            [SELECTED],
            |r| r.get(0),
        )
        .optional()?;
    let Some(id) = id.and_then(|s| Id::parse(&s).ok()) else {
        return Ok(None);
    };
    match get(conn, &id) {
        Ok(t) => Ok(Some(t)),
        Err(TabError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
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

    fn urls(c: &Connection) -> Vec<String> {
        list(c).unwrap().into_iter().map(|t| t.url).collect()
    }

    fn three(c: &Connection) -> (Tab, Tab, Tab) {
        (
            create(c, "https://a.example/", None, None).unwrap(),
            create(c, "https://b.example/", None, None).unwrap(),
            create(c, "https://c.example/", None, None).unwrap(),
        )
    }

    #[test]
    fn tabs_are_added_in_order_and_positions_stay_dense() {
        let c = db();
        let (a, b, d) = three(&c);
        assert_eq!((a.position, b.position, d.position), (0, 1, 2));
        let mid = create(&c, "https://mid.example/", Some(1), None).unwrap();
        assert_eq!(mid.position, 1);
        assert_eq!(
            urls(&c),
            vec![
                "https://a.example/",
                "https://mid.example/",
                "https://b.example/",
                "https://c.example/"
            ]
        );
        let positions: Vec<u32> = list(&c).unwrap().iter().map(|t| t.position).collect();
        assert_eq!(positions, vec![0, 1, 2, 3]);
        // a position past the end means the end
        assert_eq!(
            create(&c, "https://end.example/", Some(99), None)
                .unwrap()
                .position,
            4
        );
    }

    #[test]
    fn urls_that_may_not_be_reopened_are_refused_and_nothing_changes() {
        let c = db();
        for bad in [
            "javascript:alert(1)",
            "file:///C:/x",
            "app://shell/",
            "data:text/html,x",
            "",
        ] {
            assert!(
                matches!(create(&c, bad, None, None), Err(TabError::BadUrl)),
                "{bad}"
            );
        }
        assert!(list(&c).unwrap().is_empty());
    }

    #[test]
    fn closing_a_tab_closes_the_gap_and_forgets_the_selection() {
        let c = db();
        let (a, b, d) = three(&c);
        select(&c, &b.id).unwrap();
        close(&c, &b.id).unwrap();
        assert_eq!(urls(&c), vec!["https://a.example/", "https://c.example/"]);
        assert_eq!(get(&c, &d.id).unwrap().position, 1);
        assert_eq!(get(&c, &a.id).unwrap().position, 0);
        assert_eq!(
            selected(&c).unwrap(),
            None,
            "the closed tab is no longer selected"
        );
        assert!(matches!(close(&c, &b.id), Err(TabError::NotFound(_))));
    }

    #[test]
    fn closing_another_tab_keeps_the_selection() {
        let c = db();
        let (a, b, _d) = three(&c);
        select(&c, &a.id).unwrap();
        close(&c, &b.id).unwrap();
        assert_eq!(selected(&c).unwrap().unwrap().id, a.id);
    }

    #[test]
    fn move_shifts_the_others_in_both_directions() {
        let c = db();
        let (a, _b, d) = three(&c);
        move_to(&c, &a.id, 2).unwrap();
        assert_eq!(
            urls(&c),
            vec![
                "https://b.example/",
                "https://c.example/",
                "https://a.example/"
            ]
        );
        move_to(&c, &d.id, 0).unwrap();
        assert_eq!(
            urls(&c),
            vec![
                "https://c.example/",
                "https://b.example/",
                "https://a.example/"
            ]
        );
        move_to(&c, &d.id, 0).unwrap(); // already there: nothing changes
        assert_eq!(urls(&c)[0], "https://c.example/");
        let positions: Vec<u32> = list(&c).unwrap().iter().map(|t| t.position).collect();
        assert_eq!(positions, vec![0, 1, 2]);
        assert!(matches!(move_to(&c, &d.id, 3), Err(TabError::BadPosition)));
        let ghost = Id::new("tab");
        assert!(matches!(move_to(&c, &ghost, 0), Err(TabError::NotFound(_))));
    }

    #[test]
    fn titles_are_cleaned_and_urls_only_stored_when_restorable() {
        let c = db();
        let (a, _b, _d) = three(&c);
        set_title(&c, &a.id, "  Hello\n\tworld\u{7}  ").unwrap();
        assert_eq!(get(&c, &a.id).unwrap().title, "Hello  world");
        set_title(&c, &a.id, &"x".repeat(1000)).unwrap();
        assert_eq!(
            get(&c, &a.id).unwrap().title.chars().count(),
            MAX_TITLE_CHARS
        );
        assert!(set_url(&c, &a.id, "https://new.example/").unwrap());
        assert_eq!(get(&c, &a.id).unwrap().url, "https://new.example/");
        assert!(!set_url(&c, &a.id, "javascript:alert(1)").unwrap());
        assert!(!set_url(&c, &a.id, "data:text/html,x").unwrap());
        assert_eq!(
            get(&c, &a.id).unwrap().url,
            "https://new.example/",
            "an unrestorable address is never stored"
        );
        let ghost = Id::new("tab");
        assert!(matches!(
            set_title(&c, &ghost, "x"),
            Err(TabError::NotFound(_))
        ));
        assert!(matches!(
            set_url(&c, &ghost, "https://x.example/"),
            Err(TabError::NotFound(_))
        ));
    }

    #[test]
    fn selection_is_remembered_and_forgotten_if_the_tab_is_gone() {
        let c = db();
        let (a, b, _d) = three(&c);
        assert_eq!(selected(&c).unwrap(), None);
        select(&c, &b.id).unwrap();
        assert_eq!(selected(&c).unwrap().unwrap().id, b.id);
        select(&c, &a.id).unwrap();
        assert_eq!(selected(&c).unwrap().unwrap().id, a.id);
        // a stale id left in the state (hand edit, crash) reads as "none", not an error
        c.execute(
            "UPDATE workspace_state SET value = 'tab-0000000000000001' WHERE key = 'last_tab_id'",
            [],
        )
        .unwrap();
        assert_eq!(selected(&c).unwrap(), None);
        let ghost = Id::new("tab");
        assert!(matches!(select(&c, &ghost), Err(TabError::NotFound(_))));
    }

    #[test]
    fn a_hand_edited_bad_id_is_refused_when_read() {
        let c = db();
        c.execute("INSERT INTO tabs (id, url, position, created_at) VALUES ('../evil', 'https://x/', 0, 0)", []).unwrap();
        assert!(list(&c).is_err());
    }
}
