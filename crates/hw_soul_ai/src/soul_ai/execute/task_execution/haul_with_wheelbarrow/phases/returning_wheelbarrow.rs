//! 手押し車を駐車エリアに返却するフェーズ

use crate::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData;
use crate::soul_ai::execute::task_execution::{
    common::{is_near_target, update_task_destination_to_adjacent},
    context::{TaskExecutionContext, TaskHandlerControl},
    transport_common::{reservation, wheelbarrow as wheelbarrow_common},
};
use bevy::prelude::*;
use hw_logistics::Wheelbarrow;
use hw_world::PathSearchResult;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,

    q_wheelbarrows: &Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
    soul_pos: Vec2,
) -> TaskHandlerControl {
    let Ok(_) = q_wheelbarrows.get(data.wheelbarrow) else {
        ctx.inventory.0 = None;
        return ctx.abort_retryable_after_custom_cleanup(
            commands,
            "wheelbarrow disappeared while returning",
        );
    };

    let parking_pos = ctx
        .queries
        .designation
        .belongs
        .get(data.wheelbarrow)
        .ok()
        .and_then(|belongs| {
            ctx.queries
                .designation
                .targets
                .get(belongs.0)
                .ok()
                .map(|(tf, _, _, _, _, _, _)| tf.translation.truncate())
        })
        .unwrap_or(soul_pos);

    match update_task_destination_to_adjacent(ctx, parking_pos) {
        PathSearchResult::Found(()) => {}
        PathSearchResult::Deferred => return TaskHandlerControl::Continue,
        PathSearchResult::Unreachable => {
            reservation::release_source(ctx, data.wheelbarrow, 1);
            debug!(
                "WB_HAUL: Soul {:?} returned wheelbarrow {:?} (unreachable, parked here)",
                ctx.soul_entity, data.wheelbarrow
            );
            return wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, soul_pos);
        }
    }

    if is_near_target(soul_pos, parking_pos) {
        reservation::release_source(ctx, data.wheelbarrow, 1);
        debug!(
            "WB_HAUL: Soul {:?} returned wheelbarrow {:?}",
            ctx.soul_entity, data.wheelbarrow
        );
        return wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, parking_pos);
    }

    TaskHandlerControl::Continue
}
