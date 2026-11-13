//! The command registry: every action the user can take is a named command with default shortcuts (keyboard-first, decided
//! 2026-11-13; planning/m1-plan.md section 8a). The shell's buttons, the window-level shortcut handler and (from M2) the launcher all run
//! commands by id through `Backend::run_command`; no action exists only as a mouse click.
//!
//! Shortcuts are matched by the host in the pre-key-event handler of **every** view, so none depends on which view has focus, and a
//! matched chord is swallowed: a page never sees it. That makes every chord here *reserved* by the browser, as in other browsers.
//! `docs/src/content/docs/reference/shortcuts.md` is the user-facing list; a test below fails if it drifts from this table.

use std::sync::OnceLock;

/// A key, independent of the operating system's key codes (the host maps those).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A letter (lowercase), a digit, or `.`.
    Char(char),
    Tab,
    Left,
    Right,
    PageUp,
    PageDown,
    Escape,
    Enter,
    /// F1 to F12.
    F(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: Key,
}

impl Chord {
    pub fn new(ctrl: bool, shift: bool, alt: bool, key: Key) -> Chord {
        Chord {
            ctrl,
            shift,
            alt,
            key,
        }
    }

    /// Parse text such as `ctrl+shift+tab`, `alt+1`, `f6`, `esc`. Case does not matter.
    pub fn parse(text: &str) -> Result<Chord, String> {
        let lower = text.to_ascii_lowercase();
        let mut parts: Vec<&str> = lower.split('+').collect();
        let key_text = parts.pop().ok_or("empty chord")?;
        let key = match key_text {
            "tab" => Key::Tab,
            "pageup" => Key::PageUp,
            "pagedown" => Key::PageDown,
            "esc" | "escape" => Key::Escape,
            "enter" => Key::Enter,
            "left" => Key::Left,
            "right" => Key::Right,
            f if f.len() >= 2
                && f.starts_with('f')
                && f[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) =>
            {
                Key::F(f[1..].parse().unwrap())
            }
            c if c.chars().count() == 1
                && c.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.') =>
            {
                Key::Char(c.chars().next().unwrap())
            }
            other => return Err(format!("unknown key {other:?} in {text:?}")),
        };
        let mut chord = Chord::new(false, false, false, key);
        for m in parts {
            let flag = match m {
                "ctrl" => &mut chord.ctrl,
                "shift" => &mut chord.shift,
                "alt" => &mut chord.alt,
                other => return Err(format!("unknown modifier {other:?} in {text:?}")),
            };
            if *flag {
                return Err(format!("repeated modifier in {text:?}"));
            }
            *flag = true;
        }
        Ok(chord)
    }

    /// The form shown to people and written in the docs: `Ctrl+Shift+PageDown`, `Alt+1`, `F6`, `Esc`.
    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        match self.key {
            Key::Char(c) => s.push(c.to_ascii_uppercase()),
            Key::Tab => s.push_str("Tab"),
            Key::PageUp => s.push_str("PageUp"),
            Key::PageDown => s.push_str("PageDown"),
            Key::Escape => s.push_str("Esc"),
            Key::Enter => s.push_str("Enter"),
            Key::Left => s.push_str("Left"),
            Key::Right => s.push_str("Right"),
            Key::F(n) => s.push_str(&format!("F{n}")),
        }
        s
    }
}

/// Where a key was pressed or a command was issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The trusted shell (sidebar, toolbar).
    Shell,
    /// A page.
    Page,
}

/// Where a chord is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// In the shell and in every page.
    Anywhere,
    /// Only while the focus is in the shell (the same key belongs to pages otherwise, e.g. Esc).
    ShellOnly,
}

pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    /// Default shortcuts, as `Chord::parse` reads them.
    pub chords: &'static [&'static str],
    pub scope: Scope,
}

macro_rules! cmd {
    ($id:expr, $label:expr, [$($chord:expr),*]) => {
        Command { id: $id, label: $label, chords: &[$($chord),*], scope: Scope::Anywhere }
    };
    ($id:expr, $label:expr, [$($chord:expr),*], shell_only) => {
        Command { id: $id, label: $label, chords: &[$($chord),*], scope: Scope::ShellOnly }
    };
}

