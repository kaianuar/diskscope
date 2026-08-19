//! DiskScope scan engine adapter.
//!
//! Placeholder for Phase 1. The concrete implementations of the
//! [`domain::ports::Scanner`], [`domain::ports::Trash`], and
//! [`domain::ports::Cache`] ports — plus filters, sort, and output
//! formats — land in Phase 2. This crate exists so the workspace
//! compiles end-to-end and `cargo check --workspace` /
//! `cargo test --workspace` pass with the three placeholder crates
//! in place.

#![deny(missing_docs)]
#![deny(clippy::all)]
#![forbid(unsafe_code)]

/// Library entry point. Touches the domain to confirm the dep is wired
/// and the link graph stays healthy before Phase 2 lands.
pub fn lib() {
    let _ = domain::FileType::Other;
}