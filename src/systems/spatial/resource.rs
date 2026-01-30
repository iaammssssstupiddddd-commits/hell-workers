use super::grid::{GridData, SpatialGridOps};
use crate::systems::logistics::ResourceItem;
use bevy::prelude::*;

/// リソースアイテム用の空間グリッド
#[derive(Resource, Default)]
pub struct ResourceSpatialGrid(pub GridData);

impl ResourceSpatialGrid {
    // ResourceSpatialGrid は現状では Resource::default() で初期化されています
}

impl SpatialGridOps for ResourceSpatialGrid {
    fn insert(&mut self, entity: Entity, pos: Vec2) {
        self.0.insert(entity, pos);
    }
    fn remove(&mut self, entity: Entity) {
        self.0.remove(entity);
    }
    fn update(&mut self, entity: Entity, pos: Vec2) {
        self.0.update(entity, pos);
    }
    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        self.0.get_nearby_in_radius(pos, radius)
    }
}

pub fn update_resource_spatial_grid_system(
    mut grid: ResMut<ResourceSpatialGrid>,
    query: Query<(Entity, &Transform, Option<&Visibility>), With<ResourceItem>>,
    mut removed: RemovedComponents<ResourceItem>,
) {
    // 状態更新（移動・表示切替）
    for (entity, transform, visibility) in query.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            grid.update(entity, transform.translation.truncate());
        } else {
            // 非表示になった場合はグリッドから削除
            grid.remove(entity);
        }
    }

    // 削除されたアイテムをグリッドから除去
    for entity in removed.read() {
        grid.remove(entity);
    }
}
