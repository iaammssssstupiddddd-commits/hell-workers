//! ReturningBucket phase: バケツを返却場所に戻す（Mixer 経路の終端）

use crate::soul_ai::execute::task_execution::common::is_near_target;
use crate::soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskHandlerControl};
use bevy::prelude::*;

pub fn handle(ctx: &mut TaskExecutionContext, commands: &mut Commands) -> TaskHandlerControl {
    let soul_pos = ctx.soul_pos();

    if is_near_target(soul_pos, ctx.dest.0) {
        let bucket_entity = match ctx.inventory.0 {
            Some(e) => e,
            None => {
                return ctx.abort_closed(commands, "bucket transport returning without bucket");
            }
        };

        return super::super::helpers::drop_bucket_for_auto_haul(
            commands,
            ctx,
            bucket_entity,
            ctx.env.world_map,
        );
    }

    TaskHandlerControl::Continue
}
