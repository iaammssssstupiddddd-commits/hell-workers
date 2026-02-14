//! 手押し車「徒歩完了可能」判定の共通ロジック
//!
//! Phase 2: can_complete_pick_drop_to_point と can_complete_pick_drop_to_blueprint を
//! 単一モジュールに集約。閾値を 1 箇所で管理する。

use crate::constants::TILE_SIZE;
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// ピック位置からポイントへのドロップ判定距離（タイル単位の倍率）
pub const PICK_DROP_TO_POINT_THRESHOLD: f32 = 1.8;

/// ピック位置から Blueprint グリッドへのドロップ判定距離（タイル単位の倍率）
pub const PICK_DROP_TO_BLUEPRINT_THRESHOLD: f32 = 1.5;

/// ソース位置から徒歩で目的地（ポイント）へピック＆ドロップ完了可能か
///
// 実タスク条件に合わせる:
// 1) source に隣接して拾える立ち位置が存在し
// 2) その立ち位置が destination へのドロップ判定を満たす
pub fn can_complete_pick_drop_to_point(source_pos: Vec2, destination_pos: Vec2) -> bool {
    let source_grid = WorldMap::world_to_grid(source_pos);
    for dx in -1..=1 {
        for dy in -1..=1 {
            let stand_pos = WorldMap::grid_to_world(source_grid.0 + dx, source_grid.1 + dy);
            if stand_pos.distance(destination_pos) < TILE_SIZE * PICK_DROP_TO_POINT_THRESHOLD {
                return true;
            }
        }
    }
    false
}

/// ソース位置から徒歩で Blueprint へピック＆ドロップ完了可能か
pub fn can_complete_pick_drop_to_blueprint(
    source_pos: Vec2,
    occupied_grids: &[(i32, i32)],
) -> bool {
    let source_grid = WorldMap::world_to_grid(source_pos);
    for dx in -1..=1 {
        for dy in -1..=1 {
            let stand_grid = (source_grid.0 + dx, source_grid.1 + dy);
            if occupied_grids.contains(&stand_grid) {
                continue;
            }
            let stand_pos = WorldMap::grid_to_world(stand_grid.0, stand_grid.1);
            if occupied_grids.iter().any(|&(gx, gy)| {
                let bp_pos = WorldMap::grid_to_world(gx, gy);
                stand_pos.distance(bp_pos) < TILE_SIZE * PICK_DROP_TO_BLUEPRINT_THRESHOLD
            }) {
                return true;
            }
        }
    }
    false
}
