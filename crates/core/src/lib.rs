//! Domain and storage for The Dev Browser. No CEF: everything here is unit-testable without a browser.
//! Design: planning/m1-plan.md. A workspace is a profile; tabs belong to exactly one workspace.

pub mod address;
pub mod backend;
pub mod commands;
pub mod data;
pub mod downloads;
pub mod errorpage;
pub mod group;
pub mod id;
pub mod ipc;
pub mod migrate;
pub mod nav;
pub mod schema;
pub mod search;
pub mod settings;
pub mod tab;
pub mod url;
pub mod workspace;

#[cfg(test)]
mod testutil;
