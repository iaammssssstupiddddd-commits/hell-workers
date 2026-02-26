use super::grid::{GridData, SpatialGridOps};
use crate::systems::logistics::ResourceItem;
use bevy::prelude::*;

/// リソースアイテム用の空間グリッド
#[derive(Resource, Default)]
pub struct ResourceSpatialGrid(pub GridData);

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
    fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
        self.0.get_nearby_in_radius_into(pos, radius, out);
    }
}

pub fn update_resource_spatial_grid_system(
    mut grid: ResMut<ResourceSpatialGrid>,
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
    mut removed_items: RemovedComponents<ResourceItem>,
    mut removed_visibility: RemovedComponents<Visibility>,
) {
    // 変更があったエンティティのみ更新（移動・表示切替）
    for (entity, transform, visibility) in q_changed.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            grid.update(entity, transform.translation.truncate());
        } else {
            // 非表示になった場合はグリッドから削除
            grid.remove(entity);
        }
    }

    // Visibility コンポーネントが外れた場合は可視扱いで再登録する。
    for entity in removed_visibility.read() {
        if let Ok(transform) = q_resource_transform.get(entity) {
            grid.update(entity, transform.translation.truncate());
        }
    }

    // 削除されたアイテムをグリッドから除去
    for entity in removed_items.read() {
        grid.remove(entity);
    }
}
