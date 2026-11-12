//! Domain and storage for The Dev Browser. No CEF: everything here is unit-testable without a browser.
//! Design: planning/m1-plan.md. A workspace is a profile; tabs belong to exactly one workspace.

pub mod data;
pub mod id;
pub mod migrate;
pub mod schema;
pub mod workspace;
