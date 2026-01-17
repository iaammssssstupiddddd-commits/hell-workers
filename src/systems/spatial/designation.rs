use super::grid::{GridData, SpatialGridOps};
use crate::systems::jobs::Designation;
use bevy::prelude::*;

/// タスク（Designation）用の空間グリッド
#[derive(Resource, Default)]
pub struct DesignationSpatialGrid(pub GridData);

impl DesignationSpatialGrid {
    // DesignationSpatialGrid は現状では Resource::default() で初期化されています

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

impl SpatialGridOps for DesignationSpatialGrid {
    fn insert(&mut self, entity: Entity, pos: Vec2) {
        self.0.insert(entity, pos);
    }
    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        self.0.get_nearby_in_radius(pos, radius)
    }
}

pub fn update_designation_spatial_grid_system(
    mut grid: ResMut<DesignationSpatialGrid>,
    query: Query<(Entity, &Transform), With<Designation>>,
) {
    grid.0.clear();
    for (entity, transform) in query.iter() {
        grid.0.insert(entity, transform.translation.truncate());
    }
}
