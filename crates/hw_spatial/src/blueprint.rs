use super::grid::{
    BlueprintIndexTag, SpatialIndex, TransformSpatialUpdateQuery,
    update_transform_spatial_index_system,
};
use bevy::prelude::*;
use hw_jobs::model::Blueprint;

/// ブループリント用の空間グリッド
pub type BlueprintSpatialGrid = SpatialIndex<BlueprintIndexTag>;

pub fn update_blueprint_spatial_grid_system<T: Component>(
    grid: ResMut<BlueprintSpatialGrid>,
    query: TransformSpatialUpdateQuery<T>,
    removed: RemovedComponents<T>,
) {
    update_transform_spatial_index_system::<BlueprintIndexTag, T>(grid, query, removed);
}

/// `Blueprint` コンポーネントに特化した空間グリッド更新システム。
pub fn update_blueprint_spatial_grid_system_blueprint(
    grid: ResMut<BlueprintSpatialGrid>,
    query: TransformSpatialUpdateQuery<Blueprint>,
    removed: RemovedComponents<Blueprint>,
) {
    update_blueprint_spatial_grid_system::<Blueprint>(grid, query, removed);
}
