//! DiskScope GUI binary entrypoint.
//!
//! The real Tauri implementation lives in `src-tauri/` (see
//! `gui/src-tauri/src/lib.rs`); this stub keeps the committed binary
//! target compiling when the Tauri stack is not built. The workspace
//! also builds the full Tauri app via `gui/src-tauri/src/main.rs`.

use domain::FileType;

fn main() {
    // Touch the domain to ensure the dependency is wired and the link
    // graph stays healthy even before the real GUI is built.
    let _ = FileType::Other;
    println!("diskscope-gui — see gui/src-tauri for the Tauri app");
}
