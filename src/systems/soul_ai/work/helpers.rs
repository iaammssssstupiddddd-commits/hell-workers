use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState, Path};
use crate::relationships::{TaskWorkers, WorkingOn};
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::jobs::{Designation, IssuedBy, TaskSlots};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// 魂が作業可能な状態（待機中かつ健康）であるかを確認する
pub fn is_soul_available_for_work(
    soul: &DamnedSoul,
    task: &AssignedTask,
    idle: &IdleState,
    has_breakdown: bool,
    fatigue_threshold: f32,
) -> bool {
    if !matches!(*task, AssignedTask::None) {
        return false;
    }
    if idle.behavior == IdleBehavior::ExhaustedGathering {
        return false;
    }
    if soul.fatigue >= fatigue_threshold {
        return false;
    }
    if has_breakdown {
        return false;
    }
    true
}

/// 魂からタスクの割り当てを解除し、スロットを解放する。
///
/// ソウル側のみを処理し、タスク側（Designation, IssuedBy）には触らない。
/// 使い魔がスロットの空きを検知して別のソウルに再アサインする。
pub fn unassign_task(
    commands: &mut Commands,
    soul_entity: Entity,
    drop_pos: Vec2,
    task: &mut AssignedTask,
    path: &mut Path,
    mut inventory: Option<&mut crate::systems::logistics::Inventory>,
    _q_designations: &Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    haul_cache: &mut HaulReservationCache,
    emit_abandoned_event: bool,
) {
    // タスク中断イベントを発火
    if !matches!(*task, AssignedTask::None) && emit_abandoned_event {
        commands.trigger(crate::events::OnTaskAbandoned {
            entity: soul_entity,
        });
    }

    // 運搬タスクの備蓄場所予約を解除
    if let AssignedTask::Haul { stockpile, .. } = *task {
        haul_cache.release(stockpile);
    }

    // アイテムのドロップ処理（運搬タスクの場合）
    if let Some(inventory) = inventory.as_deref_mut() {
        if let Some(item_entity) = inventory.0 {
            // インベントリから削除
            // 注意: inventoryは可変参照なので直接書き換える
            // しかし、呼び出し元でinventory.0 = Noneする必要があるため、ここではドロップ処理のみ行う
            // あるいはここでinventory.0 = Noneする?
            // argument is Option<&mut Inventory>. 
            
            let grid = WorldMap::world_to_grid(drop_pos);
            let snapped_pos = WorldMap::grid_to_world(grid.0, grid.1);

            // クリーンな状態でドロップ（Designation なし）
            commands.entity(item_entity).insert((
                Visibility::Visible,
                Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
            ));
            // 既存のタスク関連コンポーネントを削除
            commands.entity(item_entity).remove::<Designation>();
            commands.entity(item_entity).remove::<IssuedBy>();
            commands.entity(item_entity).remove::<TaskSlots>();
            commands
                .entity(item_entity)
                .remove::<crate::systems::jobs::TargetBlueprint>();
            commands
                .entity(item_entity)
                .remove::<crate::systems::jobs::Priority>();

            // StoredIn関係も削除
            commands.entity(item_entity).remove::<crate::relationships::StoredIn>();
            
            info!(
                "UNASSIGN: Soul dropped item {:?} (clean state for auto-haul)",
                item_entity
            );
        }
    }

    // インベントリを空にする（ドロップしたとみなす）
    if let Some(inventory) = inventory {
        inventory.0 = None;
    }

    // ソウルからタスクを解除
    commands.entity(soul_entity).remove::<WorkingOn>();

    *task = AssignedTask::None;
    path.waypoints.clear();

    info!("UNASSIGN: Soul {:?} unassigned from task", soul_entity);
}
