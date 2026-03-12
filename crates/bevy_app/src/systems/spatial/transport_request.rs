use crate::systems::logistics::transport_request::TransportRequest;
use bevy::prelude::*;

/// TransportRequest 用の空間グリッド
pub use hw_spatial::TransportRequestSpatialGrid;
pub use hw_spatial::update_transport_request_spatial_grid_system as _update_transport_request_spatial_grid_system;

/// 変更差分のみを反映する。スポーン直後は次フレームで取り込まれる。
pub fn update_transport_request_spatial_grid_system(
    grid: ResMut<TransportRequestSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (
            With<TransportRequest>,
            Or<(Added<TransportRequest>, Changed<Transform>)>,
        ),
    >,
    removed: RemovedComponents<TransportRequest>,
) {
    _update_transport_request_spatial_grid_system::<TransportRequest>(grid, query, removed);
}
