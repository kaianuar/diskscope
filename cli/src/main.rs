//! DiskScope CLI binary.
//!
//! Placeholder for Phase 1. The real clap-based implementation lands
//! in Phase 3. This binary exists only so the workspace compiles
//! end-to-end and `cargo check --workspace` passes.

use domain::FileType;

fn main() {
    let _ = FileType::Other;
    println!("diskscope — not yet implemented (Phase 3)");
}
