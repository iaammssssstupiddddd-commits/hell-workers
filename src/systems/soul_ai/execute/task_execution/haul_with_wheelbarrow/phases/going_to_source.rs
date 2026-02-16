//! 積み込み元へ移動するフェーズ

use super::super::cancel;
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use crate::systems::soul_ai::execute::task_execution::{
    common::{is_near_target, update_destination_to_adjacent},
    context::TaskExecutionContext,
    types::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    soul_pos: Vec2,
) {
    let reachable = update_destination_to_adjacent(
        ctx.dest,
        data.source_pos,
        ctx.path,
        soul_pos,
        world_map,
        ctx.pf_context,
    );

    if !reachable {
        cancel::cancel_wheelbarrow_task(ctx, &data, commands);
        return;
    }

    if is_near_target(soul_pos, data.source_pos) {
        // 搬入先の空き容量チェック
        if let WheelbarrowDestination::Stockpile(stockpile) = data.destination {
            if let Ok((_, _, stock, stored_items)) = ctx.queries.storage.stockpiles.get(stockpile) {
                let current_count = stored_items.map(|s| s.len()).unwrap_or(0);
                let incoming = ctx
                    .queries
                    .reservation
                    .incoming_deliveries_query
                    .get(stockpile)
                    .ok()
                    .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
                    .unwrap_or(0);
                if (current_count + incoming) >= stock.capacity {
                    cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                    return;
                }
            }
        }

        *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
            phase: HaulWithWheelbarrowPhase::Loading,
            ..data
        });
        ctx.path.waypoints.clear();
    }
}
