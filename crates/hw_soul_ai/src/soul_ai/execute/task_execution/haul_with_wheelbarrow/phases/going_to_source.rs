//! 積み込み元へ移動するフェーズ

use super::super::cancel;
use crate::soul_ai::execute::task_execution::{
    common::{NavOutcome, navigate_to_pos},
    context::TaskExecutionContext,
    types::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use bevy::prelude::*;
use hw_logistics::transport_request::WheelbarrowDestination;
use hw_world::WorldMap;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &WorldMap,
    soul_pos: Vec2,
) {
    match navigate_to_pos(ctx, data.source_pos, soul_pos, world_map) {
        NavOutcome::Moving => return,
        NavOutcome::Unreachable => {
            cancel::cancel_wheelbarrow_task(ctx, &data, commands);
            return;
        }
        _ => {}
    }

    // 搬入先の空き容量チェック
    if let WheelbarrowDestination::Stockpile(stockpile) = data.destination
        && let Ok((_, _, stock, stored_items)) = ctx.queries.storage.stockpiles.get(stockpile) {
            let current_count = stored_items.map(|s| s.len()).unwrap_or(0);
            let incoming = ctx
                .queries
                .reservation
                .incoming_deliveries_query
                .get(stockpile)
                .ok()
                .map(|(_, inc)| inc.len())
                .unwrap_or(0);
            if (current_count + incoming) >= stock.capacity {
                cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                return;
            }
        }

    *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        phase: HaulWithWheelbarrowPhase::Loading,
        ..data
    });
    ctx.path.waypoints.clear();
}
