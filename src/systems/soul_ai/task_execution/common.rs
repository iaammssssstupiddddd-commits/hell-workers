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
pub fn update_destination_to_adjacent(
    dest: &mut Destination,
    target_pos: Vec2,
    path: &mut Path,
    soul_pos: Vec2,
    world_map: &WorldMap,
) {
    let target_grid = WorldMap::world_to_grid(target_pos);
    
    // ターゲット自体がWalkableならそのままターゲットへ
    if world_map.is_walkable(target_grid.0, target_grid.1) {
        update_destination_if_needed(dest, target_pos, path);
        return;
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
    } else {
        // 近づける場所がない場合（完全な袋小路など）、ターゲットを目的地にセットするのではなく
        // 到達不能であることを確定させる（目的地を更新しないか、不変にする）
        // もしターゲット自体が歩行可能な場合は上記でリターン済み
    }
}

/// 設計図への到達パスを設定（境界で停止するパスを探索）
pub fn update_destination_to_blueprint(
    dest: &mut Destination,
    occupied_grids: &[(i32, i32)],
    path: &mut Path,
    soul_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut crate::world::pathfinding::PathfindingContext,
) {
    let start_grid = WorldMap::world_to_grid(soul_pos);
    
    // 現在のパスが既に有効（ターゲットの隣接点に向かっている）なら再計算しない
    if !path.waypoints.is_empty() {
        if let Some(last_wp) = path.waypoints.last() {
            let last_grid = WorldMap::world_to_grid(*last_wp);
            // ターゲット領域そのものではなく、「ターゲット領域に隣接しているか」をチェック
            // ただし find_path_to_boundary はターゲット内に入る直前を返すため、
            // ターゲット内の点に隣接している点であればOK
            for &(gx, gy) in occupied_grids {
                let dx = (last_grid.0 - gx).abs();
                let dy = (last_grid.1 - gy).abs();
                // 隣接 (dx<=1, dy<=1) かつ 自分自身はターゲット外（ただし今回はターゲット内通過も許容したロジックなので、
                // 単に「ターゲットの近傍に向かっている」ことでよしとする）
                if dx <= 1 && dy <= 1 {
                    // 有効なパスを持っているので何もしない
                    return;
                }
            }
        }
    }

    // 現在地がすでにゴール条件を満たしているかチェック
    // （中心地との距離などではなく、占有グリッドへの隣接チェック）
    for &(gx, gy) in occupied_grids {
        let grid_pos = WorldMap::grid_to_world(gx, gy);
        if soul_pos.distance(grid_pos) < TILE_SIZE * 1.5 {
            // 到着済み
            return;
        }
    }

    if let Some(grid_path) = crate::world::pathfinding::find_path_to_boundary(
        world_map,
        pf_context,
        start_grid,
        occupied_grids
    ) {
        // パスが見つかった場合、そのパスを採用
        if let Some(last_grid) = grid_path.last() {
             let last_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
             
             // 目的地設定（これが移動の目標）
             update_destination_if_needed(dest, last_pos, path);
             
             // パスウェイポイントを直接上書き
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

    // 管理コンポーネントおよび備蓄状態を削除
    commands
        .entity(item_entity)
        .remove::<crate::systems::jobs::Designation>()
        .remove::<crate::systems::jobs::IssuedBy>()
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
pub fn is_near_blueprint(soul_pos: Vec2, occupied_grids: &[(i32, i32)]) -> bool {
    for &(gx, gy) in occupied_grids {
        let grid_pos = WorldMap::grid_to_world(gx, gy);
        if soul_pos.distance(grid_pos) < TILE_SIZE * 1.5 {
            return true;
        }
    }
    false
}
