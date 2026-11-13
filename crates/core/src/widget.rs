//! Pinned tools: a widget is an **instance** of a bundled tool in one workspace, listed in the right-hand panel. Its row lives in that
//! workspace's `homescreen.db` and holds only the tool, its place in the list and whether it is collapsed. **Never the text typed into
//! it**: a widget's input and output exist in memory (the panel page, and a moment in core while a tool runs), so a pinned JWT decoder
//! cannot leave a token in a database (S9).

use rusqlite::{params, Connection, OptionalExtension};

/// Most widgets in one workspace.
pub const MAX_WIDGETS: i64 = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Widget {
    pub id: i64,
    pub tool: String,
    pub position: i64,
    pub collapsed: bool,
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Widget> {
    Ok(Widget {
        id: r.get(0)?,
        tool: r.get(1)?,
        position: r.get(2)?,
        collapsed: r.get::<_, i64>(3)? != 0,
    })
}

/// Pin an instance of a tool at the end of the list. Several instances of one tool are allowed.
pub fn pin(conn: &Connection, tool: &str) -> Result<Widget, String> {
    if crate::tools::info(tool).is_none() {
        return Err("there is no such tool".into());
    }
    let sql = |e: rusqlite::Error| e.to_string();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM widgets", [], |r| r.get(0))
        .map_err(sql)?;
    if n >= MAX_WIDGETS {
        return Err("there are too many pinned tools in this workspace".into());
    }
    let next: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM widgets",
            [],
            |r| r.get(0),
        )
        .map_err(sql)?;
    conn.execute(
        "INSERT INTO widgets (tool, position, collapsed) VALUES (?1, ?2, 0)",
        params![tool, next],
    )
    .map_err(sql)?;
    let id = conn.last_insert_rowid();
    Ok(get(conn, id).map_err(sql)?.expect("just pinned"))
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Widget>> {
    conn.query_row(
        "SELECT id, tool, position, collapsed FROM widgets WHERE id = ?1",
        [id],
        row,
    )
    .optional()
}

/// In display order.
pub fn all(conn: &Connection) -> rusqlite::Result<Vec<Widget>> {
    conn.prepare("SELECT id, tool, position, collapsed FROM widgets ORDER BY position, id")?
        .query_map([], row)?
        .collect()
}

/// Remove a widget. Returns whether there was one.
pub fn unpin(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM widgets WHERE id = ?1", [id])? > 0)
}

pub fn set_collapsed(conn: &Connection, id: i64, collapsed: bool) -> rusqlite::Result<bool> {
    Ok(conn.execute(
        "UPDATE widgets SET collapsed = ?2 WHERE id = ?1",
        params![id, i64::from(collapsed)],
    )? > 0)
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
    fn widgets_are_pinned_in_order_collapsed_and_removed() {
        let c = db();
        let a = pin(&c, "jwt").unwrap();
        let b = pin(&c, "json").unwrap();
        let again = pin(&c, "jwt").unwrap();
        assert_ne!(a.id, again.id, "several instances of one tool are fine");
        let list: Vec<(String, i64)> = all(&c)
            .unwrap()
            .into_iter()
            .map(|w| (w.tool, w.position))
            .collect();
        assert_eq!(
            list,
            vec![("jwt".into(), 0), ("json".into(), 1), ("jwt".into(), 2)]
        );
        assert!(set_collapsed(&c, b.id, true).unwrap());
        assert!(get(&c, b.id).unwrap().unwrap().collapsed);
        assert!(unpin(&c, a.id).unwrap());
        assert!(!unpin(&c, a.id).unwrap());
        assert_eq!(all(&c).unwrap().len(), 2);
        assert!(!set_collapsed(&c, 9999, true).unwrap());
    }

    #[test]
    fn unknown_tools_and_too_many_are_refused() {
        let c = db();
        assert!(pin(&c, "nope").is_err());
        for _ in 0..MAX_WIDGETS {
            pin(&c, "uuid").unwrap();
        }
        assert!(pin(&c, "uuid").is_err());
    }

    #[test]
    fn the_table_has_no_column_that_could_hold_what_was_typed() {
        let c = db();
        let cols: Vec<String> = c
            .prepare("SELECT name FROM pragma_table_info('widgets')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(cols, vec!["id", "tool", "position", "collapsed"]);
    }
}
