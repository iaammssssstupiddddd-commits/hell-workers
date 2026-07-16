//! 駐車エリアへ移動するフェーズ

use super::super::cancel;
use crate::soul_ai::execute::task_execution::types::{
    AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase,
};
use crate::soul_ai::execute::task_execution::{
    common::{NavOutcome, navigate_to_pos},
    context::{TaskExecutionContext, TaskHandlerControl},
};
use bevy::prelude::*;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,

    q_wheelbarrows: &Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<hw_logistics::Wheelbarrow>,
    >,
    soul_pos: Vec2,
) -> TaskHandlerControl {
    let Ok((wb_transform, _)) = q_wheelbarrows.get(data.wheelbarrow) else {
        debug!(
            "WB_HAUL: Wheelbarrow {:?} not found, canceling",
            data.wheelbarrow
        );
        return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
    };

    let wb_pos = wb_transform.translation.truncate();
    match navigate_to_pos(ctx, wb_pos, soul_pos, ctx.env.world_map) {
        NavOutcome::Moving => return TaskHandlerControl::Continue,
        NavOutcome::Deferred => return TaskHandlerControl::Continue,
        NavOutcome::Unreachable => {
            return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
        }
        NavOutcome::Ended(control) => return control,
        NavOutcome::Arrived => {}
    }

    *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        phase: HaulWithWheelbarrowPhase::PickingUpWheelbarrow,
        ..data
    });
    ctx.path.waypoints.clear();

    TaskHandlerControl::Continue
}
