//! 空間グリッド関連のプラグイン

use crate::systems::GameSystemSet;
use crate::systems::logistics::{
    TileSiteIndex, sync_floor_tile_site_index_system, sync_removed_floor_tile_site_index_system,
    sync_removed_wall_tile_site_index_system, sync_wall_tile_site_index_system,
};
use crate::systems::spatial::{
    update_floor_construction_spatial_grid_system, update_gathering_spot_spatial_grid_system,
};
use bevy::prelude::*;
use hw_spatial::{
    DesignationSpatialGrid, TransportRequestSpatialGrid, update_blueprint_spatial_grid_system,
    update_designation_spatial_grid_system, update_familiar_spatial_grid_system,
    update_resource_spatial_grid_system, update_spatial_grid_system,
    update_stockpile_spatial_grid_system, update_transport_request_spatial_grid_system,
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
                update_spatial_grid_system::<crate::entities::damned_soul::DamnedSoul>,
                update_familiar_spatial_grid_system::<crate::entities::familiar::Familiar>,
                update_resource_spatial_grid_system::<crate::systems::logistics::ResourceItem>,
                update_designation_spatial_grid_system::<crate::systems::jobs::Designation>,
                update_gathering_spot_spatial_grid_system,
                update_blueprint_spatial_grid_system::<crate::systems::jobs::Blueprint>,
                update_floor_construction_spatial_grid_system,
                sync_floor_tile_site_index_system,
                sync_removed_floor_tile_site_index_system,
                sync_wall_tile_site_index_system,
                sync_removed_wall_tile_site_index_system,
                update_stockpile_spatial_grid_system::<crate::systems::logistics::Stockpile>,
                update_transport_request_spatial_grid_system::<
                    crate::systems::logistics::transport_request::TransportRequest,
                >,
            )
                .in_set(GameSystemSet::Spatial),
        );
    }
}
