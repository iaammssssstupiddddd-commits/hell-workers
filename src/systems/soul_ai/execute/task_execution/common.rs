//! タスク実行の共通処理

use crate::constants::*;
use crate::entities::damned_soul::{Destination, Path};
use crate::systems::jobs::Designation;
use crate::systems::logistics::{Inventory, ReservedForTask, Stockpile};
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
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
/// 到達可能な隣接マスがあれば`true`を返し、なければ`false`を返す。
/// 実際の経路探索で到達可能か確認し、最も到達コストが小さい隣接マスを目的地として設定する。
pub fn update_destination_to_adjacent(
    dest: &mut Destination,
    target_pos: Vec2,
    path: &mut Path,
    soul_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut crate::world::pathfinding::PathfindingContext,
) -> bool {
    let target_grid = WorldMap::world_to_grid(target_pos);
    let start_grid = WorldMap::world_to_grid(soul_pos);

    // すでに有効なパスがあり、目的地も変わっていないならスキップ
    if !path.waypoints.is_empty() && path.current_index < path.waypoints.len() {
        if let Some(last_wp) = path.waypoints.last() {
            let last_grid = WorldMap::world_to_grid(*last_wp);
            // 終点がターゲットに隣接していれば、そのパスは有効
            let dx = (last_grid.0 - target_grid.0).abs();
            let dy = (last_grid.1 - target_grid.1).abs();
            if dx <= 1 && dy <= 1 {
                // 目的地をパスの終点に更新（is_near_target_or_destで正しく判定するため）
                dest.0 = *last_wp;
                return true;
            }
        }
    }

    // ターゲット自体がWalkableなら、そのまま直接移動を試みる
    if world_map.is_walkable(target_grid.0, target_grid.1) {
        // 直接の経路があればそれを使用
        if let Some(grid_path) =
            crate::world::pathfinding::find_path(world_map, pf_context, start_grid, target_grid)
        {
            if let Some(&last_grid) = grid_path.last() {
                let dest_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
                // 必ず目的地を更新（近くても変更検知のため）
                dest.0 = dest_pos;
                // 経路を設定
                path.waypoints = grid_path
                    .iter()
                    .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                    .collect();
                path.current_index = 0;
            }
            return true;
        }
    }

    // 最も到達コストが小さい隣接マスを見つける
    let directions = [
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    let mut best_path: Option<Vec<(i32, i32)>> = None;
    let mut best_cost = i32::MAX;

    for (dx, dy) in directions {
        let nx = target_grid.0 + dx;
        let ny = target_grid.1 + dy;

        // 隣接マスが歩行可能かチェック
        if !world_map.is_walkable(nx, ny) {
            continue;
        }

        // 開始点からこの隣接マスへの経路を探索
        if let Some(grid_path) =
            crate::world::pathfinding::find_path(world_map, pf_context, start_grid, (nx, ny))
        {
            // 経路コストを計算（パスの長さで近似）
            let cost = grid_path.len() as i32;
            if cost < best_cost {
                best_cost = cost;
                best_path = Some(grid_path);
            }
        }
    }

    if let Some(grid_path) = best_path {
        if let Some(&last_grid) = grid_path.last() {
            let dest_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
            // 必ず目的地を更新（近くても変更検知のため）
            dest.0 = dest_pos;
        }
        // 経路を設定
        path.waypoints = grid_path
            .iter()
            .map(|&(x, y)| WorldMap::grid_to_world(x, y))
            .collect();
        path.current_index = 0;
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
        occupied_grids,
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
        .remove::<ReservedForTask>()
        .remove::<crate::relationships::StoredIn>()
        .remove::<crate::systems::logistics::InStockpile>();
}

/// アイテムを地面に落とす
pub fn drop_item(commands: &mut Commands, _soul_entity: Entity, item_entity: Entity, pos: Vec2) {
    commands
        .entity(item_entity)
        .insert((Visibility::Visible, Transform::from_xyz(pos.x, pos.y, 0.6)));
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

/// MudMixer 在庫として保持していた mud アイテムが持ち出された際の在庫解放
pub fn release_mixer_mud_storage_for_item(
    ctx: &mut crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext,
    item_entity: Entity,
    commands: &mut Commands,
) {
    let Ok(stored_by_mixer) = ctx.queries.mixer_stored_mud.get(item_entity) else {
        return;
    };

    if let Ok((_, mut mixer_storage, _)) = ctx.queries.storage.mixers.get_mut(stored_by_mixer.0) {
        mixer_storage.mud = mixer_storage.mud.saturating_sub(1);
    }

    commands
        .entity(item_entity)
        .remove::<crate::systems::jobs::StoredByMixer>();
}

/// 距離チェック: 魂がターゲットに近づいたかどうか
///
/// 隣接マス（中心間距離32px）からでも確実に「近い」と判定されるように、
/// タイルサイズの1.5倍（48px）を閾値に設定。
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

/// 拾い判定が満たされない場合はタスクをクリアする
pub fn try_pickup_item(
    commands: &mut Commands,
    soul_entity: Entity,
    item_entity: Entity,
    inventory: &mut Inventory,
    soul_pos: Vec2,
    item_pos: Vec2,
    task: &mut AssignedTask,
    path: &mut Path,
) -> bool {
    if !can_pickup_item(soul_pos, item_pos) {
        clear_task_and_path(task, path);
        return false;
    }
    pickup_item(commands, soul_entity, item_entity, inventory);
    true
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
