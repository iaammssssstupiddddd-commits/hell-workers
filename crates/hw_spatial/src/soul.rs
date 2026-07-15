use super::grid::{
    SoulIndexTag, SpatialIndex, TransformSpatialUpdateQuery, update_transform_spatial_index_system,
};
use bevy::prelude::*;

/// 空間グリッド - 魂位置の高速検索用
pub type SpatialGrid = SpatialIndex<SoulIndexTag>;

pub fn update_spatial_grid_system<T: Component>(
    grid: ResMut<SpatialGrid>,
    query: TransformSpatialUpdateQuery<T>,
    removed: RemovedComponents<T>,
) {
    update_transform_spatial_index_system::<SoulIndexTag, T>(grid, query, removed);
}

/// `DamnedSoul` 専用のグリッド更新システム（bevy_app への re-export 用）。
pub fn update_damned_soul_spatial_grid_system(
    grid: ResMut<SpatialGrid>,
    query: TransformSpatialUpdateQuery<hw_core::soul::DamnedSoul>,
    removed: RemovedComponents<hw_core::soul::DamnedSoul>,
) {
    update_spatial_grid_system::<hw_core::soul::DamnedSoul>(grid, query, removed);
}
