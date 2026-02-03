//! Auto-haul systems for automatic resource transportation
//!
//! This module provides automatic resource transportation systems:
//! - `task_area_auto_haul_system` - Auto-haul items to stockpiles within task area
//! - `blueprint_auto_haul_system` - Auto-haul materials to blueprints
//! - `bucket_auto_haul_system` - Auto-haul buckets back to bucket storage
//! - `tank_water_request_system` - Request water gathering when tank is low

mod task_area;
mod blueprint;
mod bucket;
mod tank_water_request;
mod mixer;

use bevy::prelude::*;
use std::collections::HashSet;

/// MudMixerの水運搬で予約されたバケツ（同フレーム内の競合回避用）
#[derive(Resource, Default)]
pub struct MixerWaterBucketReservations(pub HashSet<Entity>);

pub use task_area::task_area_auto_haul_system;
pub use blueprint::blueprint_auto_haul_system;
pub use bucket::bucket_auto_haul_system;
pub use tank_water_request::tank_water_request_system;
pub use mixer::mud_mixer_auto_haul_system;
