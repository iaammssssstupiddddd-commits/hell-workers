//! タスク実行の共通処理

use crate::constants::*;
use crate::entities::damned_soul::{Destination, Path};
use crate::systems::jobs::Designation;
use crate::systems::logistics::{Inventory, Stockpile};
use crate::systems::soul_ai::task_execution::types::AssignedTask;
use bevy::prelude::*;

use crate::world::map::WorldMap; // 追加

/// 目的地を更新（必要に応じて）
///
/// 目的地が2.0以上離れている場合にのみ更新します。
pub fn update_destination_if_needed(dest: &mut Destination, target_pos: Vec2, path: &mut Path) {
    if dest.0.distance_squared(target_pos) > 2.0 {
        dest.0 = target_pos;
        path.waypoints.clear();
    }
}

/// インタラクション対象への隣接目的地を設定（岩などへの近接用）
///
/// 到達可能な隣接マスがあれば`true`を返し、なければ`false`を返す
pub fn update_destination_to_adjacent(
    dest: &mut Destination,
    target_pos: Vec2,
    path: &mut Path,
    soul_pos: Vec2,
    world_map: &WorldMap,
) -> bool {
    let target_grid = WorldMap::world_to_grid(target_pos);
    
    // ターゲット自体がWalkableならそのままターゲットへ
    if world_map.is_walkable(target_grid.0, target_grid.1) {
        update_destination_if_needed(dest, target_pos, path);
        return true;
    }
    
    // 隣接マスのうち、Walkableで現在位置に最も近いものを探す（8方向）
    let directions = [
        (0, 1), (0, -1), (1, 0), (-1, 0),
        (1, 1), (1, -1), (-1, 1), (-1, -1)
    ];
    
    let mut best_pos = None;
    let mut min_dist_sq = f32::MAX;
    
    for (dx, dy) in directions {
        let nx = target_grid.0 + dx;
        let ny = target_grid.1 + dy;
        
        if world_map.is_walkable(nx, ny) {
            let world_pos = WorldMap::grid_to_world(nx, ny);
            let dist_sq = soul_pos.distance_squared(world_pos);
            if dist_sq < min_dist_sq {
                min_dist_sq = dist_sq;
                best_pos = Some(world_pos);
            }
        }
    }
    
    if let Some(pos) = best_pos {
        update_destination_if_needed(dest, pos, path);
        true
    } else {
        // 到達不能: 近づける場所がない（完全な袋小路など）
        false
    }
}


/// 設計図への到達パスを設定（予定地の中心を一意なターゲットとする）
pub fn update_destination_to_blueprint(
    dest: &mut Destination,
    occupied_grids: &[(i32, i32)],
    path: &mut Path,
    soul_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut crate::world::pathfinding::PathfindingContext,
) {
    let start_grid = WorldMap::world_to_grid(soul_pos);
    
    // 現在地がすでにゴール条件を満たしているかチェック
    if is_near_blueprint(soul_pos, occupied_grids) {
        // 到着済みなら、不要なパス（予定地内へ続くものなど）を消去して停止させる
        if !path.waypoints.is_empty() {
            path.waypoints.clear();
            path.current_index = 0;
            dest.0 = soul_pos;
        }
        return;
    }

    // 現在のパスが既に有効（ターゲットの隣接点に向かっている）なら再計算しない
    if !path.waypoints.is_empty() {
        if let Some(last_wp) = path.waypoints.last() {
            let last_grid = WorldMap::world_to_grid(*last_wp);
            
            // 終点が予定地外かつターゲットに隣接していれば、そのパスは有効
            if !occupied_grids.contains(&last_grid) {
                for &(gx, gy) in occupied_grids {
                    let dx = (last_grid.0 - gx).abs();
                    let dy = (last_grid.1 - gy).abs();
                    if dx <= 1 && dy <= 1 {
                        return;
                    }
                }
            }
        }
    }
    
    // ターゲットの中心地点を軸に「境界」までのパスを計算
    if let Some(grid_path) = crate::world::pathfinding::find_path_to_boundary(
        world_map,
        pf_context,
        start_grid,
        occupied_grids
    ) {
        if let Some(last_grid) = grid_path.last() {
             let last_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
             update_destination_if_needed(dest, last_pos, path);
             
             path.waypoints = grid_path
                .iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
             path.current_index = 0;
        }
    }
}

