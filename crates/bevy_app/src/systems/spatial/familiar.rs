use crate::entities::familiar::Familiar;
use bevy::prelude::*;

/// 使い魔用の空間グリッド - モチベーション計算の高速化用
pub use hw_spatial::FamiliarSpatialGrid;
pub use hw_spatial::update_familiar_spatial_grid_system as _update_familiar_spatial_grid_system;

pub fn update_familiar_spatial_grid_system(
    grid: ResMut<FamiliarSpatialGrid>,
    query: Query<(Entity, &Transform), (With<Familiar>, Or<(Added<Familiar>, Changed<Transform>)>)>,
    removed: RemovedComponents<Familiar>,
) {
    _update_familiar_spatial_grid_system::<Familiar>(grid, query, removed);
}
