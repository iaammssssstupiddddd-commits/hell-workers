//! バケツ搬送共通ヘルパー

use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::transport_common::cancel;
use crate::world::map::WorldMap;
use bevy::prelude::*;

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
    crate::systems::soul_ai::helpers::work::unassign_task(
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
