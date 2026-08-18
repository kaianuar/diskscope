//! DiskScope GUI binary.
//!
//! Placeholder for Phase 1. The real Tauri + React + egui implementation
//! lands in Phase 4. This binary exists only so the workspace compiles
//! end-to-end and `cargo check --workspace` passes.

use domain::FileType;

fn main() {
    // Touch the domain to ensure the dependency is wired and the link
    // graph stays healthy even before the real GUI is built.
    let _ = FileType::Other;
    println!("diskscope-gui — not yet implemented (Phase 4)");
}
