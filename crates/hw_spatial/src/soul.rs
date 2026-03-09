use super::grid::{GridData, SpatialGridOps};
use bevy::prelude::*;

/// 空間グリッド - 魂位置の高速検索用
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
    fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
        self.0.get_nearby_in_radius_into(pos, radius, out);
    }
}

pub fn update_spatial_grid_system<T: Component>(
    mut grid: ResMut<SpatialGrid>,
    query: Query<(Entity, &Transform), (With<T>, Or<(Added<T>, Changed<Transform>)>)>,
    mut removed: RemovedComponents<T>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }

    for entity in removed.read() {
        grid.remove(entity);
    }
}
