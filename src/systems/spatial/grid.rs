use crate::constants::SPATIAL_GRID_SYNC_INTERVAL;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// 空間グリッド（Designation/Familiar等）の同期タイマー
///
/// 毎フレームのフル再構築を避け、0.15秒間隔で同期する。
#[derive(Resource)]
pub struct SpatialGridSyncTimer {
    pub timer: Timer,
    pub first_run_done: bool,
}

impl Default for SpatialGridSyncTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(SPATIAL_GRID_SYNC_INTERVAL, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

/// 空間グリッド同期タイマーを進める。6つの update システムより先に実行する。
pub fn tick_spatial_grid_sync_timer_system(
    time: Res<Time>,
    mut sync_timer: ResMut<SpatialGridSyncTimer>,
) {
    sync_timer.timer.tick(time.delta());
}

/// 空間グリッドの共通操作を定義するトレイト
pub trait SpatialGridOps {
    fn insert(&mut self, entity: Entity, pos: Vec2);
    fn remove(&mut self, entity: Entity);
    fn update(&mut self, entity: Entity, pos: Vec2);
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

    pub fn remove(&mut self, entity: Entity) {
        if let Some(pos) = self.positions.remove(&entity) {
            let cell = self.pos_to_cell(pos);
            if let Some(entities) = self.grid.get_mut(&cell) {
                entities.remove(&entity);
                if entities.is_empty() {
                    self.grid.remove(&cell);
                }
            }
        }
    }

    pub fn update(&mut self, entity: Entity, new_pos: Vec2) {
        if let Some(&old_pos) = self.positions.get(&entity) {
            if old_pos == new_pos {
                return;
            }

            let old_cell = self.pos_to_cell(old_pos);
            let new_cell = self.pos_to_cell(new_pos);

            if old_cell == new_cell {
                // セルが変わらない場合は位置情報のみ更新（高速パス）
                self.positions.insert(entity, new_pos);
            } else {
                // セルが変わる場合は移動処理
                if let Some(entities) = self.grid.get_mut(&old_cell) {
                    entities.remove(&entity);
                    if entities.is_empty() {
                        self.grid.remove(&old_cell);
                    }
                }
                self.grid.entry(new_cell).or_default().insert(entity);
                self.positions.insert(entity, new_pos);
            }
        } else {
            // 新規登録
            self.insert(entity, new_pos);
        }
    }

    pub fn clear(&mut self) {
        self.grid.clear();
        self.positions.clear();
    }
}
