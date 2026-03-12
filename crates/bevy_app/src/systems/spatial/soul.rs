use crate::entities::damned_soul::DamnedSoul;
use bevy::prelude::*;

pub use hw_spatial::SpatialGrid;
pub use hw_spatial::update_spatial_grid_system as _update_spatial_grid_system;

/// 空間グリッド - Soul位置の高速検索用
pub fn update_spatial_grid_system(
    grid: ResMut<SpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (
            With<DamnedSoul>,
            Or<(Added<DamnedSoul>, Changed<Transform>)>,
        ),
    >,
    removed: RemovedComponents<DamnedSoul>,
) {
    _update_spatial_grid_system::<DamnedSoul>(grid, query, removed);
}
