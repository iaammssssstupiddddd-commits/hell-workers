//! ReturningBucket phase: Return bucket to tank storage

use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::gather_water::helpers::drop_bucket_for_auto_haul;
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    // 目的地（バケツ置き場）に到着したら終了
    if is_near_target(soul_pos, ctx.dest.0) {
        drop_bucket_for_auto_haul(commands, ctx, bucket_entity, tank_entity, world_map);
    }
}
