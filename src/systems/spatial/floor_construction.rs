use super::grid::{GridData, SpatialGridOps};
use crate::systems::jobs::floor_construction::FloorConstructionSite;
use bevy::prelude::*;

/// FloorConstructionSite用の空間グリッド
#[derive(Resource, Default)]
pub struct FloorConstructionSpatialGrid(pub GridData);

impl FloorConstructionSpatialGrid {
    /// 指定範囲内のフロア建設サイトを取得（TaskAreaとの連携用）
    pub fn get_in_area(&self, min: Vec2, max: Vec2) -> Vec<Entity> {
        let mut results = Vec::new();
        let min_cell = self.0.pos_to_cell(min);
        let max_cell = self.0.pos_to_cell(max);

        for dy in min_cell.1..=max_cell.1 {
            for dx in min_cell.0..=max_cell.0 {
                let cell = (dx, dy);
                if let Some(entities) = self.0.grid.get(&cell) {
                    for &entity in entities {
                        if let Some(&pos) = self.0.positions.get(&entity) {
                            if pos.x >= min.x && pos.x <= max.x && pos.y >= min.y && pos.y <= max.y
                            {
                                results.push(entity);
                            }
                        }
                    }
                }
            }
        }
        results
    }
}

impl SpatialGridOps for FloorConstructionSpatialGrid {
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

pub fn update_floor_construction_spatial_grid_system(
    mut grid: ResMut<FloorConstructionSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (
            With<FloorConstructionSite>,
            Or<(Added<FloorConstructionSite>, Changed<Transform>)>,
        ),
    >,
    mut removed: RemovedComponents<FloorConstructionSite>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
