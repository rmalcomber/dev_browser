//! A small, hand-written migration runner for SQLite.
//!
//! Each database (the global one, and one per workspace) has its own list of migrations. A migration is a version number, a name and
//! some SQL. The runner records applied versions in `schema_migrations`, applies the missing ones in order, each inside its own
//! transaction, and refuses to touch a database written by a newer version of the program.
//!
//! The spec warns that there are no atomic transactions across databases, so a migration only ever changes the one database it is run on.

use rusqlite::Connection;
use std::fmt;

/// One step of a database's history. Versions start at 1, rise by exactly 1, and never change once released.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug)]
pub enum MigrateError {
    /// The list itself is wrong (not 1, 2, 3 ... in order). A programming error, caught by tests.
    BadList(String),
    /// The database has a version this build does not know: it was written by a newer program. Never downgrade silently.
    DatabaseIsNewer {
        database: u32,
        known: u32,
    },
    /// A migration failed; its changes were rolled back.
    Failed {
        version: u32,
        name: &'static str,
        source: rusqlite::Error,
    },
    Sqlite(rusqlite::Error),
}

impl fmt::Display for MigrateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrateError::BadList(m) => write!(f, "bad migration list: {m}"),
            MigrateError::DatabaseIsNewer { database, known } => {
                write!(f, "the database is at version {database} but this build only knows up to {known}; it was written by a newer version")
            }
            MigrateError::Failed {
                version,
                name,
                source,
            } => write!(
                f,
                "migration {version} ({name}) failed and was rolled back: {source}"
            ),
            MigrateError::Sqlite(e) => write!(f, "sqlite error: {e}"),
        }
    }
}

impl std::error::Error for MigrateError {}

impl From<rusqlite::Error> for MigrateError {
    fn from(e: rusqlite::Error) -> Self {
        MigrateError::Sqlite(e)
    }
}

/// What a run did, so the caller can log it.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub from: u32,
    pub to: u32,
    pub applied: Vec<u32>,
}

fn check_list(list: &[Migration]) -> Result<(), MigrateError> {
    for (i, m) in list.iter().enumerate() {
        let want = i as u32 + 1;
        if m.version != want {
            return Err(MigrateError::BadList(format!(
                "entry {i} has version {}, expected {want}",
                m.version
            )));
        }
        if m.name.is_empty() || m.sql.trim().is_empty() {
            return Err(MigrateError::BadList(format!(
                "migration {} has an empty name or empty SQL",
                m.version
            )));
        }
    }
    Ok(())
}

/// Bring `conn` up to the newest version in `list`. Safe to call on every start.
pub fn migrate(conn: &mut Connection, list: &[Migration]) -> Result<Report, MigrateError> {
    check_list(list)?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at INTEGER NOT NULL);")?;
    let current: u32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    let known = list.len() as u32;
    if current > known {
        return Err(MigrateError::DatabaseIsNewer {
            database: current,
            known,
        });
    }
    let mut applied = Vec::new();
    for m in list.iter().filter(|m| m.version > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)
            .map_err(|source| MigrateError::Failed {
                version: m.version,
                name: m.name,
                source,
            })?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, strftime('%s','now'))",
            rusqlite::params![m.version, m.name],
        )?;
        tx.commit()?;
        applied.push(m.version);
    }
    Ok(Report {
        from: current,
        to: current.max(applied.last().copied().unwrap_or(current)),
        applied,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1: Migration = Migration {
        version: 1,
        name: "create a",
        sql: "CREATE TABLE a (id INTEGER PRIMARY KEY);",
    };
    const V2: Migration = Migration {
        version: 2,
        name: "create b",
        sql: "CREATE TABLE b (id INTEGER PRIMARY KEY);",
    };

    fn tables(c: &Connection) -> Vec<String> {
        let mut s = c.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
        s.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn applies_everything_to_a_fresh_database() {
        let mut c = Connection::open_in_memory().unwrap();
        let r = migrate(&mut c, &[V1, V2]).unwrap();
        assert_eq!(
            r,
            Report {
                from: 0,
                to: 2,
                applied: vec![1, 2]
            }
        );
        assert_eq!(tables(&c), vec!["a", "b", "schema_migrations"]);
    }

    #[test]
    fn running_again_does_nothing() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, &[V1, V2]).unwrap();
        let r = migrate(&mut c, &[V1, V2]).unwrap();
        assert_eq!(
            r,
            Report {
                from: 2,
                to: 2,
                applied: vec![]
            }
        );
    }

    #[test]
    fn applies_only_the_missing_migrations() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, &[V1]).unwrap();
        let r = migrate(&mut c, &[V1, V2]).unwrap();
        assert_eq!(
            r,
            Report {
                from: 1,
                to: 2,
                applied: vec![2]
            }
        );
    }

    #[test]
    fn a_failing_migration_rolls_back_and_leaves_the_version_alone() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, &[V1]).unwrap();
        let bad = Migration {
            version: 2,
            name: "half done",
            sql: "CREATE TABLE half (id INTEGER); THIS IS NOT SQL;",
        };
        let err = migrate(&mut c, &[V1, bad]).unwrap_err();
        assert!(
            matches!(err, MigrateError::Failed { version: 2, .. }),
            "{err}"
        );
        assert!(
            !tables(&c).contains(&"half".to_string()),
            "the first statement of the failed migration must be rolled back"
        );
        let v: u32 = c
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, 1);
        // and a corrected migration can then be applied
        let r = migrate(&mut c, &[V1, V2]).unwrap();
        assert_eq!(r.applied, vec![2]);
    }

    #[test]
    fn refuses_a_database_from_a_newer_build() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, &[V1, V2]).unwrap();
        let err = migrate(&mut c, &[V1]).unwrap_err();
        assert!(
            matches!(
                err,
                MigrateError::DatabaseIsNewer {
                    database: 2,
                    known: 1
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn rejects_a_bad_migration_list() {
        let mut c = Connection::open_in_memory().unwrap();
        let skip = Migration { version: 3, ..V2 };
        assert!(matches!(
            migrate(&mut c, &[V1, skip]),
            Err(MigrateError::BadList(_))
        ));
        assert!(matches!(
            migrate(&mut c, &[V2, V1]),
            Err(MigrateError::BadList(_))
        ));
        let empty = Migration { sql: "  ", ..V1 };
        assert!(matches!(
            migrate(&mut c, &[empty]),
            Err(MigrateError::BadList(_))
        ));
        // nothing was created by the rejected runs
        assert!(tables(&c).is_empty());
    }
}
