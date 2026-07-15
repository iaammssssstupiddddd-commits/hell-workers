//! hw_logistics 型に特化した空間グリッド更新システム。
//! bevy_app の thin wrapper を代替する。

use crate::transport_request::TransportRequest;
use crate::types::ResourceItem;
use crate::zone::Stockpile;
use bevy::prelude::*;
use hw_spatial::{
    ResourceSpatialGrid, StockpileIndexTag, StockpileSpatialGrid, TransformSpatialUpdateQuery,
    TransportRequestIndexTag, TransportRequestSpatialGrid, update_resource_spatial_grid_system,
    update_transform_spatial_index_system,
};

type ResourceItemChangedQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, Option<&'static Visibility>),
    (
        With<ResourceItem>,
        Or<(
            Added<ResourceItem>,
            Added<Visibility>,
            Changed<Transform>,
            Changed<Visibility>,
        )>,
    ),
>;

/// `Stockpile` コンポーネントに特化した空間グリッド更新システム。
pub fn update_stockpile_spatial_grid_system_stockpile(
    grid: ResMut<StockpileSpatialGrid>,
    query: TransformSpatialUpdateQuery<Stockpile>,
    removed: RemovedComponents<Stockpile>,
) {
    update_transform_spatial_index_system::<StockpileIndexTag, Stockpile>(grid, query, removed);
}

/// `TransportRequest` コンポーネントに特化した空間グリッド更新システム。
pub fn update_transport_request_spatial_grid_system_transport_request(
    grid: ResMut<TransportRequestSpatialGrid>,
    query: TransformSpatialUpdateQuery<TransportRequest>,
    removed: RemovedComponents<TransportRequest>,
) {
    update_transform_spatial_index_system::<TransportRequestIndexTag, TransportRequest>(
        grid, query, removed,
    );
}

/// `ResourceItem` コンポーネントに特化した空間グリッド更新システム。
pub fn update_resource_spatial_grid_system_resource_item(
    grid: ResMut<ResourceSpatialGrid>,
    q_changed: ResourceItemChangedQuery,
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
