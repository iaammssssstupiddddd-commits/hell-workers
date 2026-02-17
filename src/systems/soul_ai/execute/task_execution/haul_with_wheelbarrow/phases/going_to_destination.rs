//! 目的地へ移動するフェーズ

use super::super::cancel;
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use crate::systems::soul_ai::execute::task_execution::{
    common::{
        is_near_blueprint, is_near_target, is_near_target_or_dest, update_destination_to_adjacent,
        update_destination_to_blueprint,
    },
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
    let (reachable, arrived) = match data.destination {
        WheelbarrowDestination::Stockpile(stockpile_entity) => {
            if let Ok((_, stock_transform, _, _)) =
                ctx.queries.storage.stockpiles.get(stockpile_entity)
            {
                let stock_pos = stock_transform.translation.truncate();
                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    stock_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );
                (reachable, is_near_target(soul_pos, stock_pos))
            } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(stockpile_entity) {
                let site_pos = site.material_center;
                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    site_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );
                (
                    reachable,
                    is_near_target_or_dest(soul_pos, site_pos, ctx.dest.0),
                )
            } else {
                info!("WB_HAUL: Destination stockpile/floor-site not found, canceling");
                cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                return;
            }
        }
        WheelbarrowDestination::Blueprint(blueprint_entity) => {
            let Ok((_, blueprint, _)) = ctx.queries.storage.blueprints.get(blueprint_entity) else {
                info!("WB_HAUL: Destination blueprint destroyed, dropping items");
                cancel::drop_items_and_cancel(ctx, &data, commands);
                return;
            };

            let reachable = update_destination_to_blueprint(
                ctx.dest,
                &blueprint.occupied_grids,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );
            (
                reachable,
                is_near_blueprint(soul_pos, &blueprint.occupied_grids),
            )
        }
        WheelbarrowDestination::Mixer { entity, .. } => {
            let Ok((mixer_transform, _, _)) = ctx.queries.storage.mixers.get(entity) else {
                info!("WB_HAUL: Destination mixer not found, dropping items");
                cancel::drop_items_and_cancel(ctx, &data, commands);
                return;
            };

            let mixer_pos = mixer_transform.translation.truncate();
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                mixer_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );
            (
                reachable,
                is_near_target_or_dest(soul_pos, mixer_pos, ctx.dest.0),
            )
        }
    };

    if !reachable {
        cancel::cancel_wheelbarrow_task(ctx, &data, commands);
        return;
    }

    if arrived {
        *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
            phase: HaulWithWheelbarrowPhase::Unloading,
            ..data
        });
        ctx.path.waypoints.clear();
    }
}
