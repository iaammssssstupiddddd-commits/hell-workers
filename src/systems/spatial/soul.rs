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
    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        self.0.get_nearby_in_radius(pos, radius)
    }
}

pub fn update_spatial_grid_system(
    mut grid: ResMut<SpatialGrid>,
    query: Query<(Entity, &Transform), With<DamnedSoul>>,
) {
    grid.0.clear();
    for (entity, transform) in query.iter() {
        grid.0.insert(entity, transform.translation.truncate());
    }
}
