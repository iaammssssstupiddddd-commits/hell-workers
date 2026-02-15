//! 空間グリッド関連のプラグイン

use crate::systems::GameSystemSet;
use crate::systems::spatial::{
    tick_spatial_grid_sync_timer_system, update_blueprint_spatial_grid_system,
    update_designation_spatial_grid_system, update_familiar_spatial_grid_system,
    update_floor_construction_spatial_grid_system, update_gathering_spot_spatial_grid_system,
    update_resource_spatial_grid_system, update_spatial_grid_system,
    update_stockpile_spatial_grid_system, update_transport_request_spatial_grid_system,
    SpatialGridSyncTimer,
};
use bevy::prelude::*;

pub struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpatialGridSyncTimer>()
            .add_systems(
            Update,
            (
                tick_spatial_grid_sync_timer_system,
                update_spatial_grid_system,
                update_familiar_spatial_grid_system,
                update_resource_spatial_grid_system,
                update_designation_spatial_grid_system,
                update_gathering_spot_spatial_grid_system,
                update_blueprint_spatial_grid_system,
                update_floor_construction_spatial_grid_system,
                update_stockpile_spatial_grid_system,
                update_transport_request_spatial_grid_system,
            )
                .in_set(GameSystemSet::Spatial),
            );
    }
}
