//! 目的地へ移動するフェーズ

use super::super::cancel;
use crate::soul_ai::execute::task_execution::{
    common::{NavOutcome, is_near_blueprint, navigate_to_pos, update_destination_to_blueprint},
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
    let (reachable, arrived) = match data.destination {
        WheelbarrowDestination::Stockpile(stockpile_entity) => {
            if let Ok((_, stock_transform, _, _)) =
                ctx.queries.storage.stockpiles.get(stockpile_entity)
            {
                let stock_pos = stock_transform.translation.truncate();
                let outcome = navigate_to_pos(ctx, stock_pos, soul_pos, world_map);
                (
                    !matches!(outcome, NavOutcome::Unreachable),
                    matches!(outcome, NavOutcome::Arrived),
                )
            } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(stockpile_entity) {
                let site_pos = site.material_center;
                let outcome = navigate_to_pos(ctx, site_pos, soul_pos, world_map);
                (
                    !matches!(outcome, NavOutcome::Unreachable),
                    matches!(outcome, NavOutcome::Arrived),
                )
            } else if let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(stockpile_entity) {
                let site_pos = site.material_center;
                let outcome = navigate_to_pos(ctx, site_pos, soul_pos, world_map);
                (
                    !matches!(outcome, NavOutcome::Unreachable),
                    matches!(outcome, NavOutcome::Arrived),
                )
            } else if let Ok((wall_transform, building, _)) =
                ctx.queries.storage.buildings.get(stockpile_entity)
            {
                if building.kind == hw_jobs::BuildingType::Wall && building.is_provisional {
                    let site_pos = wall_transform.translation.truncate();
                    let outcome = navigate_to_pos(ctx, site_pos, soul_pos, world_map);
                    (
                        !matches!(outcome, NavOutcome::Unreachable),
                        matches!(outcome, NavOutcome::Arrived),
                    )
                } else {
                    info!("WB_HAUL: Destination stockpile/site not found, canceling");
                    cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                    return;
                }
            } else {
                info!("WB_HAUL: Destination stockpile/site not found, canceling");
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
            let outcome = navigate_to_pos(ctx, mixer_pos, soul_pos, world_map);
            (
                !matches!(outcome, NavOutcome::Unreachable),
                matches!(outcome, NavOutcome::Arrived),
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
