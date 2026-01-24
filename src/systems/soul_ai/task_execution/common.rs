//! タスク実行の共通処理

use crate::constants::*;
use crate::entities::damned_soul::{Destination, Path};
use crate::relationships::Holding;
use crate::systems::jobs::Designation;
use crate::systems::logistics::Stockpile;
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
    
    // 隣接マスのうち、Walkableで現在位置に最も近いものを探す（4方向のみ）
    let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    
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
pub fn pickup_item(commands: &mut Commands, soul_entity: Entity, item_entity: Entity) {
    commands.entity(soul_entity).insert(Holding(item_entity));
    commands.entity(item_entity).insert(Visibility::Hidden);
}

/// アイテムをドロップ
///
/// 魂からアイテムを外し、指定位置にアイテムを表示します。
pub fn drop_item(
    commands: &mut Commands,
    soul_entity: Entity,
    item_entity: Entity,
    drop_pos: Vec2,
) {
    commands.entity(soul_entity).remove::<Holding>();
    commands.entity(item_entity).insert((
        Visibility::Visible,
        Transform::from_xyz(drop_pos.x, drop_pos.y, Z_ITEM_PICKUP),
    ));
}

/// ストックパイルからアイテムが取り出された際の更新処理
///
/// ストックパイルが空になった場合、リソースタイプをリセットします。
pub fn update_stockpile_on_item_removal(
    stock_entity: Entity,
    q_stockpiles: &mut Query<(
        &Transform,
        &mut Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
) {
    if let Ok((_, mut stock_comp, Some(stored_items))) = q_stockpiles.get_mut(stock_entity) {
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
/// 4方向隣接（1タイル＝32px）をカバーするため、タイルサイズの1.5倍を閾値に設定。
/// これにより、隣接マス（中心間距離32px）からでもターゲットに「近い」と判定される。
pub fn is_near_target(soul_pos: Vec2, target_pos: Vec2) -> bool {
    soul_pos.distance(target_pos) < TILE_SIZE * 1.5
}
