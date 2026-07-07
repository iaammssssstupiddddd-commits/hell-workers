//! 手押し車を駐車エリアに返却するフェーズ

use crate::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData;
use crate::soul_ai::execute::task_execution::{
    common::{is_near_target, update_destination_to_adjacent},
    context::TaskExecutionContext,
    transport_common::{reservation, wheelbarrow as wheelbarrow_common},
};
use bevy::prelude::*;
use hw_logistics::Wheelbarrow;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    
    q_wheelbarrows: &Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
    soul_pos: Vec2,
) {
    let Ok(_) = q_wheelbarrows.get(data.wheelbarrow) else {
        reservation::release_source(ctx, data.wheelbarrow, 1);
        ctx.inventory.0 = None;
        ctx.clear_soul_assignment(
            commands,
            crate::soul_ai::execute::task_execution::context::TaskEndDisposition::AbortedRetryable,
        );
        return;
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

    let reachable = update_destination_to_adjacent(
        ctx.dest,
        parking_pos,
        ctx.path,
        soul_pos,
        ctx.env.world_map,
        ctx.pf_context,
    );

    if !reachable {
        reservation::release_source(ctx, data.wheelbarrow, 1);
        wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, soul_pos);
        debug!(
            "WB_HAUL: Soul {:?} returned wheelbarrow {:?} (unreachable, parked here)",
            ctx.soul_entity, data.wheelbarrow
        );
        return;
    }

    if is_near_target(soul_pos, parking_pos) {
        reservation::release_source(ctx, data.wheelbarrow, 1);
        wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, parking_pos);
        debug!(
            "WB_HAUL: Soul {:?} returned wheelbarrow {:?}",
            ctx.soul_entity, data.wheelbarrow
        );
    }
}
