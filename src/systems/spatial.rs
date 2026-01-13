//! 空間グリッドモジュール
//!
//! エンティティの位置を高速検索するためのグリッドデータ構造を提供します。

use crate::constants::TILE_SIZE;
use crate::entities::familiar::Familiar;
use crate::systems::logistics::ResourceItem;
use crate::systems::work::AssignedTask;
use bevy::prelude::*;
use std::collections::HashMap;

// ============================================================
// SpatialGrid - Soul位置の高速検索用
// ============================================================

/// 空間グリッド - Soul位置の高速検索用
#[derive(Resource, Default)]
pub struct SpatialGrid {
    cells: HashMap<(i32, i32), Vec<Entity>>,
    cell_size: f32,
    // 差分更新用: 各エンティティがどのセルにいるかを記録
    entity_cells: HashMap<Entity, (i32, i32)>,
}

impl SpatialGrid {
    #[allow(dead_code)]
    pub fn new(cell_size: f32) -> Self {
        Self {
            cells: HashMap::new(),
            cell_size,
            entity_cells: HashMap::new(),
        }
    }

    fn pos_to_cell(&self, pos: Vec2) -> (i32, i32) {
        let cell_size = if self.cell_size > 0.0 {
            self.cell_size
        } else {
            TILE_SIZE * 8.0
        };
        (
            (pos.x / cell_size).floor() as i32,
            (pos.y / cell_size).floor() as i32,
        )
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.cells.clear();
        self.entity_cells.clear();
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        let cell = self.pos_to_cell(pos);

        // 既に登録されている場合は古いセルから削除
        if let Some(old_cell) = self.entity_cells.get(&entity) {
            if *old_cell != cell {
                if let Some(entities) = self.cells.get_mut(old_cell) {
                    entities.retain(|&e| e != entity);
                }
            } else {
                // 同じセルにいる場合は何もしない
                return;
            }
        }

        self.cells.entry(cell).or_default().push(entity);
        self.entity_cells.insert(entity, cell);
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some(old_cell) = self.entity_cells.remove(&entity) {
            if let Some(entities) = self.cells.get_mut(&old_cell) {
                entities.retain(|&e| e != entity);
            }
        }
    }

    /// 指定位置周辺の9セルにいるエンティティを返す
    pub fn get_nearby(&self, pos: Vec2) -> Vec<Entity> {
        let (cx, cy) = self.pos_to_cell(pos);
        let mut result = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(entities) = self.cells.get(&(cx + dx, cy + dy)) {
                    result.extend(entities.iter().copied());
                }
            }
        }
        result
    }

    /// 指定位置周辺のセルにいるエンティティを返す（検索半径を考慮）
    #[allow(dead_code)]
    pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        let (cx, cy) = self.pos_to_cell(pos);
        let cell_size = if self.cell_size > 0.0 {
            self.cell_size
        } else {
            TILE_SIZE * 8.0
        };
        // 半径を考慮して必要なセル数を計算
        let cells_needed = (radius / cell_size).ceil() as i32 + 1;
        let mut result = Vec::new();
        for dx in -cells_needed..=cells_needed {
            for dy in -cells_needed..=cells_needed {
                if let Some(entities) = self.cells.get(&(cx + dx, cy + dy)) {
                    result.extend(entities.iter().copied());
                }
            }
        }
        result
    }
}

// ============================================================
// FamiliarSpatialGrid - 使い魔用の空間グリッド
// ============================================================

/// 使い魔用の空間グリッド - モチベーション計算の高速化用
#[derive(Resource, Default)]
pub struct FamiliarSpatialGrid {
    cells: HashMap<(i32, i32), Vec<Entity>>,
    cell_size: f32,
    // 差分更新用
    entity_cells: HashMap<Entity, (i32, i32)>,
}

impl FamiliarSpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cells: HashMap::new(),
            cell_size,
            entity_cells: HashMap::new(),
        }
    }

    fn pos_to_cell(&self, pos: Vec2) -> (i32, i32) {
        let cell_size = if self.cell_size > 0.0 {
            self.cell_size
        } else {
            TILE_SIZE * 8.0
        };
        (
            (pos.x / cell_size).floor() as i32,
            (pos.y / cell_size).floor() as i32,
        )
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.cells.clear();
        self.entity_cells.clear();
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        let cell = self.pos_to_cell(pos);

        // 既に登録されている場合は古いセルから削除
        if let Some(old_cell) = self.entity_cells.get(&entity) {
            if *old_cell != cell {
                if let Some(entities) = self.cells.get_mut(old_cell) {
                    entities.retain(|&e| e != entity);
                }
            } else {
                // 同じセルにいる場合は何もしない
                return;
            }
        }

        self.cells.entry(cell).or_default().push(entity);
        self.entity_cells.insert(entity, cell);
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, entity: Entity) {
        if let Some(old_cell) = self.entity_cells.remove(&entity) {
            if let Some(entities) = self.cells.get_mut(&old_cell) {
                entities.retain(|&e| e != entity);
            }
        }
    }

    /// 指定位置周辺のセルにいるエンティティを返す（検索半径を考慮）
    pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        let (cx, cy) = self.pos_to_cell(pos);
        let cell_size = if self.cell_size > 0.0 {
            self.cell_size
        } else {
            TILE_SIZE * 8.0
        };
        // 半径を考慮して必要なセル数を計算
        let cells_needed = (radius / cell_size).ceil() as i32 + 1;
        let mut result = Vec::new();
        for dx in -cells_needed..=cells_needed {
            for dy in -cells_needed..=cells_needed {
                if let Some(entities) = self.cells.get(&(cx + dx, cy + dy)) {
                    result.extend(entities.iter().copied());
                }
            }
        }
        result
    }
}

