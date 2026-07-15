use super::grid::{
    FloorConstructionIndexTag, SpatialIndex, TransformSpatialUpdateQuery,
    update_transform_spatial_index_system,
};
use bevy::prelude::*;
use hw_jobs::FloorConstructionSite;

/// FloorConstructionSite 用の空間グリッド
pub type FloorConstructionSpatialGrid = SpatialIndex<FloorConstructionIndexTag>;

pub fn update_floor_construction_spatial_grid_system(
    grid: ResMut<FloorConstructionSpatialGrid>,
    query: TransformSpatialUpdateQuery<FloorConstructionSite>,
    removed: RemovedComponents<FloorConstructionSite>,
) {
    update_transform_spatial_index_system::<FloorConstructionIndexTag, FloorConstructionSite>(
        grid, query, removed,
    );
}
