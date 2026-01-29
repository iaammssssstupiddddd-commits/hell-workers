use super::grid::{GridData, SpatialGridOps};
use crate::entities::damned_soul::DamnedSoul;
use bevy::prelude::*;

/// 空間グリッド - Soul位置の高速検索用
#[derive(Resource, Default)]
pub struct SpatialGrid(pub GridData);

impl SpatialGrid {
    // SpatialGrid は現状では Resource::default() で初期化されています
}

impl SpatialGridOps for SpatialGrid {
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

pub fn update_spatial_grid_system(
    mut grid: ResMut<SpatialGrid>,
    query: Query<(Entity, &Transform), With<DamnedSoul>>,
    mut removed: RemovedComponents<DamnedSoul>,
) {
    // 移動したエンティティのみ更新（GridData::update側で最適化）
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }

    // 削除されたエンティティをグリッドから除去
    for entity in removed.read() {
        grid.remove(entity);
    }
}
