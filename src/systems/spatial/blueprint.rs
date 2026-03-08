use bevy::prelude::*;
use crate::systems::jobs::Blueprint;

/// ブループリント用の空間グリッド
pub use hw_spatial::BlueprintSpatialGrid;
pub use hw_spatial::update_blueprint_spatial_grid_system as _update_blueprint_spatial_grid_system;

pub fn update_blueprint_spatial_grid_system(
    grid: ResMut<BlueprintSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<Blueprint>, Or<(Added<Blueprint>, Changed<Transform>)>),
    >,
    removed: RemovedComponents<Blueprint>,
) {
    _update_blueprint_spatial_grid_system::<Blueprint>(grid, query, removed);
}
