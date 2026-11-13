//! Test-only helpers.

use crate::id::Id;
use std::path::{Path, PathBuf};

/// A throw-away directory under the system temp folder, removed when dropped. (No tempfile crate: the standard library is enough.)
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        TempDir(
            std::env::temp_dir().join(format!("dev_browser_test_{tag}_{}", Id::new("t").as_str())),
        )
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