/// Every command, in the order the docs list them. `Ctrl+1` and `Ctrl+2` are left free on purpose: the spec gives them to pane focus
/// in split view (M4).
pub const COMMANDS: &[Command] = &[
    cmd!("tab.new", "New tab", ["ctrl+t"]),
    cmd!("tab.close", "Close tab", ["ctrl+w"]),
    cmd!("tab.next", "Next tab", ["ctrl+tab"]),
    cmd!("tab.prev", "Previous tab", ["ctrl+shift+tab"]),
    cmd!("tab.select.1", "Go to tab 1", ["alt+1"]),
    cmd!("tab.select.2", "Go to tab 2", ["alt+2"]),
    cmd!("tab.select.3", "Go to tab 3", ["alt+3"]),
    cmd!("tab.select.4", "Go to tab 4", ["alt+4"]),
    cmd!("tab.select.5", "Go to tab 5", ["alt+5"]),
    cmd!("tab.select.6", "Go to tab 6", ["alt+6"]),
    cmd!("tab.select.7", "Go to tab 7", ["alt+7"]),
    cmd!("tab.select.8", "Go to tab 8", ["alt+8"]),
    cmd!("tab.select.last", "Go to the last tab", ["alt+9"]),
    cmd!("tab.move.down", "Move tab down", ["ctrl+shift+pagedown"]),
    cmd!("tab.move.up", "Move tab up", ["ctrl+shift+pageup"]),
    cmd!("tab.discard", "Unload background tabs", ["ctrl+shift+d"]),
    cmd!(
        "tab.group.new",
        "Put the tab in a new group",
        ["ctrl+shift+g"]
    ),
    cmd!(
        "tab.ungroup",
        "Take the tab out of its group",
        ["ctrl+shift+u"]
    ),
    cmd!("nav.back", "Back", ["alt+left"]),
    cmd!("nav.forward", "Forward", ["alt+right"]),
    cmd!("nav.reload", "Reload", ["f5", "ctrl+r"]),
    cmd!(
        "nav.reload.hard",
        "Reload, ignoring the cache",
        ["ctrl+shift+r"]
    ),
    cmd!("nav.stop", "Stop loading", ["ctrl+."]),
    cmd!("nav.address", "Go to the address bar", ["ctrl+l"]),
    cmd!("workspace.new", "New workspace", ["alt+n"]),
    cmd!("workspace.next", "Next workspace", ["alt+pagedown"]),
    cmd!("workspace.prev", "Previous workspace", ["alt+pageup"]),
    cmd!(
        "focus.cycle",
        "Move focus between the sidebar and the page",
        ["f6"]
    ),
    cmd!(
        "focus.page",
        "Return focus to the page",
        ["esc"],
        shell_only
    ),
];

pub fn find(id: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.id == id)
}

/// The parsed chord table, built once.
fn table() -> &'static Vec<(Chord, &'static Command)> {
    static TABLE: OnceLock<Vec<(Chord, &'static Command)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        COMMANDS
            .iter()
            .flat_map(|c| {
                c.chords.iter().map(move |text| {
                    (
                        Chord::parse(text).expect("registry chords are valid (checked by a test)"),
                        c,
                    )
                })
            })
            .collect()
    })
}

