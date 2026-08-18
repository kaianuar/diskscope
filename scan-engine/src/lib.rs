//! DiskScope scan engine adapter.
//!
//! Placeholder for Phase 1. The real implementation (jwalk + rayon +
//! ignore + redb + trash) lands in Phase 2. This crate exists only so
//! `cargo check --workspace` passes during scaffolding.

/// Public no-op so the placeholder has a real symbol to point at.
///
/// Real functions (`Scanner::scan`, `Cache::get`, etc.) are added in
/// Phase 2 as the adapter grows to depend on `jwalk`, `rayon`, etc.
pub fn lib() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_compile_when_placeholder_loaded() {
        lib();
    }
}
