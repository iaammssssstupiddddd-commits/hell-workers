//! パス探索と障害物脱出

use hw_core::constants::MAX_PATHFINDS_PER_FRAME;

mod fallback;
mod reuse;
mod system;

#[cfg(feature = "profiling")]
pub use system::RuntimePathDeferMetrics;
pub use system::{
    pathfinding_system, reset_runtime_path_search_budget_system, soul_stuck_escape_system,
};

/// Escape is evaluated before Actor pathfinding, so cap it before the Actor
/// task phase raises the cumulative ceiling to this value.
pub(crate) const ESCAPE_PATHFINDS_PER_FRAME: usize = 2;

/// Keep this many core A* slots available after the Actor task phase.
const RESERVED_IDLE_PATHFINDS_PER_FRAME: usize = 2;

/// Keep part of the ActiveTask class for Actor-side replans after Execute has
/// routed task handlers. Without this reservation, a full Execute lane could
/// starve movement-triggered task replans indefinitely.
const RESERVED_ACTOR_TASK_PATHFINDS_PER_FRAME: usize = 2;

/// Cumulative ceiling for Actor task pathfinding after the escape phase.
pub(crate) const TASK_PATHFINDS_PHASE_LIMIT: usize =
    MAX_PATHFINDS_PER_FRAME.saturating_sub(RESERVED_IDLE_PATHFINDS_PER_FRAME);

/// Cumulative ceiling used by task handlers in Execute. Actor-side task
/// replans raise it to [`TASK_PATHFINDS_PHASE_LIMIT`] later in the frame.
pub(crate) const TASK_EXECUTION_PATHFINDS_PHASE_LIMIT: usize =
    TASK_PATHFINDS_PHASE_LIMIT.saturating_sub(RESERVED_ACTOR_TASK_PATHFINDS_PER_FRAME);

#[derive(bevy::prelude::Component, Debug, Clone, Copy)]
pub struct PathCooldown {
    remaining_frames: u8,
}
