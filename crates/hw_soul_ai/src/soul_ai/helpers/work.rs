use bevy::prelude::*;
use hw_core::constants::Z_ITEM_PICKUP;
use hw_core::events::{OnTaskAbandoned, ResourceReservationRequest};
use hw_core::relationships::{DeliveringTo, LoadedIn, ParkedAt, PushedBy, StoredIn};
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

/// `cleanup_task_assignment` / `unassign_task` に渡す Soul の位置とインベントリ情報。
pub struct SoulDropCtx<'a> {
    pub soul_entity: Entity,
    pub drop_pos: Vec2,
    pub inventory: Option<&'a mut Inventory>,
    pub dropped_item_res: Option<ResourceType>,
}

/// タスク解除時の低レベル cleanup を適用する。
///
/// `AssignedTask` / インベントリ / 予約状態の cleanup を行うが、
/// `WorkingOn` の削除は行わない。公開 API としての `unassign_task` は
/// root crate 側の wrapper が所有する。
pub fn cleanup_task_assignment<'w, 's, Q: TaskReservationAccess<'w, 's>>(
    commands: &mut Commands,
    ctx: SoulDropCtx<'_>,
    task: &mut AssignedTask,
    path: &mut hw_core::soul::Path,
    queries: &mut Q,
    world_map: &WorldMap,
    emit_abandoned_event: bool,
) {
    let SoulDropCtx {
        soul_entity,
        drop_pos,
        inventory,
        dropped_item_res,
    } = ctx;
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

    if let Some(inventory) = inventory {
        if let Some(item_entity) = inventory.0
            && Some(item_entity) != skip_inventory_drop_for
        {
            commands.entity(item_entity).try_insert((
                Visibility::Visible,
                Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
            ));

            let _res_item =
                dropped_item_res.or_else(|| queries.resources().get(item_entity).ok().map(|r| r.0));

            commands.entity(item_entity).remove::<TargetBlueprint>();
            commands.entity(item_entity).remove::<Priority>();
            // TaskWorkers は RelationshipTarget のため手動で remove してはいけない。
            // WorkingOn を外すと Bevy の関係システムが自動的に管理する。
            commands.entity(item_entity).remove::<StoredIn>();
            commands.entity(item_entity).remove::<DeliveringTo>();
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
    ctx: SoulDropCtx<'_>,
    task: &mut AssignedTask,
    path: &mut hw_core::soul::Path,
    queries: &mut Q,
    world_map: &WorldMap,
    emit_abandoned_event: bool,
) {
    let soul_entity = ctx.soul_entity;
    cleanup_task_assignment(
        commands,
        ctx,
        task,
        path,
        queries,
        world_map,
        emit_abandoned_event,
    );

    commands
        .entity(soul_entity)
        .remove::<hw_core::relationships::WorkingOn>();
}
