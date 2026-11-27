//! Host-side tests for the shell's screen-space rectangle helper. The shell
//! itself is a `no_std` Ring-3 binary, so the pure geometry module is included
//! here directly, the same way `status_model.rs` includes `status.rs`.

extern crate libwoven;

#[path = "../src/geometry.rs"]
mod geometry;
