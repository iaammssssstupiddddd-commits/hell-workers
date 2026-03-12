//! ナビゲーション純粋ヘルパー
//!
//! `Commands` / root-only Resource に依存しない純粋関数群。
//! 目的地更新・距離判定・隣接判定などを提供する。

use bevy::prelude::Vec2;
use hw_core::constants::TILE_SIZE;
use hw_core::soul::{Destination, Path};
use hw_world::map::WorldMap;

/// 目的地を更新（必要に応じて）
///
/// 目的地が2.0以上離れている場合にのみ更新します。
pub fn update_destination_if_needed(dest: &mut Destination, target_pos: Vec2, path: &mut Path) {
    if dest.0.distance_squared(target_pos) > 2.0 {
        dest.0 = target_pos;
        path.waypoints.clear();
    }
}

/// 距離チェック: 魂がターゲットに近づいたかどうか
///
/// 隣接マス（中心間距離32px）からでも確実に「近い」と判定されるように、
/// タイルサイズの1.8倍（57.6px）を閾値に設定。
pub fn is_near_target(soul_pos: Vec2, target_pos: Vec2) -> bool {
    soul_pos.distance(target_pos) < TILE_SIZE * 1.8
}

/// ターゲットまたは現在の目的地への近接判定
pub fn is_near_target_or_dest(soul_pos: Vec2, target_pos: Vec2, dest_pos: Vec2) -> bool {
    is_near_target(soul_pos, target_pos) || is_near_target(soul_pos, dest_pos)
}

/// グリッド上で隣接しているか（斜め含む）
pub fn is_adjacent_grid(soul_pos: Vec2, target_pos: Vec2) -> bool {
    let sg = WorldMap::world_to_grid(soul_pos);
    let tg = WorldMap::world_to_grid(target_pos);
    (sg.0 - tg.0).abs() <= 1 && (sg.1 - tg.1).abs() <= 1
}

/// アイテムの拾い判定は隣接グリッドのみ許可する
pub fn can_pickup_item(soul_pos: Vec2, item_pos: Vec2) -> bool {
    is_adjacent_grid(soul_pos, item_pos)
}

/// 設計図への距離チェック: 魂が設計図の構成タイルのいずれかに近づいたかどうか
///
/// 修正: 建設作業を予定地の上で行わないようにするため、
/// 1. ソウルの中心が予定地（occupied_grids）のいずれかに含まれている場合は false を返す。
/// 2. その上で、予定地のいずれかのタイルに隣接（距離 1.5 TILE_SIZE 未満）している場合に true を返す。
pub fn is_near_blueprint(soul_pos: Vec2, occupied_grids: &[(i32, i32)]) -> bool {
    let soul_grid = WorldMap::world_to_grid(soul_pos);

    if occupied_grids.contains(&soul_grid) {
        return false;
    }

    for &(gx, gy) in occupied_grids {
        let grid_pos = WorldMap::grid_to_world(gx, gy);
        let dist = soul_pos.distance(grid_pos);

        // 隣接（1.5タイル分以内）していればOK
        // 斜め方向の距離が約1.414なため、1.5必要。
        if dist < TILE_SIZE * 1.5 {
            return true;
        }
    }
    false
}
