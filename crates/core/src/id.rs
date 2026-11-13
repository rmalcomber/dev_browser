//! Identifiers. A workspace id becomes a folder name, so an id must be impossible to turn into a path trick:
//! lowercase letters, digits and hyphens only, a short fixed shape, and nothing the caller invents is accepted without `parse`.
//!
//! Ids are random, not sequential, so one workspace's id says nothing about another's. Randomness comes from the standard library's
//! per-process hash seeding (which the operating system seeds), so no random-number crate is needed.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// The longest id `parse` accepts: a kind of up to 8 letters, a hyphen and 16 hex digits fit well inside it.
pub const MAX_LEN: usize = 32;

/// A validated identifier such as `ws-3fa9c2d41b7e0a55` or `tab-9d01...`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(String);

#[derive(Debug, PartialEq, Eq)]
pub enum IdError {
    Empty,
    TooLong,
    BadCharacter(char),
    BadShape,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Empty => write!(f, "id is empty"),
            IdError::TooLong => write!(f, "id is longer than {MAX_LEN} characters"),
            IdError::BadCharacter(c) => write!(
                f,
                "id contains {c:?}; only lowercase letters, digits and hyphens are allowed"
            ),
            IdError::BadShape => write!(f, "id must look like kind-hexdigits"),
        }
    }
}

impl std::error::Error for IdError {}

impl Id {
    /// A fresh random id of the given kind (`"ws"`, `"tab"`, `"grp"`). The kind must be 1 to 8 lowercase letters.
    pub fn new(kind: &str) -> Id {
        assert!(
            !kind.is_empty() && kind.len() <= 8 && kind.bytes().all(|b| b.is_ascii_lowercase()),
            "id kind must be 1 to 8 lowercase letters, got {kind:?}"
        );
        let mut h = RandomState::new().build_hasher();
        h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        h.write_u128(n);
        Id(format!("{kind}-{:016x}", h.finish()))
    }

    /// Accept an id from outside (the database, the shell). Everything that becomes a path or a query goes through this.
    pub fn parse(s: &str) -> Result<Id, IdError> {
        if s.is_empty() {
            return Err(IdError::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(IdError::TooLong);
        }
        if let Some(c) = s
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-'))
        {
            return Err(IdError::BadCharacter(c));
        }
        let (kind, hex) = s.split_once('-').ok_or(IdError::BadShape)?;
        let ok = !kind.is_empty()
            && kind.len() <= 8
            && kind.bytes().all(|b| b.is_ascii_lowercase())
            && !hex.is_empty()
            && hex
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
        if ok {
            Ok(Id(s.to_string()))
        } else {
            Err(IdError::BadShape)
        }
    }

    /// The part before the first hyphen: `ws`, `tab`, `grp`.
    pub fn kind(&self) -> &str {
        self.0.split_once('-').map(|(k, _)| k).unwrap_or("")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn new_ids_parse_back_and_are_unique() {
        let mut seen = HashSet::new();
        for _ in 0..10_000 {
            let id = Id::new("ws");
            assert_eq!(Id::parse(id.as_str()).unwrap(), id);
            assert!(seen.insert(id), "duplicate id");
        }
    }

    #[test]
    fn ids_are_safe_as_folder_names() {
        let id = Id::new("tab");
        assert!(id
            .as_str()
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'));
        assert!(!id.as_str().starts_with('-') && !id.as_str().contains(".."));
    }

    #[test]
    fn parse_rejects_path_tricks_and_bad_shapes() {
        for bad in [
            "",
            "../x",
            "ws-../x",
            "ws/1",
            "ws\\1",
            "WS-1a",
            "ws-1A",
            "ws",
            "-1a",
            "ws-",
            "ws-zz",
            "ws-1 a",
            "ws-1\u{e9}",
        ] {
            assert!(Id::parse(bad).is_err(), "{bad:?} should be rejected");
        }
        assert_eq!(Id::parse(&"a".repeat(33)), Err(IdError::TooLong));
        assert_eq!(Id::parse(""), Err(IdError::Empty));
        assert!(matches!(
            Id::parse("ws-1/"),
            Err(IdError::BadCharacter('/'))
        ));
    }

    #[test]
    #[should_panic]
    fn kind_must_be_lowercase_letters() {
        let _ = Id::new("Ws");
    }
}
