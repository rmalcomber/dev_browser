//! CEF integration for The Dev Browser. Empty until M1.1 (see planning/m1-plan.md): this is the only crate that will touch CEF.
//! All browser logic belongs in `dev_browser_core`; this crate only executes what core decides.

/// The version of the host crate, shown by `dev_browser --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
