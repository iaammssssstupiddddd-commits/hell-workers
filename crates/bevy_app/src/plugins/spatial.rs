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
    update_blueprint_spatial_grid_system_blueprint, update_damned_soul_spatial_grid_system,
    update_designation_spatial_grid_system_designation, update_familiar_entity_spatial_grid_system,
    update_floor_construction_spatial_grid_system, update_gathering_spot_spatial_grid_system,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::reset_runtime_caches;
    use hw_core::soul::DamnedSoul;
    use hw_logistics::types::{ResourceItem, ResourceType};
    use hw_spatial::{
        BlueprintSpatialGrid, FamiliarSpatialGrid, FloorConstructionSpatialGrid,
        GatheringSpotSpatialGrid, ResourceSpatialGrid, SpatialGrid, SpatialGridOps,
        StockpileSpatialGrid,
    };

    #[test]
    fn spatial_plugin_builds_and_rebuilds_indexes_for_rehydrated_entities() {
        let mut app = App::new();
        app.add_plugins(SpatialPlugin)
            .init_resource::<Time<Virtual>>()
            .init_resource::<SpatialGrid>()
            .init_resource::<FamiliarSpatialGrid>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<GatheringSpotSpatialGrid>()
            .init_resource::<BlueprintSpatialGrid>()
            .init_resource::<FloorConstructionSpatialGrid>()
            .init_resource::<StockpileSpatialGrid>()
            .configure_sets(
                Update,
                GameSystemSet::Spatial.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
            );

        let old_item = app
            .world_mut()
            .spawn((
                ResourceItem(ResourceType::Wood),
                Transform::from_xyz(32.0, 0.0, 0.0),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .resource::<ResourceSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(32.0, 0.0), 1.0),
            vec![old_item]
        );

        // A load replaces old entities, resets derived indexes, then writes the
        // new payload. The paused frame must not eagerly rebuild any index.
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut().despawn(old_item);
        reset_runtime_caches(app.world_mut());
        assert!(
            app.world()
                .resource::<ResourceSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(32.0, 0.0), 1.0)
                .is_empty()
        );

        let rehydrated_soul = app
            .world_mut()
            .spawn((DamnedSoul::default(), Transform::from_xyz(128.0, 0.0, 0.0)))
            .id();
        let rehydrated_item = app
            .world_mut()
            .spawn((
                ResourceItem(ResourceType::Wood),
                Transform::from_xyz(96.0, 0.0, 0.0),
            ))
            .id();
        app.update();
        assert!(
            app.world()
                .resource::<SpatialGrid>()
                .get_nearby_in_radius(Vec2::new(128.0, 0.0), 1.0)
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<ResourceSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(96.0, 0.0), 1.0)
                .is_empty()
        );

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.update();
        assert_eq!(
            app.world()
                .resource::<SpatialGrid>()
                .get_nearby_in_radius(Vec2::new(128.0, 0.0), 1.0),
            vec![rehydrated_soul]
        );
        assert_eq!(
            app.world()
                .resource::<ResourceSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(96.0, 0.0), 1.0),
            vec![rehydrated_item]
        );
    }
}
