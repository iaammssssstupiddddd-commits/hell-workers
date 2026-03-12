//! ReturningBucket phase: バケツを返却場所に戻す（Mixer 経路の終端）

use crate::soul_ai::execute::task_execution::common::is_near_target;
use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;
use hw_world::WorldMap;

pub fn handle(ctx: &mut TaskExecutionContext, commands: &mut Commands, world_map: &WorldMap) {
    let soul_pos = ctx.soul_pos();

    if is_near_target(soul_pos, ctx.dest.0) {
        let bucket_entity = match ctx.inventory.0 {
            Some(e) => e,
            None => {
                crate::soul_ai::execute::task_execution::common::clear_task_and_path(
                    ctx.task, ctx.path,
                );
                return;
            }
        };

        super::super::helpers::drop_bucket_for_auto_haul(commands, ctx, bucket_entity, world_map);
    }
}