// ============================================================
// ResourceSpatialGrid - リソースアイテム用の空間グリッド
// ============================================================

/// リソースアイテム用の空間グリッド
#[derive(Resource, Default)]
pub struct ResourceSpatialGrid {
    cells: HashMap<(i32, i32), Vec<Entity>>,
    cell_size: f32,
    entity_cells: HashMap<Entity, (i32, i32)>,
}

impl ResourceSpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cells: HashMap::new(),
            cell_size,
            entity_cells: HashMap::new(),
        }
    }

    fn pos_to_cell(&self, pos: Vec2) -> (i32, i32) {
        let cell_size = if self.cell_size > 0.0 {
            self.cell_size
        } else {
            TILE_SIZE * 8.0
        };
        (
            (pos.x / cell_size).floor() as i32,
            (pos.y / cell_size).floor() as i32,
        )
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.cells.clear();
        self.entity_cells.clear();
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        let cell = self.pos_to_cell(pos);

        if let Some(old_cell) = self.entity_cells.get(&entity) {
            if *old_cell != cell {
                if let Some(entities) = self.cells.get_mut(old_cell) {
                    entities.retain(|&e| e != entity);
                }
            } else {
                return;
            }
        }

        self.cells.entry(cell).or_default().push(entity);
        self.entity_cells.insert(entity, cell);
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some(old_cell) = self.entity_cells.remove(&entity) {
            if let Some(entities) = self.cells.get_mut(&old_cell) {
                entities.retain(|&e| e != entity);
            }
        }
    }

    pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        let (cx, cy) = self.pos_to_cell(pos);
        let cell_size = if self.cell_size > 0.0 {
            self.cell_size
        } else {
            TILE_SIZE * 8.0
        };
        let cells_needed = (radius / cell_size).ceil() as i32 + 1;
        let mut result = Vec::new();
        for dx in -cells_needed..=cells_needed {
            for dy in -cells_needed..=cells_needed {
                if let Some(entities) = self.cells.get(&(cx + dx, cy + dy)) {
                    result.extend(entities.iter().copied());
                }
            }
        }
        result
    }
}

// ============================================================
// 空間グリッド更新システム
// ============================================================

/// SpatialGridを更新するシステム（差分更新）
///
/// グリッドに登録される条件：
/// - タスクなし（AssignedTask::None）
/// - ExhaustedGatheringではない
///
/// 疲労やmotivationはリクルート検索時にチェックする
pub fn update_spatial_grid_system(
    mut spatial_grid: ResMut<SpatialGrid>,
    q_souls: Query<(
        Entity,
        &Transform,
        &AssignedTask,
        &crate::entities::damned_soul::IdleState,
    )>,
) {
    for (entity, transform, task, idle) in q_souls.iter() {
        let should_be_in_grid = matches!(task, AssignedTask::None)
            && idle.behavior != crate::entities::damned_soul::IdleBehavior::ExhaustedGathering;

        if should_be_in_grid {
            spatial_grid.insert(entity, transform.translation.truncate());
        } else {
            spatial_grid.remove(entity);
        }
    }
}

/// FamiliarSpatialGridを更新するシステム（差分更新）
pub fn update_familiar_spatial_grid_system(
    mut familiar_grid: ResMut<FamiliarSpatialGrid>,
    q_familiars: Query<(Entity, &Transform, &Familiar), Changed<Transform>>,
) {
    // 変更された使い魔のみ更新
    for (entity, transform, _) in q_familiars.iter() {
        familiar_grid.insert(entity, transform.translation.truncate());
    }
}

/// リソースグリッドを更新するシステム（差分更新）
pub fn update_resource_spatial_grid_system(
    mut resource_grid: ResMut<ResourceSpatialGrid>,
    q_resources_added: Query<
        (Entity, &Transform, Option<&Visibility>),
        (With<ResourceItem>, Added<ResourceItem>),
    >,
    q_resources: Query<
        (Entity, &Transform, Option<&Visibility>),
        (With<ResourceItem>, Changed<Transform>),
    >,
    q_visibility_changed: Query<
        (Entity, &Transform, Option<&Visibility>),
        (With<ResourceItem>, Changed<Visibility>),
    >,
) {
    // 新しく追加されたリソースを登録
    // Visibility::Hiddenのリソース（拾われている）は除外、それ以外は登録
    for (entity, transform, visibility) in q_resources_added.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
            info!(
                "RESOURCE_GRID: Added resource {:?} at {:?}",
                entity,
                transform.translation.truncate()
            );
        }
    }

    // 位置が変更されたリソースを更新
    for (entity, transform, visibility) in q_resources.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
        } else {
            resource_grid.remove(entity);
        }
    }

    // 可視性が変更されたリソースを更新
    for (entity, transform, visibility) in q_visibility_changed.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
        } else {
            resource_grid.remove(entity);
        }
    }
}
