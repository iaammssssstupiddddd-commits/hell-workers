//! 手押し車を駐車エリアに返却するフェーズ

use crate::systems::logistics::Wheelbarrow;
use crate::systems::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData;
use crate::systems::soul_ai::execute::task_execution::{
    common::{is_near_target, update_destination_to_adjacent},
    context::TaskExecutionContext,
    transport_common::{reservation, wheelbarrow as wheelbarrow_common},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    q_wheelbarrows: &Query<
        (&Transform, Option<&crate::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
    soul_pos: Vec2,
) {
    let Ok(_) = q_wheelbarrows.get(data.wheelbarrow) else {
        reservation::release_source(ctx, data.wheelbarrow, 1);
        ctx.inventory.0 = None;
        if let Ok(mut soul_commands) = commands.get_entity(ctx.soul_entity) {
            soul_commands.try_remove::<crate::relationships::WorkingOn>();
        }
        crate::systems::soul_ai::execute::task_execution::common::clear_task_and_path(
            ctx.task, ctx.path,
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
        world_map,
        ctx.pf_context,
    );

    if !reachable {
        reservation::release_source(ctx, data.wheelbarrow, 1);
        wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, soul_pos);
        info!(
            "WB_HAUL: Soul {:?} returned wheelbarrow {:?} (unreachable, parked here)",
            ctx.soul_entity, data.wheelbarrow
        );
        return;
    }

    if is_near_target(soul_pos, parking_pos) {
        reservation::release_source(ctx, data.wheelbarrow, 1);
        wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, parking_pos);
        info!(
            "WB_HAUL: Soul {:?} returned wheelbarrow {:?}",
            ctx.soul_entity, data.wheelbarrow
        );
    }
}
