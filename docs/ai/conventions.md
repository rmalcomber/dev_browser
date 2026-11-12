# Conventions

Planning stage: these are the intended conventions. Confirm or change them when the M0 spike picks the toolchain.

- **Language/tooling versions:** Rust stable (pin in `rust-toolchain.toml` once code exists). Trusted shell UI in TypeScript. Docs: Node 24, Astro + Starlight.
- **Formatting/linting:** `cargo fmt`, `cargo clippy -- -D warnings` for Rust. Docs and shell UI formatting to be chosen with the shell.
- **Structure:** concrete structs first; introduce traits only at real boundaries (engine adapter, test doubles, plugin runtime). The "services" in the spec are logical Rust structs, not OS services.
- **Naming:** workspaces are keyed by generated stable IDs on disk, never by their mutable display name.
- **Errors and logging:** explicit, with context. No silent catches. Never log sensitive tool input, URLs or titles from another workspace's scope.
- **Tests:** unit tests beside the code; integration tests under `tests/`. The scope and privacy rules in `planning/security-acceptance-tests.md` are mandatory tests, written with the feature, not after.
- **Spikes:** disposable code lives in `spike/`, is labelled as such, and ends in a decision entry. It is never promoted to `src/` by renaming.
- **Planning vs docs:** `planning/` is working material; behaviour that is settled goes into `docs/src/content/docs/` in the same commit as the code.
- **Git:** branch `main`; small commits; imperative subject; explain why in the body. No AI attribution lines.
- **Dependencies:** standard library first; justify each addition in `decisions.md`. Chromium/CEF is the one large, deliberate exception (see decisions).
