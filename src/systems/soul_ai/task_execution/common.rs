//! タスク実行の共通処理

use crate::constants::*;
use crate::entities::damned_soul::{Destination, Path};
use crate::relationships::Holding;
use crate::systems::jobs::Designation;
use crate::systems::logistics::Stockpile;
use crate::systems::soul_ai::task_execution::types::AssignedTask;
use bevy::prelude::*;

/// 目的地を更新（必要に応じて）
/// 
/// 目的地が2.0以上離れている場合にのみ更新します。
pub fn update_destination_if_needed(
    dest: &mut Destination,
    target_pos: Vec2,
    path: &mut Path,
) {
    if dest.0.distance_squared(target_pos) > 2.0 {
        dest.0 = target_pos;
        path.waypoints.clear();
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
    soul_entity: Entity,
    item_entity: Entity,
) {
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
        Transform::from_xyz(drop_pos.x, drop_pos.y, 0.6),
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
/// `TILE_SIZE * 1.2`以内にいる場合、`true`を返します。
pub fn is_near_target(soul_pos: Vec2, target_pos: Vec2) -> bool {
    soul_pos.distance(target_pos) < TILE_SIZE * 1.2
}
