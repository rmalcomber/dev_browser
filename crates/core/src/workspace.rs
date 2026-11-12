//! Workspaces: the user-named, colour-coded profiles. The records live in the global database; each workspace's browsing data lives in its
//! own database and Chromium profile (see `data`). Deleting the record here does not delete those folders: `data::DataDir::delete_workspace`
//! does both, in the right order.

use crate::id::Id;
use rusqlite::{params, Connection, OptionalExtension};
use std::fmt;

pub const MAX_NAME_CHARS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub id: Id,
    pub name: String,
    /// 0xRRGGBB
    pub colour: u32,
    pub position: u32,
}

#[derive(Debug)]
pub enum WorkspaceError {
    InvalidName(&'static str),
    InvalidColour(u32),
    NotFound(Id),
    Sqlite(rusqlite::Error),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceError::InvalidName(why) => write!(f, "invalid workspace name: {why}"),
            WorkspaceError::InvalidColour(c) => {
                write!(f, "invalid colour {c:#x}: must be 0xRRGGBB")
            }
            WorkspaceError::NotFound(id) => write!(f, "no workspace {id}"),
            WorkspaceError::Sqlite(e) => write!(f, "sqlite error: {e}"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

impl From<rusqlite::Error> for WorkspaceError {
    fn from(e: rusqlite::Error) -> Self {
        WorkspaceError::Sqlite(e)
    }
}

/// Trim, then require 1 to 64 characters with no control characters.
pub fn validate_name(name: &str) -> Result<String, WorkspaceError> {
    let n = name.trim();
    if n.is_empty() {
        return Err(WorkspaceError::InvalidName("it is empty"));
    }
    if n.chars().count() > MAX_NAME_CHARS {
        return Err(WorkspaceError::InvalidName(
            "it is longer than 64 characters",
        ));
    }
    if n.chars().any(|c| c.is_control()) {
        return Err(WorkspaceError::InvalidName(
            "it contains a control character",
        ));
    }
    Ok(n.to_string())
}

fn validate_colour(colour: u32) -> Result<u32, WorkspaceError> {
    if colour > 0xFF_FFFF {
        Err(WorkspaceError::InvalidColour(colour))
    } else {
        Ok(colour)
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    let id: String = r.get(0)?;
    Ok(Workspace {
        // ids in the database were written by `create`; a bad one means the file was edited by hand, so fail loudly rather than build a path from it
        id: Id::parse(&id).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: r.get(1)?,
        colour: r.get(2)?,
        position: r.get(3)?,
    })
}

pub fn create(conn: &Connection, name: &str, colour: u32) -> Result<Workspace, WorkspaceError> {
    let name = validate_name(name)?;
    let colour = validate_colour(colour)?;
    let id = Id::new("ws");
    let position: u32 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM workspaces",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO workspaces (id, name, colour, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id.as_str(), name, colour, position, now()],
    )?;
    Ok(Workspace {
        id,
        name,
        colour,
        position,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Workspace>, WorkspaceError> {
    let mut s =
        conn.prepare("SELECT id, name, colour, position FROM workspaces ORDER BY position")?;
    let rows = s.query_map([], row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: &Id) -> Result<Workspace, WorkspaceError> {
    conn.query_row(
        "SELECT id, name, colour, position FROM workspaces WHERE id = ?1",
        [id.as_str()],
        row,
    )
    .optional()?
    .ok_or_else(|| WorkspaceError::NotFound(id.clone()))
}

pub fn rename(conn: &Connection, id: &Id, name: &str) -> Result<Workspace, WorkspaceError> {
    let name = validate_name(name)?;
    if conn.execute(
        "UPDATE workspaces SET name = ?2 WHERE id = ?1",
        params![id.as_str(), name],
    )? == 0
    {
        return Err(WorkspaceError::NotFound(id.clone()));
    }
    get(conn, id)
}

pub fn set_colour(conn: &Connection, id: &Id, colour: u32) -> Result<Workspace, WorkspaceError> {
    let colour = validate_colour(colour)?;
    if conn.execute(
        "UPDATE workspaces SET colour = ?2 WHERE id = ?1",
        params![id.as_str(), colour],
    )? == 0
    {
        return Err(WorkspaceError::NotFound(id.clone()));
    }
    get(conn, id)
}

/// Remove the record and close the gap in the ordering. Does not touch any folder on disk (see the module comment).
pub fn delete_record(conn: &mut Connection, id: &Id) -> Result<(), WorkspaceError> {
    let tx = conn.transaction()?;
    let position: u32 = tx
        .query_row(
            "SELECT position FROM workspaces WHERE id = ?1",
            [id.as_str()],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| WorkspaceError::NotFound(id.clone()))?;
    tx.execute("DELETE FROM workspaces WHERE id = ?1", [id.as_str()])?;
    // the unique index on position would trip on an in-place shift, so move the rows past the gap in two steps
    tx.execute(
        "UPDATE workspaces SET position = position + 1000000 WHERE position > ?1",
        [position],
    )?;
    tx.execute(
        "UPDATE workspaces SET position = position - 1000001 WHERE position > 1000000",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate::migrate, schema};

    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, schema::GLOBAL).unwrap();
        c
    }

    #[test]
    fn create_list_rename_colour() {
        let c = db();
        let work = create(&c, "  Work ", 0x2C5FA8).unwrap();
        let home = create(&c, "Personal", 0xA78BFA).unwrap();
        assert_eq!(
            (work.name.as_str(), work.position, home.position),
            ("Work", 0, 1)
        );
        assert_eq!(list(&c).unwrap(), vec![work.clone(), home.clone()]);
        assert_eq!(rename(&c, &home.id, "Home").unwrap().name, "Home");
        assert_eq!(set_colour(&c, &home.id, 0x00FF00).unwrap().colour, 0x00FF00);
        assert_eq!(get(&c, &work.id).unwrap(), work);
    }

    #[test]
    fn rejects_bad_names_and_colours_and_changes_nothing() {
        let c = db();
        for bad in ["", "   ", "tab\there", "x\ny", &"n".repeat(65)] {
            assert!(
                matches!(create(&c, bad, 0), Err(WorkspaceError::InvalidName(_))),
                "{bad:?}"
            );
        }
        assert!(matches!(
            create(&c, "ok", 0x1_000_000),
            Err(WorkspaceError::InvalidColour(_))
        ));
        assert!(list(&c).unwrap().is_empty());
        assert!(create(&c, &"n".repeat(64), 0xFFFFFF).is_ok());
    }

    #[test]
    fn unknown_workspace_is_an_error_not_a_silent_no_op() {
        let c = db();
        let ghost = Id::new("ws");
        assert!(matches!(
            rename(&c, &ghost, "x"),
            Err(WorkspaceError::NotFound(_))
        ));
        assert!(matches!(
            set_colour(&c, &ghost, 1),
            Err(WorkspaceError::NotFound(_))
        ));
        assert!(matches!(get(&c, &ghost), Err(WorkspaceError::NotFound(_))));
    }

    #[test]
    fn delete_closes_the_gap_in_the_ordering() {
        let mut c = db();
        let a = create(&c, "A", 1).unwrap();
        let b = create(&c, "B", 2).unwrap();
        let d = create(&c, "C", 3).unwrap();
        delete_record(&mut c, &b.id).unwrap();
        let left = list(&c).unwrap();
        assert_eq!(
            left.iter()
                .map(|w| (w.name.as_str(), w.position))
                .collect::<Vec<_>>(),
            vec![("A", 0), ("C", 1)]
        );
        assert_eq!(left[0].id, a.id);
        assert_eq!(left[1].id, d.id);
        assert!(matches!(
            delete_record(&mut c, &b.id),
            Err(WorkspaceError::NotFound(_))
        ));
        // a new workspace goes to the end
        assert_eq!(create(&c, "D", 4).unwrap().position, 2);
    }

    #[test]
    fn a_hand_edited_bad_id_is_refused_when_read() {
        let c = db();
        c.execute("INSERT INTO workspaces (id, name, colour, position, created_at) VALUES ('../evil', 'x', 0, 0, 0)", []).unwrap();
        assert!(list(&c).is_err());
    }
}
