use bevy::prelude::*;
use hw_core::constants::Z_ITEM_PICKUP;
use hw_core::events::{OnTaskAbandoned, ResourceReservationRequest};
use hw_core::relationships::{DeliveringTo, LoadedIn, ParkedAt, PushedBy, StoredIn, TaskWorkers};
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};
use hw_core::visual::WheelbarrowMovement;
use hw_jobs::{AssignedTask, Priority, TargetBlueprint};
use hw_logistics::{Inventory, ResourceType};
use hw_world::WorldMap;

use crate::soul_ai::execute::task_execution::context::TaskReservationAccess;
use hw_jobs::lifecycle;

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
    if matches!(
        idle.behavior,
        IdleBehavior::ExhaustedGathering
            | IdleBehavior::Resting
            | IdleBehavior::GoingToRest
            | IdleBehavior::Escaping
            | IdleBehavior::Drifting
    ) {
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

/// タスク解除時の低レベル cleanup を適用する。
///
/// `AssignedTask` / インベントリ / 予約状態の cleanup を行うが、
/// `WorkingOn` の削除は行わない。公開 API としての `unassign_task` は
/// root crate 側の wrapper が所有する。
pub fn cleanup_task_assignment<'w, 's, Q: TaskReservationAccess<'w, 's>>(
    commands: &mut Commands,
    soul_entity: Entity,
    drop_pos: Vec2,
    task: &mut AssignedTask,
    path: &mut hw_core::soul::Path,
    mut inventory: Option<&mut Inventory>,
    dropped_item_res: Option<ResourceType>,
    queries: &mut Q,
    world_map: &WorldMap,
    emit_abandoned_event: bool,
) {
    if !matches!(*task, AssignedTask::None) && emit_abandoned_event {
        commands.trigger(OnTaskAbandoned {
            entity: soul_entity,
        });
    }

    let (gx, gy) = WorldMap::world_to_grid(drop_pos);
    let drop_grid = if world_map.is_walkable(gx, gy) {
        (gx, gy)
    } else {
        world_map
            .get_nearest_walkable_grid(drop_pos)
            .unwrap_or((gx, gy))
    };
    let snapped_pos = WorldMap::grid_to_world(drop_grid.0, drop_grid.1);

    let mut skip_inventory_drop_for: Option<Entity> = None;

    let release_ops = lifecycle::collect_release_reservation_ops(task, |item, fallback| {
        queries
            .resources()
            .get(item)
            .ok()
            .map(|r| r.0)
            .unwrap_or(fallback)
    });
    for op in release_ops {
        queries
            .reservation_writer()
            .write(ResourceReservationRequest { op });
    }

    if let AssignedTask::HaulWithWheelbarrow(data) = task {
        for &item_entity in &data.items {
            if let Ok(mut entity_commands) = commands.get_entity(item_entity) {
                entity_commands.remove::<LoadedIn>();
                entity_commands.remove::<DeliveringTo>();
                entity_commands.try_insert((
                    Visibility::Visible,
                    Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
                ));
            }
        }
        if let Ok(mut wb_commands) = commands.get_entity(data.wheelbarrow) {
            wb_commands.remove::<(PushedBy, WheelbarrowMovement)>();
            if let Some(parking_entity) = queries.belongs_to(data.wheelbarrow) {
                wb_commands.try_insert(ParkedAt(parking_entity));
            }
            wb_commands.try_insert((
                Visibility::Visible,
                Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
            ));
        }
        skip_inventory_drop_for = Some(data.wheelbarrow);
    }

    if let Some(inventory) = inventory.as_deref_mut() {
        if let Some(item_entity) = inventory.0 {
            if Some(item_entity) != skip_inventory_drop_for {
                commands.entity(item_entity).try_insert((
                    Visibility::Visible,
                    Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
                ));

                let _res_item = dropped_item_res
                    .or_else(|| queries.resources().get(item_entity).ok().map(|r| r.0));

                commands.entity(item_entity).remove::<TargetBlueprint>();
                commands.entity(item_entity).remove::<Priority>();
                commands.entity(item_entity).remove::<TaskWorkers>();
                commands.entity(item_entity).remove::<StoredIn>();
                commands.entity(item_entity).remove::<DeliveringTo>();
            }
        }
        inventory.0 = None;
    }

    *task = AssignedTask::None;
    path.waypoints.clear();
}

/// タスク解除の公開 API。
///
/// 内部で `cleanup_task_assignment` を呼び出し、加えて
/// `OnTaskAbandoned` イベントの発行と `WorkingOn` コンポーネントの削除を行う。
pub fn unassign_task<'w, 's, Q: TaskReservationAccess<'w, 's>>(
    commands: &mut Commands,
    soul_entity: Entity,
    drop_pos: Vec2,
    task: &mut AssignedTask,
    path: &mut hw_core::soul::Path,
    inventory: Option<&mut hw_logistics::Inventory>,
    dropped_item_res: Option<hw_logistics::ResourceType>,
    queries: &mut Q,
    world_map: &WorldMap,
    emit_abandoned_event: bool,
) {
    cleanup_task_assignment(
        commands,
        soul_entity,
        drop_pos,
        task,
        path,
        inventory,
        dropped_item_res,
        queries,
        world_map,
        emit_abandoned_event,
    );

    commands
        .entity(soul_entity)
        .remove::<hw_core::relationships::WorkingOn>();
}
