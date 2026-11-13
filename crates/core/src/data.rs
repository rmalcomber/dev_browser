//! Where the data lives and how the databases are opened. Layout (spec section 6.1, decision D6):
//!
//! ```text
//! <root>/
//!   browser.db                  global: workspace identity, ordering, settings; nothing browsed
//!   chromium-root/<ws-id>/      CEF's root cache path; one DIRECT child per workspace (cookies, storage, cache)
//!   workspaces/<ws-id>/
//!     browser.db                that workspace's tabs, groups and state
//!     homescreen.db             created in M2
//! ```
//!
//! Two rules from the spike are enforced here so the host cannot get them wrong:
//! - a workspace's Chromium folder must be a direct child of `chromium-root` (CEF refuses a deeper or looser path and silently falls back
//!   to an in-memory profile), and
//! - every folder name comes from a validated `Id`, never from a name or from the shell.

use crate::{id::Id, migrate, schema, workspace};
use rusqlite::Connection;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum DataError {
    Io(std::io::Error),
    Migrate(migrate::MigrateError),
    Workspace(workspace::WorkspaceError),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io(e) => write!(f, "file error: {e}"),
            DataError::Migrate(e) => write!(f, "{e}"),
            DataError::Workspace(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DataError {}
impl From<std::io::Error> for DataError {
    fn from(e: std::io::Error) -> Self {
        DataError::Io(e)
    }
}
impl From<migrate::MigrateError> for DataError {
    fn from(e: migrate::MigrateError) -> Self {
        DataError::Migrate(e)
    }
}
impl From<workspace::WorkspaceError> for DataError {
    fn from(e: workspace::WorkspaceError) -> Self {
        DataError::Workspace(e)
    }
}
impl From<rusqlite::Error> for DataError {
    fn from(e: rusqlite::Error) -> Self {
        DataError::Migrate(migrate::MigrateError::Sqlite(e))
    }
}

/// The application's data directory. Cheap to clone; holds only a path.
#[derive(Clone, Debug)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// Use `root` as the data directory, creating it and `chromium-root` and `workspaces` if needed.
    pub fn open(root: impl Into<PathBuf>) -> Result<DataDir, DataError> {
        let d = DataDir { root: root.into() };
        std::fs::create_dir_all(d.chromium_root())?;
        std::fs::create_dir_all(d.workspaces_root())?;
        Ok(d)
    }

    /// A `DataDir` that has not touched the disk. For processes that must not create folders (CEF child processes).
    pub fn unopened(root: impl Into<PathBuf>) -> DataDir {
        DataDir { root: root.into() }
    }

    /// The default location on this machine: `%LOCALAPPDATA%\dev_browser` on Windows (decision D6). `None` if it cannot be worked out.
    pub fn default_location() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA").map(|p| Path::new(&p).join("dev_browser"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn global_db_path(&self) -> PathBuf {
        self.root.join("browser.db")
    }
    /// CEF's `root_cache_path`.
    pub fn chromium_root(&self) -> PathBuf {
        self.root.join("chromium-root")
    }
    /// The workspace's Chromium profile folder: always a direct child of `chromium_root()`.
    pub fn chromium_profile(&self, id: &Id) -> PathBuf {
        self.chromium_root().join(id.as_str())
    }
    fn workspaces_root(&self) -> PathBuf {
        self.root.join("workspaces")
    }
    pub fn workspace_dir(&self, id: &Id) -> PathBuf {
        self.workspaces_root().join(id.as_str())
    }
    pub fn homescreen_db_path(&self, id: &Id) -> PathBuf {
        self.workspace_dir(id).join("homescreen.db")
    }
    pub fn workspace_db_path(&self, id: &Id) -> PathBuf {
        self.workspace_dir(id).join("browser.db")
    }

    /// Open (creating and migrating) the global database.
    pub fn open_global(&self) -> Result<Connection, DataError> {
        let mut c = Connection::open(self.global_db_path())?;
        c.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        migrate::migrate(&mut c, schema::GLOBAL)?;
        Ok(c)
    }

    /// Open (creating and migrating) one workspace's database, and make sure its folders exist.
    pub fn open_workspace(&self, id: &Id) -> Result<Connection, DataError> {
        std::fs::create_dir_all(self.workspace_dir(id))?;
        std::fs::create_dir_all(self.chromium_profile(id))?;
        let mut c = Connection::open(self.workspace_db_path(id))?;
        c.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        migrate::migrate(&mut c, schema::WORKSPACE)?;
        Ok(c)
    }

    /// Open (creating and migrating) one workspace's Home Screen database.
    pub fn open_homescreen(&self, id: &Id) -> Result<Connection, DataError> {
        std::fs::create_dir_all(self.workspace_dir(id))?;
        let mut c = Connection::open(self.homescreen_db_path(id))?;
        c.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        migrate::migrate(&mut c, schema::HOMESCREEN)?;
        Ok(c)
    }

    /// Create a workspace: its record, its database and its folders. If anything after the record fails, the record is removed again so
    /// there is no half-made workspace in the list (there are no transactions across a database and the file system, so this is by hand).
    pub fn create_workspace(
        &self,
        global: &mut Connection,
        name: &str,
        colour: u32,
    ) -> Result<workspace::Workspace, DataError> {
        let ws = workspace::create(global, name, colour)?;
        if let Err(e) = self.open_workspace(&ws.id) {
            let _ = workspace::delete_record(global, &ws.id);
            let _ = self.remove_workspace_folders(&ws.id);
            return Err(e);
        }
        Ok(ws)
    }

    /// Delete a workspace completely: the record first (so it disappears from the list even if a folder is locked), then its folders.
    /// A folder that cannot be removed is reported, not ignored, and the next start can retry it with `remove_orphans`.
    pub fn delete_workspace(&self, global: &mut Connection, id: &Id) -> Result<(), DataError> {
        workspace::delete_record(global, id)?;
        self.remove_workspace_folders(id)?;
        Ok(())
    }

    /// Remove a workspace's own folder and its Chromium profile. Missing folders are fine; a locked one is an error (retry later, or
    /// `remove_orphans` at the next start). Does not touch the record.
    pub fn remove_workspace_folders(&self, id: &Id) -> Result<(), DataError> {
        for dir in [self.workspace_dir(id), self.chromium_profile(id)] {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(DataError::Io(e)),
            }
        }
        Ok(())
    }

    /// Remove workspace folders that have no record (left behind by a crash or a locked folder). Returns the ids removed.
    /// Folder names that are not valid ids are left alone and reported by the caller, never deleted on a guess.
    pub fn remove_orphans(&self, global: &Connection) -> Result<Vec<Id>, DataError> {
        let known: std::collections::HashSet<Id> =
            workspace::list(global)?.into_iter().map(|w| w.id).collect();
        let mut removed = Vec::new();
        let mut first_error = None;
        for base in [self.workspaces_root(), self.chromium_root()] {
            for entry in std::fs::read_dir(&base)? {
                let entry = entry?;
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                let Ok(id) = Id::parse(&name) else { continue };
                // only folders that look exactly like ours (kind "ws"): CEF keeps its own folders in chromium-root
                if id.kind() != "ws" {
                    continue;
                }
                if !known.contains(&id) && entry.path().is_dir() {
                    // a locked folder must not stop the others being removed; report the first failure at the end
                    if let Err(e) = std::fs::remove_dir_all(entry.path()) {
                        first_error.get_or_insert(e);
                        continue;
                    }
                    if !removed.contains(&id) {
                        removed.push(id);
                    }
                }
            }
        }
        match first_error {
            Some(e) => Err(DataError::Io(e)),
            None => Ok(removed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throw-away directory under the system temp folder, removed when dropped. (No tempfile crate: the standard library is enough.)
    struct Temp(PathBuf);
    impl Temp {
        fn new(tag: &str) -> Temp {
            let p = std::env::temp_dir()
                .join(format!("dev_browser_test_{tag}_{}", Id::new("t").as_str()));
            Temp(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_workspace_gets_its_own_database_and_a_direct_child_profile() {
        let t = Temp::new("layout");
        let d = DataDir::open(&t.0).unwrap();
        let mut g = d.open_global().unwrap();
        let ws = d.create_workspace(&mut g, "Work", 0x2C5FA8).unwrap();
        assert!(d.workspace_db_path(&ws.id).is_file());
        assert!(d.chromium_profile(&ws.id).is_dir());
        assert_eq!(
            d.chromium_profile(&ws.id).parent().unwrap(),
            d.chromium_root(),
            "CEF needs a direct child of the root cache path"
        );
        assert!(d.workspace_dir(&ws.id).starts_with(d.root()));
    }

    #[test]
    fn workspaces_have_separate_databases() {
        let t = Temp::new("separate");
        let d = DataDir::open(&t.0).unwrap();
        let mut g = d.open_global().unwrap();
        let a = d.create_workspace(&mut g, "A", 1).unwrap();
        let b = d.create_workspace(&mut g, "B", 2).unwrap();
        let ca = d.open_workspace(&a.id).unwrap();
        ca.execute("INSERT INTO tabs (id, url, position, created_at) VALUES ('tab-1', 'https://a.example/', 0, 0)", []).unwrap();
        let cb = d.open_workspace(&b.id).unwrap();
        let n: i64 = cb
            .query_row("SELECT COUNT(*) FROM tabs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "a tab in A must not be visible in B");
        assert_ne!(d.workspace_db_path(&a.id), d.workspace_db_path(&b.id));
    }

    #[test]
    fn data_survives_reopening() {
        let t = Temp::new("reopen");
        let d = DataDir::open(&t.0).unwrap();
        let ws = {
            let mut g = d.open_global().unwrap();
            let ws = d.create_workspace(&mut g, "Work", 7).unwrap();
            d.open_workspace(&ws.id)
                .unwrap()
                .execute(
                    "INSERT INTO workspace_state (key, value) VALUES ('last_tab_id', 'tab-1')",
                    [],
                )
                .unwrap();
            ws
        };
        let g = d.open_global().unwrap();
        assert_eq!(workspace::list(&g).unwrap(), vec![ws.clone()]);
        let v: String = d
            .open_workspace(&ws.id)
            .unwrap()
            .query_row(
                "SELECT value FROM workspace_state WHERE key = 'last_tab_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "tab-1");
    }

    #[test]
    fn deleting_a_workspace_removes_the_record_and_both_folders() {
        let t = Temp::new("delete");
        let d = DataDir::open(&t.0).unwrap();
        let mut g = d.open_global().unwrap();
        let keep = d.create_workspace(&mut g, "Keep", 1).unwrap();
        let gone = d.create_workspace(&mut g, "Gone", 2).unwrap();
        d.delete_workspace(&mut g, &gone.id).unwrap();
        assert!(!d.workspace_dir(&gone.id).exists() && !d.chromium_profile(&gone.id).exists());
        assert!(
            d.workspace_dir(&keep.id).is_dir() && d.chromium_profile(&keep.id).is_dir(),
            "other workspaces are untouched"
        );
        assert_eq!(workspace::list(&g).unwrap(), vec![keep]);
    }

    #[test]
    fn orphans_are_removed_but_unknown_folder_names_are_left_alone() {
        let t = Temp::new("orphans");
        let d = DataDir::open(&t.0).unwrap();
        let mut g = d.open_global().unwrap();
        let keep = d.create_workspace(&mut g, "Keep", 1).unwrap();
        let orphan = Id::new("ws");
        std::fs::create_dir_all(d.workspace_dir(&orphan)).unwrap();
        std::fs::create_dir_all(d.chromium_profile(&orphan)).unwrap();
        let stranger = d.chromium_root().join("Default"); // not a valid id: must never be deleted on a guess
        std::fs::create_dir_all(&stranger).unwrap();
        let removed = d.remove_orphans(&g).unwrap();
        assert_eq!(removed, vec![orphan.clone()]);
        assert!(!d.workspace_dir(&orphan).exists() && !d.chromium_profile(&orphan).exists());
        assert!(
            stranger.is_dir(),
            "a folder that is not a workspace id is left alone"
        );
        assert!(d.workspace_dir(&keep.id).is_dir());
    }

    #[test]
    fn default_location_is_under_localappdata() {
        if let Some(p) = DataDir::default_location() {
            assert!(p.ends_with("dev_browser"));
        }
    }
}
