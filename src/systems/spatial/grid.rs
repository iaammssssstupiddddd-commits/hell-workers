use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// 空間グリッドの共通操作を定義するトレイト
pub trait SpatialGridOps {
    fn insert(&mut self, entity: Entity, pos: Vec2);
    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity>;
}

/// 汎用的なグリッドデータ構造
#[derive(Clone)]
pub struct GridData {
    pub cell_size: f32,
    pub grid: HashMap<(i32, i32), HashSet<Entity>>,
    pub positions: HashMap<Entity, Vec2>,
}

impl Default for GridData {
    fn default() -> Self {
        Self::new(32.0 * 20.0) // 640px - マップ全体の数分の一程度
    }
}

impl GridData {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            grid: HashMap::default(),
            positions: HashMap::default(),
        }
    }

    pub fn pos_to_cell(&self, pos: Vec2) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        let cell = self.pos_to_cell(pos);
        self.grid.entry(cell).or_default().insert(entity);
        self.positions.insert(entity, pos);
    }

    pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        let mut results = Vec::new();
        let cell_radius = (radius / self.cell_size).ceil() as i32;
        let center_cell = self.pos_to_cell(pos);

        for dy in -cell_radius..=cell_radius {
            for dx in -cell_radius..=cell_radius {
                let cell = (center_cell.0 + dx, center_cell.1 + dy);
                if let Some(entities) = self.grid.get(&cell) {
                    for &entity in entities {
                        if let Some(&entity_pos) = self.positions.get(&entity) {
                            if pos.distance(entity_pos) <= radius {
                                results.push(entity);
                            }
                        }
                    }
                }
            }
        }
        results
    }

    pub fn clear(&mut self) {
        self.grid.clear();
        self.positions.clear();
    }
}
