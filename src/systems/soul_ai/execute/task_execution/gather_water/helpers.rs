//! Helper functions for water gathering task

use crate::systems::soul_ai::task_execution::context::TaskExecutionContext;
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// バケツをドロップしてオートホールに任せるヘルパー関数
/// タンクが満タンになった場合や、水汲み完了後に使用
pub fn drop_bucket_for_auto_haul(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    _tank_entity: Entity,
    // haul_cache removed
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();
    let drop_grid = WorldMap::world_to_grid(soul_pos);
    let drop_pos = WorldMap::grid_to_world(drop_grid.0, drop_grid.1);

    commands.entity(bucket_entity).insert((
        Visibility::Visible,
        Transform::from_xyz(drop_pos.x, drop_pos.y, crate::constants::Z_ITEM_PICKUP),
    ));
    commands
        .entity(bucket_entity)
        .remove::<crate::relationships::StoredIn>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::logistics::InStockpile>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::IssuedBy>();
    commands
        .entity(bucket_entity)
        .remove::<crate::relationships::TaskWorkers>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::Designation>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::TaskSlots>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::TargetMixer>();

    ctx.inventory.0 = None;
    crate::systems::soul_ai::work::unassign_task(
        commands,
        ctx.soul_entity,
        soul_pos,
        ctx.task,
        ctx.path,
        None,
        None,
        ctx.queries,
        world_map,
        false,
    );
}

/// タスクを中断する（インベントリにアイテムがない場合）
/// バケツがインベントリにない状態でのタスク中断時に使用
pub fn abort_task_without_item(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    // haul_cache removed
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();
    crate::systems::soul_ai::work::unassign_task(
        commands,
        ctx.soul_entity,
        soul_pos,
        ctx.task,
        ctx.path,
        None,
        None,
        ctx.queries,
        world_map,
        true,
    );
}

/// タスクを中断する（インベントリにアイテムがある場合）
/// 経路探索失敗やターゲット消失などのエラー時に使用
pub fn abort_task_with_item(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    // haul_cache removed
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();
    crate::systems::soul_ai::work::unassign_task(
        commands,
        ctx.soul_entity,
        soul_pos,
        ctx.task,
        ctx.path,
        Some(ctx.inventory),
        None,
        ctx.queries,
        world_map,
        true,
    );
}
