//! App-wide preferences in the global database's `settings` table. Only preferences that are not about browsing (no URLs, no titles):
//! the global database must stay free of browsing data (see `schema`).

use rusqlite::{params, Connection, OptionalExtension};

pub const ACTIVE_WORKSPACE: &str = "active_workspace";
pub const WINDOW: &str = "window";

/// The window's size and whether it was maximised, kept so the next start looks the same. Size only, never a position: a window
/// restored to a monitor that is gone would be unreachable. Whatever is stored is clamped on the way in and out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
}

impl WindowState {
    pub const DEFAULT: WindowState = WindowState {
        width: 1200,
        height: 720,
        maximized: false,
    };
    const MIN: (i32, i32) = (480, 320);
    const MAX: (i32, i32) = (16000, 16000);

    pub fn new(width: i32, height: i32, maximized: bool) -> WindowState {
        WindowState {
            width: width.clamp(Self::MIN.0, Self::MAX.0),
            height: height.clamp(Self::MIN.1, Self::MAX.1),
            maximized,
        }
    }

    /// `1200x720` or `1200x720,max`. Anything else is not a state.
    pub fn parse(text: &str) -> Option<WindowState> {
        let (size, rest) = match text.split_once(',') {
            Some((s, r)) => (s, Some(r)),
            None => (text, None),
        };
        let maximized = match rest {
            None => false,
            Some("max") => true,
            Some(_) => return None,
        };
        let (w, h) = size.split_once('x')?;
        Some(WindowState::new(
            w.parse().ok()?,
            h.parse().ok()?,
            maximized,
        ))
    }

    pub fn format(&self) -> String {
        format!(
            "{}x{}{}",
            self.width,
            self.height,
            if self.maximized { ",max" } else { "" }
        )
    }
}

pub fn window_state(conn: &Connection) -> WindowState {
    get(conn, WINDOW)
        .ok()
        .flatten()
        .and_then(|s| WindowState::parse(&s))
        .unwrap_or(WindowState::DEFAULT)
}

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
    fn window_state_round_trips_and_distrusts_what_it_reads() {
        let s = WindowState::new(1000, 600, true);
        assert_eq!(WindowState::parse(&s.format()), Some(s));
        assert_eq!(
            WindowState::parse("900x500"),
            Some(WindowState::new(900, 500, false))
        );
        // absurd sizes are clamped, junk is not a state
        assert_eq!(
            WindowState::parse("1x1").unwrap(),
            WindowState::new(480, 320, false)
        );
        assert_eq!(WindowState::parse("99999x99999").unwrap().width, 16000);
        for bad in [
            "",
            "x",
            "10x",
            "axb",
            "1000x600,",
            "1000x600,big",
            "1000x600x1",
        ] {
            assert_eq!(WindowState::parse(bad), None, "{bad:?}");
        }
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c, schema::GLOBAL).unwrap();
        assert_eq!(window_state(&c), WindowState::DEFAULT);
        set(&c, WINDOW, "garbage").unwrap();
        assert_eq!(window_state(&c), WindowState::DEFAULT);
        set(&c, WINDOW, &s.format()).unwrap();
        assert_eq!(window_state(&c), s);
    }

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
