//! The migrations for each database. Released migrations are never edited: a change is a new entry at the end of its list.
//!
//! Privacy shape (spec section 6.2): the **global** database holds only workspace identity, ordering and app preferences, no history,
//! no tab titles, no URLs. Everything that describes what the user browsed lives in that workspace's own database.

use crate::migrate::Migration;

pub const GLOBAL: &[Migration] = &[Migration {
    version: 1,
    name: "workspaces and settings",
    sql: "
        CREATE TABLE workspaces (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            colour      INTEGER NOT NULL,            -- 0xRRGGBB
            position    INTEGER NOT NULL,
            created_at  INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX workspaces_position ON workspaces (position);
        CREATE TABLE settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ",
}];

pub const WORKSPACE: &[Migration] = &[
    Migration {
    version: 1,
    name: "tab groups, tabs and workspace state",
    sql: "
        CREATE TABLE tab_groups (
            id        TEXT PRIMARY KEY,
            name      TEXT NOT NULL,
            position  INTEGER NOT NULL,
            collapsed INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE tabs (
            id          TEXT PRIMARY KEY,
            group_id    TEXT REFERENCES tab_groups (id) ON DELETE SET NULL,
            url         TEXT NOT NULL,
            title       TEXT NOT NULL DEFAULT '',
            position    INTEGER NOT NULL,
            created_at  INTEGER NOT NULL,
            last_active INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX tabs_group ON tabs (group_id, position);
        CREATE TABLE workspace_state (
            key   TEXT PRIMARY KEY,                  -- e.g. 'last_tab_id': restore-last-tab lives here, not in the global database
            value TEXT NOT NULL
        );
    ",
    },
    Migration {
        version: 2,
        name: "history and bookmarks",
        sql: "
        CREATE TABLE history (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            url        TEXT NOT NULL UNIQUE,         -- a web page without its #fragment
            title      TEXT NOT NULL DEFAULT '',
            visits     INTEGER NOT NULL DEFAULT 1,
            last_visit INTEGER NOT NULL
        );
        CREATE INDEX history_last_visit ON history (last_visit);
        CREATE TABLE bookmarks (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            url        TEXT NOT NULL UNIQUE,
            title      TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL
        );
    ",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::migrate;
    use rusqlite::Connection;

    #[test]
    fn both_lists_are_valid_and_apply() {
        for list in [GLOBAL, WORKSPACE] {
            let mut c = Connection::open_in_memory().unwrap();
            let r = migrate(&mut c, list).unwrap();
            assert_eq!(r.to as usize, list.len());
        }
    }

    #[test]
    fn the_global_database_has_no_browsing_data() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, GLOBAL).unwrap();
        let cols: Vec<String> = c
            .prepare("SELECT m.name || '.' || p.name FROM sqlite_master m JOIN pragma_table_info(m.name) p WHERE m.type = 'table'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for forbidden in ["url", "title", "history", "visit", "cookie"] {
            assert!(
                !cols.iter().any(|c| c.contains(forbidden)),
                "the global database must not hold {forbidden}: {cols:?}"
            );
        }
    }

    #[test]
    fn an_existing_workspace_database_gains_history_and_bookmarks() {
        let mut c = Connection::open_in_memory().unwrap();
        let r = migrate(&mut c, &WORKSPACE[..1]).unwrap();
        assert_eq!(r.to, 1);
        c.execute("INSERT INTO tabs (id, url, title, position, created_at) VALUES ('tab-0000000000000001', 'https://a.example/', 'A', 0, 0)", []).unwrap();
        let r = migrate(&mut c, WORKSPACE).unwrap();
        assert_eq!(r.to as usize, WORKSPACE.len());
        let tabs: i64 = c
            .query_row("SELECT COUNT(*) FROM tabs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tabs, 1, "existing data is kept");
        for table in ["history", "bookmarks"] {
            let n: i64 = c
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 0);
        }
    }
}
