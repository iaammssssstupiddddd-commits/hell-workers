use super::grid::{
    DesignationIndexTag, SpatialIndex, TransformSpatialUpdateQuery,
    update_transform_spatial_index_system,
};
use bevy::prelude::*;
use hw_jobs::model::Designation;

/// タスク（Designation）用の空間グリッド
pub type DesignationSpatialGrid = SpatialIndex<DesignationIndexTag>;

pub fn update_designation_spatial_grid_system<T: Component>(
    grid: ResMut<DesignationSpatialGrid>,
    query: TransformSpatialUpdateQuery<T>,
    removed: RemovedComponents<T>,
) {
    update_transform_spatial_index_system::<DesignationIndexTag, T>(grid, query, removed);
}

/// `Designation` コンポーネントに特化した空間グリッド更新システム。
pub fn update_designation_spatial_grid_system_designation(
    grid: ResMut<DesignationSpatialGrid>,
    query: TransformSpatialUpdateQuery<Designation>,
    removed: RemovedComponents<Designation>,
) {
    update_designation_spatial_grid_system::<Designation>(grid, query, removed);
}
