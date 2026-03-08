use bevy::prelude::*;
use crate::systems::logistics::ResourceItem;

/// リソースアイテム用の空間グリッド
pub use hw_spatial::ResourceSpatialGrid;
pub use hw_spatial::update_resource_spatial_grid_system as _update_resource_spatial_grid_system;

pub fn update_resource_spatial_grid_system(
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
    _update_resource_spatial_grid_system::<ResourceItem>(
        grid,
        q_changed,
        q_resource_transform,
        removed_items,
        removed_visibility,
    );
}
