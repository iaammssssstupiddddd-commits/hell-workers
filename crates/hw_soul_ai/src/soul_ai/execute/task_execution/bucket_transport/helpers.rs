//! バケツ搬送共通ヘルパー

use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::soul_ai::execute::task_execution::transport_common::cancel;
use bevy::prelude::*;
use hw_world::WorldMap;

/// バケツをドロップして auto haul タスクに任せる。
/// River→Tank 経路の「タンク満杯」や「搬送完了」後に使用。
pub fn drop_bucket_for_auto_haul(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();
    cancel::drop_bucket_with_cleanup(commands, bucket_entity, soul_pos);
    ctx.inventory.0 = None;
    crate::soul_ai::helpers::work::cleanup_task_assignment(
        commands,
        crate::soul_ai::helpers::work::SoulDropCtx {
            soul_entity: ctx.soul_entity,
            drop_pos: soul_pos,
            inventory: None,
            dropped_item_res: None,
        },
        ctx.task,
        ctx.path,
        ctx.queries,
        world_map,
        false,
    );
}
