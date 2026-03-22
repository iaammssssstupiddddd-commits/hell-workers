//! 空間グリッド関連のプラグイン

use crate::systems::GameSystemSet;
use crate::systems::logistics::{
    TileSiteIndex, sync_floor_tile_site_index_system, sync_removed_floor_tile_site_index_system,
    sync_removed_wall_tile_site_index_system, sync_wall_tile_site_index_system,
};
use bevy::prelude::*;
use hw_logistics::{
    update_resource_spatial_grid_system_resource_item,
    update_stockpile_spatial_grid_system_stockpile,
    update_transport_request_spatial_grid_system_transport_request,
};
use hw_spatial::{
    DesignationSpatialGrid, TransportRequestSpatialGrid,
    update_blueprint_spatial_grid_system_blueprint,
    update_damned_soul_spatial_grid_system,
    update_designation_spatial_grid_system_designation,
    update_familiar_entity_spatial_grid_system,
    update_floor_construction_spatial_grid_system,
    update_gathering_spot_spatial_grid_system,
};

pub struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileSiteIndex>();
        app.init_resource::<DesignationSpatialGrid>();
        app.init_resource::<TransportRequestSpatialGrid>();
        app.add_systems(
            Update,
            (
                update_damned_soul_spatial_grid_system,
                update_familiar_entity_spatial_grid_system,
                update_resource_spatial_grid_system_resource_item,
                update_designation_spatial_grid_system_designation,
                update_gathering_spot_spatial_grid_system,
                update_blueprint_spatial_grid_system_blueprint,
                update_floor_construction_spatial_grid_system,
                sync_floor_tile_site_index_system,
                sync_removed_floor_tile_site_index_system,
                sync_wall_tile_site_index_system,
                sync_removed_wall_tile_site_index_system,
                update_stockpile_spatial_grid_system_stockpile,
                update_transport_request_spatial_grid_system_transport_request,
            )
                .in_set(GameSystemSet::Spatial),
        );
    }
}
