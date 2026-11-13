//! Downloads, minimal (M1.6): the user decides to save or cancel; the app picks the folder and a safe file name. A page's suggested name is
//! untrusted: it may carry path separators, reserved Windows device names, control characters or be absurdly long.

const MAX_NAME_CHARS: usize = 100;
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A file name that is safe to create in the downloads folder: no path, no reserved or special characters, never empty.
pub fn safe_name(suggested: &str) -> String {
    // only the last path component, however the page spelled the separators
    let last = suggested.rsplit(['/', '\\']).next().unwrap_or("");
    let mut name: String = last
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(MAX_NAME_CHARS)
        .collect();
    name = name
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string();
    let stem = name.split('.').next().unwrap_or("").to_uppercase();
    if name.is_empty() || RESERVED.contains(&stem.as_str()) {
        return "download".to_string();
    }
    name
}

/// `name`, or `name (2).ext`, `name (3).ext` ... the first that `exists` says is free.
pub fn unique_name(name: &str, exists: impl Fn(&str) -> bool) -> String {
    if !exists(name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..1000)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|c| !exists(c))
        .unwrap_or_else(|| format!("{stem} (copy){ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_special_characters_are_removed() {
        assert_eq!(safe_name("..\\..\\Windows\\evil.exe"), "evil.exe");
        assert_eq!(safe_name("/etc/passwd"), "passwd");
        assert_eq!(safe_name("a<b>:c|d?.txt"), "a_b__c_d_.txt");
        assert_eq!(safe_name("x\u{0}y\n.txt"), "x_y_.txt");
    }

    #[test]
    fn reserved_empty_and_dot_names_become_download() {
        for n in ["", ".", "..", "  ", "CON", "nul.txt", "com1.tar.gz", "dir/"] {
            assert_eq!(safe_name(n), "download", "{n:?}");
        }
        assert_eq!(safe_name("console.txt"), "console.txt");
    }

    #[test]
    fn long_names_are_cut() {
        assert!(safe_name(&"a".repeat(500)).chars().count() <= MAX_NAME_CHARS);
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let taken = ["f.txt", "f (2).txt"];
        assert_eq!(unique_name("f.txt", |n| taken.contains(&n)), "f (3).txt");
        assert_eq!(unique_name("g", |_| false), "g");
        assert_eq!(unique_name("noext", |n| n == "noext"), "noext (2)");
    }
}
