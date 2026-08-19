//! DiskScope GUI binary entrypoint.
//!
//! Kept intentionally thin: all command wiring and event handling lives
//! in the `gui` library crate so it can be tested without launching a
//! window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    gui::run();
}