/// タスクとパスをクリア
pub fn clear_task_and_path(task: &mut AssignedTask, path: &mut Path) {
    *task = AssignedTask::None;
    path.waypoints.clear();
}

/// 指定が解除されていたらタスクをキャンセル
///
/// 指定が解除されていた場合、タスクとパスをクリアして`true`を返します。
/// 指定が存在する場合、`false`を返します。
pub fn cancel_task_if_designation_missing(
    des_opt: Option<&Designation>,
    task: &mut AssignedTask,
    path: &mut Path,
) -> bool {
    if des_opt.is_none() {
        clear_task_and_path(task, path);
        return true;
    }
    false
}

/// アイテムをピックアップ
///
/// 魂にアイテムを持たせ、アイテムを非表示にします。
pub fn pickup_item(
    commands: &mut Commands,
    _soul_entity: Entity,
    item_entity: Entity,
    inventory: &mut Inventory,
) {
    inventory.0 = Some(item_entity);
    commands.entity(item_entity).insert(Visibility::Hidden);

    // タスク指定・備蓄状態を削除
    //
    // 重要: `IssuedBy(=ManagedBy)` はここでは削除しない。
    // タスク実行中にアイテムを一時的に拾っている間も「どの使い魔が管理していたか」を保持しておくことで、
    // タスク放棄などでドロップされた場合でも ManagedTasks 経由で再検知できる。
    commands
        .entity(item_entity)
        .remove::<crate::systems::jobs::Designation>()
        .remove::<crate::systems::jobs::TaskSlots>()
        .remove::<crate::systems::jobs::Priority>()
        .remove::<crate::relationships::StoredIn>()
        .remove::<crate::systems::logistics::InStockpile>();
}

/// アイテムを地面に落とす
pub fn drop_item(
    commands: &mut Commands,
    _soul_entity: Entity,
    item_entity: Entity,
    pos: Vec2,
) {
    commands.entity(item_entity).insert((
        Visibility::Visible,
        Transform::from_xyz(pos.x, pos.y, 0.6),
    ));
}

/// ストックパイルからアイテムが取り出された際の更新処理
///
/// ストックパイルが空になった場合、リソースタイプをリセットします。
pub fn update_stockpile_on_item_removal(
    stock_entity: Entity,
    q_stockpiles: &mut Query<(
        Entity,
        &Transform,
        &mut Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
) {
    if let Ok((_, _, mut stock_comp, Some(stored_items))) = q_stockpiles.get_mut(stock_entity) {
        // 自分を引いて 0 個になるなら None に戻す
        if stored_items.len() <= 1 {
            stock_comp.resource_type = None;
            info!(
                "STOCKPILE: Stockpile {:?} became empty. Resetting resource type.",
                stock_entity
            );
        }
    }
}

/// 距離チェック: 魂がターゲットに近づいたかどうか
///
/// 隣接マス（中心間距離32px）からでも確実に「近い」と判定されるように、
/// タイルサイズの1.5倍（48px）を閾値に設定。
pub fn is_near_target(soul_pos: Vec2, target_pos: Vec2) -> bool {
    soul_pos.distance(target_pos) < TILE_SIZE * 1.8
}

/// 設計図への距離チェック: 魂が設計図の構成タイルのいずれかに近づいたかどうか
///
/// 修正: 建設作業を予定地の上で行わないようにするため、
/// 1. ソウルの中心が予定地（occupied_grids）のいずれかに含まれている場合は false を返す。
/// 2. その上で、予定地のいずれかのタイルに隣接（距離 1.5 TILE_SIZE 未満）している場合に true を返す。
pub fn is_near_blueprint(soul_pos: Vec2, occupied_grids: &[(i32, i32)]) -> bool {
    let soul_grid = WorldMap::world_to_grid(soul_pos);
    
    // 予定地の上に立っていたらダメ
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
