//! パス探索と障害物脱出

mod fallback;
mod reuse;
mod system;

pub use system::{pathfinding_system, soul_stuck_escape_system};

#[derive(bevy::prelude::Component, Debug, Clone, Copy)]
pub struct PathCooldown {
    remaining_frames: u8,
}
