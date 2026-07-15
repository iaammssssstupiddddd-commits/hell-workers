use super::grid::{
    SpatialIndex, StockpileIndexTag, TransformSpatialUpdateQuery,
    update_transform_spatial_index_system,
};
use bevy::prelude::*;

/// ストックパイル用の空間グリッド
pub type StockpileSpatialGrid = SpatialIndex<StockpileIndexTag>;

pub fn update_stockpile_spatial_grid_system<T: Component>(
    grid: ResMut<StockpileSpatialGrid>,
    query: TransformSpatialUpdateQuery<T>,
    removed: RemovedComponents<T>,
) {
    update_transform_spatial_index_system::<StockpileIndexTag, T>(grid, query, removed);
}
