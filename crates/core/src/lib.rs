//! ftc_core: every rule of Food Truck Circus.
//!
//! Builder requirement: this crate performs no input or output. It takes
//! values and returns values, so `cargo test` reaches every rule without a
//! browser. `tests/boundary.rs` enforces that, along with the dependency
//! allowlist and the reference ban on floats, hash maps and clocks.
//!
//! Two lints are off: a body's particles, masses and radii are parallel
//! arrays read by one index, which an iterator would hide; and a mission's
//! `match` on (goal, signal) reads better with its test inside the arm.
#![allow(clippy::needless_range_loop, clippy::collapsible_match)]

pub mod acts;
pub mod balance;
pub mod body;
pub mod crowd;
pub mod fx;
pub mod game;
pub mod geom;
pub mod mesh;
pub mod mission;
pub mod performer;
pub mod render;
pub mod rng;
pub mod scene;
pub mod stage;
pub mod tour;
pub mod trick;
