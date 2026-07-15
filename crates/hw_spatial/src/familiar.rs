use super::grid::{
    FamiliarIndexTag, SpatialIndex, TransformSpatialUpdateQuery,
    update_transform_spatial_index_system,
};
use bevy::prelude::*;

/// 使い魔用の空間グリッド - モチベーション計算の高速化用
pub type FamiliarSpatialGrid = SpatialIndex<FamiliarIndexTag>;

pub fn update_familiar_spatial_grid_system<T: Component>(
    grid: ResMut<FamiliarSpatialGrid>,
    query: TransformSpatialUpdateQuery<T>,
    removed: RemovedComponents<T>,
) {
    update_transform_spatial_index_system::<FamiliarIndexTag, T>(grid, query, removed);
}

/// `Familiar` 専用のグリッド更新システム（bevy_app への re-export 用）。
pub fn update_familiar_entity_spatial_grid_system(
    grid: ResMut<FamiliarSpatialGrid>,
    query: TransformSpatialUpdateQuery<hw_core::familiar::Familiar>,
    removed: RemovedComponents<hw_core::familiar::Familiar>,
) {
    update_familiar_spatial_grid_system::<hw_core::familiar::Familiar>(grid, query, removed);
}
