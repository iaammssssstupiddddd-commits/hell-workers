//! Auto-haul systems for automatic resource transportation
//!
//! This module provides automatic resource transportation systems:
//! - `task_area_auto_haul_system` - Auto-haul items to stockpiles within task area
//! - `blueprint_auto_haul_system` - Auto-haul materials to blueprints
//! - `bucket_auto_haul_system` - Auto-haul buckets back to bucket storage
//! - `tank_water_request_system` - Request water gathering when tank is low

mod blueprint;
mod bucket;
mod mixer;
mod tank_water_request;
mod task_area;

use bevy::prelude::*;
use std::collections::HashSet;

/// 同フレーム内の競合回避用: タスク発行済みアイテム
#[derive(Resource, Default)]
pub struct ItemReservations(pub HashSet<Entity>);

pub use blueprint::blueprint_auto_haul_system;
pub use bucket::bucket_auto_haul_system;
pub use mixer::mud_mixer_auto_haul_system;
pub use tank_water_request::tank_water_request_system;
pub use task_area::task_area_auto_haul_system;