/// The command a key press runs, if any. `source` is where the key was pressed.
pub fn lookup(chord: &Chord, source: Source) -> Option<&'static Command> {
    table()
        .iter()
        .find(|(c, cmd)| c == chord && (cmd.scope == Scope::Anywhere || source == Source::Shell))
        .map(|(_, cmd)| *cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn parse_and_display_round_trip() {
        for (text, shown) in [
            ("ctrl+t", "Ctrl+T"),
            ("CTRL+SHIFT+TAB", "Ctrl+Shift+Tab"),
            ("alt+1", "Alt+1"),
            ("ctrl+alt+d", "Ctrl+Alt+D"),
            ("alt+pagedown", "Alt+PageDown"),
            ("f6", "F6"),
            ("esc", "Esc"),
            ("escape", "Esc"),
            ("enter", "Enter"),
            ("alt+left", "Alt+Left"),
            ("ALT+RIGHT", "Alt+Right"),
            ("ctrl+.", "Ctrl+."),
        ] {
            let c = Chord::parse(text).unwrap();
            assert_eq!(c.display(), shown, "{text}");
            assert_eq!(Chord::parse(&c.display()).unwrap(), c, "{shown}");
        }
    }

    #[test]
    fn bad_chords_are_errors() {
        for bad in [
            "",
            "ctrl+",
            "ctrl+ctrl+t",
            "meta+t",
            "ctrl+f13",
            "ctrl+f0",
            "ctrl+tt",
            "ctrl+é",
            "ctrl+-",
            "+t",
            "ctrl+..",
            "ctrl+up",
        ] {
            assert!(Chord::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn every_registered_chord_is_valid_and_ids_are_unique() {
        let mut ids = HashSet::new();
        for c in COMMANDS {
            assert!(ids.insert(c.id), "duplicate command id {}", c.id);
            assert!(!c.label.is_empty());
            for text in c.chords {
                Chord::parse(text).unwrap_or_else(|e| panic!("{}: {e}", c.id));
            }
        }
    }

    #[test]
    fn no_two_commands_share_a_chord() {
        // A shell-only chord collides with an anywhere chord too (it would shadow it in the shell), so any repeat is a failure.
        let mut seen: Vec<(Chord, &str)> = vec![];
        for c in COMMANDS {
            for text in c.chords {
                let chord = Chord::parse(text).unwrap();
                if let Some((_, other)) = seen.iter().find(|(s, _)| *s == chord) {
                    panic!("{} and {} both use {}", c.id, other, chord.display());
                }
                seen.push((chord, c.id));
            }
        }
    }

    #[test]
    fn no_chord_uses_ctrl_and_alt_together() {
        // Windows reports AltGr as Ctrl+Alt on many keyboard layouts, so such a chord would swallow typed characters
        for c in COMMANDS {
            for text in c.chords {
                let chord = Chord::parse(text).unwrap();
                assert!(
                    !(chord.ctrl && chord.alt),
                    "{} uses {}",
                    c.id,
                    chord.display()
                );
            }
        }
    }

    #[test]
    fn spec_reserved_chords_stay_free() {
        // Ctrl+P (launcher, M2), Ctrl+D (bookmark), Ctrl+\ (split), Ctrl+1/Ctrl+2 (pane focus, M4), Ctrl+Shift+P (print)
        for text in ["ctrl+p", "ctrl+d", "ctrl+1", "ctrl+2", "ctrl+shift+p"] {
            let chord = Chord::parse(text).unwrap();
            assert!(
                table().iter().all(|(c, _)| *c != chord),
                "{text} is promised to a later milestone"
            );
        }
    }

    #[test]
    fn lookup_respects_scope() {
        let esc = Chord::parse("esc").unwrap();
        assert_eq!(lookup(&esc, Source::Shell).unwrap().id, "focus.page");
        assert!(
            lookup(&esc, Source::Page).is_none(),
            "Esc in a page belongs to the page"
        );
        let t = Chord::parse("ctrl+t").unwrap();
        assert_eq!(lookup(&t, Source::Shell).unwrap().id, "tab.new");
        assert_eq!(lookup(&t, Source::Page).unwrap().id, "tab.new");
        assert!(lookup(&Chord::parse("ctrl+q").unwrap(), Source::Page).is_none());
    }

    #[test]
    fn the_docs_list_every_shortcut() {
        let docs = include_str!("../../../docs/src/content/docs/reference/shortcuts.md");
        for c in COMMANDS {
            for text in c.chords {
                let shown = Chord::parse(text).unwrap().display();
                assert!(
                    docs.contains(&format!("`{shown}`")),
                    "docs/.../reference/shortcuts.md must list `{shown}` ({})",
                    c.id
                );
            }
        }
    }
}
