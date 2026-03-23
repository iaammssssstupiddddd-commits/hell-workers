pub mod exhausted_gathering;
pub mod motion_dispatch;
pub mod rest_area;
pub mod rest_decision;
mod system;
pub mod task_override;
pub mod transitions;

use hw_core::constants::{GATHERING_ARRIVAL_RADIUS_BASE, TILE_SIZE};

pub(crate) const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE;

pub use rest_area::{
    find_nearest_available_rest_area, has_arrived_at_rest_area,
    nearest_walkable_adjacent_to_rest_area, rest_area_has_capacity,
};
pub use system::idle_behavior_decision_system;
