//! 駐車エリアへ移動するフェーズ

use crate::systems::soul_ai::execute::task_execution::{
    common::{is_near_target, update_destination_to_adjacent},
    context::TaskExecutionContext,
};
use super::super::cancel;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    q_wheelbarrows: &Query<(&Transform, Option<&crate::relationships::ParkedAt>), With<crate::systems::logistics::Wheelbarrow>>,
    soul_pos: Vec2,
) {
    let Ok((wb_transform, _)) = q_wheelbarrows.get(data.wheelbarrow) else {
        info!(
            "WB_HAUL: Wheelbarrow {:?} not found, canceling",
            data.wheelbarrow
        );
        cancel::cancel_wheelbarrow_task(ctx, &data, commands);
        return;
    };

    let wb_pos = wb_transform.translation.truncate();
    let reachable = update_destination_to_adjacent(
        ctx.dest,
        wb_pos,
        ctx.path,
        soul_pos,
        world_map,
        ctx.pf_context,
    );

    if !reachable {
        cancel::cancel_wheelbarrow_task(ctx, &data, commands);
        return;
    }

    if is_near_target(soul_pos, wb_pos) {
        *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
            phase: HaulWithWheelbarrowPhase::PickingUpWheelbarrow,
            ..data
        });
        ctx.path.waypoints.clear();
    }
}
