//! hw_logistics 型に特化した空間グリッド更新システム。
//! bevy_app の thin wrapper を代替する。

use crate::transport_request::TransportRequest;
use crate::types::ResourceItem;
use crate::zone::Stockpile;
use bevy::prelude::*;
use hw_spatial::{
    ResourceSpatialGrid, StockpileSpatialGrid, TransportRequestSpatialGrid,
    update_resource_spatial_grid_system, update_stockpile_spatial_grid_system,
    update_transport_request_spatial_grid_system,
};

/// `Stockpile` コンポーネントに特化した空間グリッド更新システム。
pub fn update_stockpile_spatial_grid_system_stockpile(
    grid: ResMut<StockpileSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<Stockpile>, Or<(Added<Stockpile>, Changed<Transform>)>),
    >,
    removed: RemovedComponents<Stockpile>,
) {
    update_stockpile_spatial_grid_system::<Stockpile>(grid, query, removed);
}

/// `TransportRequest` コンポーネントに特化した空間グリッド更新システム。
pub fn update_transport_request_spatial_grid_system_transport_request(
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
    // 変更差分のみを反映する。スポーン直後は次フレームで取り込まれる。
    update_transport_request_spatial_grid_system::<TransportRequest>(grid, query, removed);
}

/// `ResourceItem` コンポーネントに特化した空間グリッド更新システム。
pub fn update_resource_spatial_grid_system_resource_item(
    grid: ResMut<ResourceSpatialGrid>,
    q_changed: Query<
        (Entity, &Transform, Option<&Visibility>),
        (
            With<ResourceItem>,
            Or<(
                Added<ResourceItem>,
                Added<Visibility>,
                Changed<Transform>,
                Changed<Visibility>,
            )>,
        ),
    >,
    q_resource_transform: Query<&Transform, With<ResourceItem>>,
    removed_items: RemovedComponents<ResourceItem>,
    removed_visibility: RemovedComponents<Visibility>,
) {
    update_resource_spatial_grid_system::<ResourceItem>(
        grid,
        q_changed,
        q_resource_transform,
        removed_items,
        removed_visibility,
    );
}
