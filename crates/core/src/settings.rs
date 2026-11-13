//! App-wide preferences in the global database's `settings` table. Only preferences that are not about browsing (no URLs, no titles):
//! the global database must stay free of browsing data (see `schema`).

use rusqlite::{params, Connection, OptionalExtension};

pub const ACTIVE_WORKSPACE: &str = "active_workspace";

pub fn get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .optional()
}

pub fn set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate::migrate, schema};

    #[test]
    fn set_get_and_overwrite() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, schema::GLOBAL).unwrap();
        assert_eq!(get(&c, "k").unwrap(), None);
        set(&c, "k", "one").unwrap();
        assert_eq!(get(&c, "k").unwrap().as_deref(), Some("one"));
        set(&c, "k", "two").unwrap();
        assert_eq!(get(&c, "k").unwrap().as_deref(), Some("two"));
    }
}
