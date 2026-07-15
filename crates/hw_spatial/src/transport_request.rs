use super::grid::{
    SpatialIndex, TransformSpatialUpdateQuery, TransportRequestIndexTag,
    update_transform_spatial_index_system,
};
use bevy::prelude::*;

/// TransportRequest 用の空間グリッド
pub type TransportRequestSpatialGrid = SpatialIndex<TransportRequestIndexTag>;

pub fn update_transport_request_spatial_grid_system<T: Component>(
    grid: ResMut<TransportRequestSpatialGrid>,
    query: TransformSpatialUpdateQuery<T>,
    removed: RemovedComponents<T>,
) {
    update_transform_spatial_index_system::<TransportRequestIndexTag, T>(grid, query, removed);
}
