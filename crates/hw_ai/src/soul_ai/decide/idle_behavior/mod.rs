pub mod exhausted_gathering;
pub mod motion_dispatch;
pub mod rest_area;
pub mod rest_decision;
pub mod task_override;
pub mod transitions;

use hw_core::constants::*;

pub(crate) const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE;
