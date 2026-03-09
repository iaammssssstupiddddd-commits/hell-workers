// System functions imported from hw_logistics
pub use hw_logistics::transport_request::plugin::{TransportRequestPlugin, TransportRequestSet};

use super::producer::{
    floor_construction::{
        floor_construction_auto_haul_system, floor_material_delivery_sync_system,
        floor_tile_designation_system,
    },
    wall_construction::{
        wall_construction_auto_haul_system, wall_material_delivery_sync_system,
        wall_tile_designation_system,
    },
};
use crate::systems::GameSystemSet;
use bevy::prelude::*;

/// Optional M_extra が完了するまで root に残る floor/wall construction producer の追加プラグイン
pub struct FloorWallTransportPlugin;

impl Plugin for FloorWallTransportPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                floor_construction_auto_haul_system,
                floor_material_delivery_sync_system.after(floor_construction_auto_haul_system),
                floor_tile_designation_system.after(floor_material_delivery_sync_system),
                wall_construction_auto_haul_system,
                wall_material_delivery_sync_system.after(wall_construction_auto_haul_system),
                wall_tile_designation_system.after(wall_material_delivery_sync_system),
            )
                .in_set(TransportRequestSet::Decide)
                .in_set(GameSystemSet::Logic),
        );
    }
}
