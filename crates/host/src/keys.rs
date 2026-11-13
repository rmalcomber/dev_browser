//! From a CEF key event to a registry chord. Pure functions, so the mapping is unit-tested without a browser.
//!
//! Matching is exact on the modifiers: `Ctrl+Alt+<key>` is how Windows reports AltGr on many keyboard layouts, so a chord without Ctrl
//! never matches an AltGr character, and no registered chord uses Ctrl and Alt together (a test in `core` would not catch that, so the
//! registry avoids it by rule; see `decisions.md`).

use dev_browser_core::commands::{Chord, Key};

pub const SHIFT_DOWN: u32 = 1 << 1;
pub const CONTROL_DOWN: u32 = 1 << 2;
pub const ALT_DOWN: u32 = 1 << 3;

/// A Windows virtual-key code as a registry key. Only the keys the registry can name; digits on the main row (not the numpad).
pub fn key_from_vk(vk: i32) -> Option<Key> {
    match vk {
        0x09 => Some(Key::Tab),
        0x0D => Some(Key::Enter),
        0x1B => Some(Key::Escape),
        0x21 => Some(Key::PageUp),
        0x22 => Some(Key::PageDown),
        0x30..=0x39 | 0x41..=0x5A => Some(Key::Char((vk as u8 as char).to_ascii_lowercase())),
        0x70..=0x7B => Some(Key::F((vk - 0x6F) as u8)),
        _ => None,
    }
}

/// The chord a key press forms, if the key is one the registry knows.
pub fn chord_from_event(vk: i32, modifiers: u32) -> Option<Chord> {
    Some(Chord::new(
        modifiers & CONTROL_DOWN != 0,
        modifiers & SHIFT_DOWN != 0,
        modifiers & ALT_DOWN != 0,
        key_from_vk(vk)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_the_registry() {
        assert_eq!(key_from_vk(0x54), Some(Key::Char('t')));
        assert_eq!(key_from_vk(0x31), Some(Key::Char('1')));
        assert_eq!(key_from_vk(0x39), Some(Key::Char('9')));
        assert_eq!(key_from_vk(0x09), Some(Key::Tab));
        assert_eq!(key_from_vk(0x21), Some(Key::PageUp));
        assert_eq!(key_from_vk(0x22), Some(Key::PageDown));
        assert_eq!(key_from_vk(0x1B), Some(Key::Escape));
        assert_eq!(key_from_vk(0x75), Some(Key::F(6)));
        assert_eq!(key_from_vk(0x70), Some(Key::F(1)));
        assert_eq!(key_from_vk(0x7B), Some(Key::F(12)));
        for unmapped in [0x00, 0x08, 0x20, 0x25, 0x60, 0x7C, 0xBA, 0xDB] {
            assert_eq!(key_from_vk(unmapped), None, "{unmapped:#x}");
        }
    }

    #[test]
    fn modifiers_are_exact() {
        let tab = chord_from_event(0x09, CONTROL_DOWN | SHIFT_DOWN).unwrap();
        assert_eq!(tab.display(), "Ctrl+Shift+Tab");
        assert_eq!(chord_from_event(0x31, ALT_DOWN).unwrap().display(), "Alt+1");
        assert_eq!(chord_from_event(0x75, 0).unwrap().display(), "F6");
        // AltGr arrives as Ctrl+Alt: that must not look like Alt+1 or Ctrl+1
        let altgr = chord_from_event(0x31, CONTROL_DOWN | ALT_DOWN).unwrap();
        assert_ne!(altgr, chord_from_event(0x31, ALT_DOWN).unwrap());
        assert_ne!(altgr, chord_from_event(0x31, CONTROL_DOWN).unwrap());
        assert_eq!(chord_from_event(0xBA, CONTROL_DOWN), None);
    }

    #[test]
    fn real_chords_find_their_commands() {
        use dev_browser_core::commands::{lookup, Source};
        let find = |vk, m, src| lookup(&chord_from_event(vk, m).unwrap(), src).map(|c| c.id);
        assert_eq!(find(0x54, CONTROL_DOWN, Source::Page), Some("tab.new"));
        assert_eq!(find(0x57, CONTROL_DOWN, Source::Page), Some("tab.close"));
        assert_eq!(find(0x09, CONTROL_DOWN, Source::Page), Some("tab.next"));
        assert_eq!(
            find(0x09, CONTROL_DOWN | SHIFT_DOWN, Source::Shell),
            Some("tab.prev")
        );
        assert_eq!(find(0x33, ALT_DOWN, Source::Page), Some("tab.select.3"));
        assert_eq!(find(0x75, 0, Source::Page), Some("focus.cycle"));
        assert_eq!(find(0x1B, 0, Source::Shell), Some("focus.page"));
        assert_eq!(find(0x1B, 0, Source::Page), None, "Esc stays with the page");
        // typing: a plain letter, and Shift+letter, never match
        assert_eq!(find(0x54, 0, Source::Page), None);
        assert_eq!(find(0x54, SHIFT_DOWN, Source::Page), None);
        // Ctrl+P and Ctrl+1 are promised to later milestones and must not match anything yet
        assert_eq!(find(0x50, CONTROL_DOWN, Source::Page), None);
        assert_eq!(find(0x31, CONTROL_DOWN, Source::Page), None);
    }
}
