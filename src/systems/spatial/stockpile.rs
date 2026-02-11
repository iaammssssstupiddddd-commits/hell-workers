use super::grid::{sync_grid_timed, GridData, SpatialGridOps, SpatialGridSyncTimer, SyncGridClear};
use crate::systems::logistics::Stockpile;
use bevy::prelude::*;

/// ストックパイル用の空間グリッド
#[derive(Resource, Default)]
pub struct StockpileSpatialGrid(pub GridData);

impl StockpileSpatialGrid {
    /// 指定範囲内のストックパイルを取得
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

impl SpatialGridOps for StockpileSpatialGrid {
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

impl SyncGridClear for StockpileSpatialGrid {
    fn clear_and_sync<I>(&mut self, entities: I)
    where
        I: Iterator<Item = (Entity, Vec2)>,
    {
        self.0.clear();
        for (entity, pos) in entities {
            self.0.insert(entity, pos);
        }
    }
}

pub fn update_stockpile_spatial_grid_system(
    mut sync_timer: ResMut<SpatialGridSyncTimer>,
    mut grid: ResMut<StockpileSpatialGrid>,
    query: Query<(Entity, &Transform), With<Stockpile>>,
) {
    sync_grid_timed(
        &mut sync_timer,
        &mut *grid,
        query.iter().map(|(e, t)| (e, t.translation.truncate())),
    );
}
