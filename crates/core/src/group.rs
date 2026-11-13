//! Tab groups, in a workspace's own database. A group is a named, collapsible run of tabs in the sidebar. The tab order (`tab::normalize`)
//! keeps each group's tabs together, so the group is also what `Ctrl+Tab` walks through. A group with no tabs is deleted: there are no
//! empty groups.

use crate::id::Id;
use crate::tab::{self, TabError};
use crate::workspace;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub id: Id,
    pub name: String,
    pub position: u32,
    pub collapsed: bool,
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Group> {
    let id: String = r.get(0)?;
    Ok(Group {
        id: Id::parse(&id).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: r.get(1)?,
        position: r.get(2)?,
        collapsed: r.get::<_, i64>(3)? != 0,
    })
}

fn valid_name(name: &str) -> Result<String, TabError> {
    workspace::validate_name(name).map_err(|e| match e {
        workspace::WorkspaceError::InvalidName(why) => TabError::InvalidName(why),
        _ => TabError::InvalidName("it is not acceptable"),
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Group>, TabError> {
    let mut s =
        conn.prepare("SELECT id, name, position, collapsed FROM tab_groups ORDER BY position")?;
    let groups = s.query_map([], row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(groups)
}

pub fn get(conn: &Connection, id: &Id) -> Result<Group, TabError> {
    conn.query_row(
        "SELECT id, name, position, collapsed FROM tab_groups WHERE id = ?1",
        [id.as_str()],
        row,
    )
    .optional()?
    .ok_or_else(|| TabError::NoSuchGroup(id.clone()))
}

/// Make a new group (after the existing ones) containing `tab_id`, which leaves any group it was in.
pub fn create_for(conn: &Connection, tab_id: &Id, name: &str) -> Result<Group, TabError> {
    let name = valid_name(name)?;
    tab::get(conn, tab_id)?;
    let tx = conn.unchecked_transaction()?;
    let position: u32 = tx.query_row("SELECT COUNT(*) FROM tab_groups", [], |r| r.get(0))?;
    let id = Id::new("grp");
    tx.execute(
        "INSERT INTO tab_groups (id, name, position, collapsed) VALUES (?1, ?2, ?3, 0)",
        params![id.as_str(), name, position],
    )?;
    tx.execute(
        "UPDATE tabs SET group_id = ?2 WHERE id = ?1",
        params![tab_id.as_str(), id.as_str()],
    )?;
    prune_empty(&tx)?; // the tab's old group may now be empty
    tab::normalize(&tx)?;
    tx.commit()?;
    get(conn, &id)
}

pub fn rename(conn: &Connection, id: &Id, name: &str) -> Result<(), TabError> {
    let name = valid_name(name)?;
    get(conn, id)?;
    conn.execute(
        "UPDATE tab_groups SET name = ?2 WHERE id = ?1",
        params![id.as_str(), name],
    )?;
    Ok(())
}

pub fn set_collapsed(conn: &Connection, id: &Id, collapsed: bool) -> Result<(), TabError> {
    get(conn, id)?;
    conn.execute(
        "UPDATE tab_groups SET collapsed = ?2 WHERE id = ?1",
        params![id.as_str(), collapsed as i64],
    )?;
    Ok(())
}

/// Put a tab into a group, or out of every group (`None`). The group must exist in this workspace.
pub fn assign(conn: &Connection, tab_id: &Id, group: Option<&Id>) -> Result<(), TabError> {
    tab::get(conn, tab_id)?;
    if let Some(g) = group {
        get(conn, g)?;
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE tabs SET group_id = ?2 WHERE id = ?1",
        params![tab_id.as_str(), group.map(|g| g.as_str())],
    )?;
    prune_empty(&tx)?;
    tab::normalize(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Delete groups that have no tabs and close the gaps in the group order.
pub fn prune_empty(conn: &Connection) -> Result<(), TabError> {
    conn.execute(
        "DELETE FROM tab_groups WHERE id NOT IN (SELECT group_id FROM tabs WHERE group_id IS NOT NULL)",
        [],
    )?;
    let ids: Vec<String> = {
        let mut s = conn.prepare("SELECT id FROM tab_groups ORDER BY position")?;
        let rows = s.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        rows
    };
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE tab_groups SET position = ?2 WHERE id = ?1 AND position != ?2",
            params![id, i as u32],
        )?;
    }
    Ok(())
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

    /// Add a tab whose address is https://<name>.example/ and return its id.
    fn add(c: &Connection, name: &str) -> Id {
        tab::create(c, &format!("https://{name}.example/"), None, None)
            .unwrap()
            .id
    }

    /// The tab names in visual order, e.g. ["b", "c", "a"].
    fn order(c: &Connection) -> Vec<String> {
        tab::list(c)
            .unwrap()
            .into_iter()
            .map(|t| {
                t.url
                    .trim_start_matches("https://")
                    .trim_end_matches(".example/")
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn a_new_group_takes_its_tab_and_ungrouped_tabs_stay_first() {
        let c = db();
        let (a, b, _c) = (add(&c, "a"), add(&c, "b"), add(&c, "c"));
        let g = create_for(&c, &a, "Work").unwrap();
        assert_eq!(
            (g.name.as_str(), g.position, g.collapsed),
            ("Work", 0, false)
        );
        assert_eq!(order(&c), vec!["b", "c", "a"]);
        assert_eq!(tab::get(&c, &a).unwrap().group, Some(g.id));
        assert_eq!(tab::get(&c, &b).unwrap().group, None);
    }

    #[test]
    fn a_groups_tabs_stay_together_and_follow_group_order() {
        let c = db();
        let (a, b, c_, _d) = (add(&c, "a"), add(&c, "b"), add(&c, "c"), add(&c, "d"));
        let g1 = create_for(&c, &b, "One").unwrap(); // [a, c, d | One: b]
        let g2 = create_for(&c, &a, "Two").unwrap(); // [c, d | One: b | Two: a]
        assert_eq!(order(&c), vec!["c", "d", "b", "a"]);
        assert_eq!((g1.position, g2.position), (0, 1));
        assign(&c, &c_, Some(&g2.id)).unwrap(); // c joins Two, which keeps its tabs together: [d | One: b | Two: c, a]
        assert_eq!(order(&c), vec!["d", "b", "c", "a"]);
        assert_eq!(tab::get(&c, &c_).unwrap().group, Some(g2.id));
        let positions: Vec<u32> = tab::list(&c).unwrap().iter().map(|t| t.position).collect();
        assert_eq!(positions, vec![0, 1, 2, 3], "dense");
    }

    #[test]
    fn leaving_the_last_group_deletes_it_and_moves_the_tab_to_the_ungrouped_block() {
        let c = db();
        let (a, _b) = (add(&c, "a"), add(&c, "b"));
        let g = create_for(&c, &a, "Solo").unwrap(); // [b | Solo: a]
        assign(&c, &a, None).unwrap();
        assert!(list(&c).unwrap().is_empty(), "an empty group is deleted");
        assert_eq!(order(&c), vec!["b", "a"]);
        assert_eq!(tab::get(&c, &a).unwrap().group, None);
        assert!(matches!(get(&c, &g.id), Err(TabError::NoSuchGroup(_))));
    }

    #[test]
    fn moving_a_tab_into_another_group_prunes_the_emptied_one_and_renumbers() {
        let c = db();
        let (a, b, _c) = (add(&c, "a"), add(&c, "b"), add(&c, "c"));
        let g1 = create_for(&c, &a, "One").unwrap(); // [b, c | One: a]
        let g2 = create_for(&c, &b, "Two").unwrap(); // [c | One: a | Two: b]
        assert_eq!(order(&c), vec!["c", "a", "b"]);
        assign(&c, &a, Some(&g2.id)).unwrap(); // One is empty now: gone
        let groups = list(&c).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!((groups[0].id.clone(), groups[0].position), (g2.id, 0));
        assert!(matches!(get(&c, &g1.id), Err(TabError::NoSuchGroup(_))));
        assert_eq!(order(&c), vec!["c", "a", "b"]);
    }

    #[test]
    fn closing_the_last_tab_of_a_group_deletes_the_group() {
        let c = db();
        let (a, _b) = (add(&c, "a"), add(&c, "b"));
        create_for(&c, &a, "Solo").unwrap();
        tab::close(&c, &a).unwrap();
        assert!(list(&c).unwrap().is_empty());
        let positions: Vec<u32> = tab::list(&c).unwrap().iter().map(|t| t.position).collect();
        assert_eq!(positions, vec![0]);
    }

    #[test]
    fn a_new_tab_in_a_group_lands_inside_that_groups_block() {
        let c = db();
        let (a, _b, d) = (add(&c, "a"), add(&c, "b"), add(&c, "c"));
        let g = create_for(&c, &a, "G").unwrap(); // [b, c | G: a]
        assign(&c, &d, Some(&g.id)).unwrap(); // [b | G: c, a]
        assert_eq!(order(&c), vec!["b", "c", "a"]);
        // a new tab requested right after a, in G
        let after_a = tab::get(&c, &a).unwrap().position + 1;
        let n = tab::create(&c, "https://new.example/", Some(after_a), Some(&g.id)).unwrap();
        assert_eq!(order(&c), vec!["b", "c", "a", "new"]);
        assert_eq!(n.group, Some(g.id.clone()));
        // an ungrouped tab requested in the middle of G's block is moved out to the ungrouped block
        let loose = tab::create(&c, "https://loose.example/", Some(2), None).unwrap();
        assert_eq!(order(&c), vec!["b", "loose", "c", "a", "new"]);
        assert_eq!(loose.group, None);
    }

    #[test]
    fn a_move_cannot_take_a_tab_out_of_its_block() {
        let c = db();
        let (a, b, d) = (add(&c, "a"), add(&c, "b"), add(&c, "c"));
        let g = create_for(&c, &b, "G").unwrap();
        assign(&c, &d, Some(&g.id)).unwrap(); // [a | G: c, b]
        assert_eq!(order(&c), vec!["a", "c", "b"]);
        tab::move_to(&c, &a, 2).unwrap(); // would put the ungrouped tab among the group's tabs
        assert_eq!(
            order(&c),
            vec!["a", "c", "b"],
            "it went back to its own block"
        );
        tab::move_to(&c, &d, 2).unwrap(); // inside the group it works
        assert_eq!(order(&c), vec!["a", "b", "c"]);
    }

    #[test]
    fn rename_collapse_and_validation() {
        let c = db();
        let a = add(&c, "a");
        let g = create_for(&c, &a, "  Reading  ").unwrap();
        assert_eq!(g.name, "Reading");
        rename(&c, &g.id, "Docs").unwrap();
        set_collapsed(&c, &g.id, true).unwrap();
        let g = get(&c, &g.id).unwrap();
        assert_eq!((g.name.as_str(), g.collapsed), ("Docs", true));
        for bad in ["", "   ", "tab\there", &"n".repeat(65)] {
            assert!(
                matches!(rename(&c, &g.id, bad), Err(TabError::InvalidName(_))),
                "{bad:?}"
            );
            assert!(
                matches!(create_for(&c, &a, bad), Err(TabError::InvalidName(_))),
                "{bad:?}"
            );
        }
        let ghost = Id::new("grp");
        assert!(matches!(
            rename(&c, &ghost, "x"),
            Err(TabError::NoSuchGroup(_))
        ));
        assert!(matches!(
            set_collapsed(&c, &ghost, true),
            Err(TabError::NoSuchGroup(_))
        ));
        assert!(matches!(
            assign(&c, &a, Some(&ghost)),
            Err(TabError::NoSuchGroup(_))
        ));
        assert!(matches!(
            tab::create(&c, "https://x.example/", None, Some(&ghost)),
            Err(TabError::NoSuchGroup(_))
        ));
        assert!(matches!(
            assign(&c, &Id::new("tab"), None),
            Err(TabError::NotFound(_))
        ));
        assert!(matches!(
            create_for(&c, &Id::new("tab"), "x"),
            Err(TabError::NotFound(_))
        ));
        assert_eq!(list(&c).unwrap().len(), 1, "no refusal changed anything");
    }
}
