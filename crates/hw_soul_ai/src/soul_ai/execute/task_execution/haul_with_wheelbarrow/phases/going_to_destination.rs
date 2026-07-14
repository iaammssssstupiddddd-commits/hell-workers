//! 目的地へ移動するフェーズ

use super::super::cancel;
use crate::soul_ai::execute::task_execution::{
    common::{NavOutcome, is_near_blueprint, navigate_to_pos, update_destination_to_blueprint},
    context::{TaskExecutionContext, TaskHandlerControl},
    types::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use bevy::prelude::*;
use hw_logistics::transport_request::WheelbarrowDestination;

fn navigation_progress(outcome: NavOutcome) -> Result<(bool, bool), TaskHandlerControl> {
    match outcome {
        NavOutcome::Moving => Ok((true, false)),
        NavOutcome::Arrived => Ok((true, true)),
        NavOutcome::Unreachable => Ok((false, false)),
        NavOutcome::Ended(control) => Err(control),
    }
}

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    soul_pos: Vec2,
) -> TaskHandlerControl {
    let (reachable, arrived) = match data.destination {
        WheelbarrowDestination::Stockpile(stockpile_entity) => {
            if let Ok((_, stock_transform, _, _)) =
                ctx.queries.storage.stockpiles.get(stockpile_entity)
            {
                let stock_pos = stock_transform.translation.truncate();
                match navigation_progress(navigate_to_pos(
                    ctx,
                    stock_pos,
                    soul_pos,
                    ctx.env.world_map,
                )) {
                    Ok(progress) => progress,
                    Err(control) => return control,
                }
            } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(stockpile_entity) {
                let site_pos = site.material_center;
                match navigation_progress(navigate_to_pos(
                    ctx,
                    site_pos,
                    soul_pos,
                    ctx.env.world_map,
                )) {
                    Ok(progress) => progress,
                    Err(control) => return control,
                }
            } else if let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(stockpile_entity) {
                let site_pos = site.material_center;
                match navigation_progress(navigate_to_pos(
                    ctx,
                    site_pos,
                    soul_pos,
                    ctx.env.world_map,
                )) {
                    Ok(progress) => progress,
                    Err(control) => return control,
                }
            } else if let Ok(soul_spa_transform) =
                ctx.queries.storage.soul_spa_sites.get(stockpile_entity)
            {
                // SoulSpaSite は Building コンポーネントも持つため、buildings チェックより先に処理する。
                let site_pos = soul_spa_transform.translation.truncate();
                match navigation_progress(navigate_to_pos(
                    ctx,
                    site_pos,
                    soul_pos,
                    ctx.env.world_map,
                )) {
                    Ok(progress) => progress,
                    Err(control) => return control,
                }
            } else if let Ok((wall_transform, building, _)) =
                ctx.queries.storage.buildings.get(stockpile_entity)
            {
                if building.kind == hw_jobs::BuildingType::Wall && building.is_provisional {
                    let site_pos = wall_transform.translation.truncate();
                    match navigation_progress(navigate_to_pos(
                        ctx,
                        site_pos,
                        soul_pos,
                        ctx.env.world_map,
                    )) {
                        Ok(progress) => progress,
                        Err(control) => return control,
                    }
                } else {
                    debug!("WB_HAUL: Destination stockpile/site not found, canceling");
                    return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                }
            } else {
                debug!("WB_HAUL: Destination stockpile/site not found, canceling");
                return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
            }
        }
        WheelbarrowDestination::Blueprint(blueprint_entity) => {
            let Ok((_, blueprint, _)) = ctx.queries.storage.blueprints.get(blueprint_entity) else {
                debug!("WB_HAUL: Destination blueprint destroyed, dropping items");
                return cancel::drop_items_and_cancel(ctx, &data, commands);
            };

            let reachable = update_destination_to_blueprint(
                ctx.dest,
                &blueprint.occupied_grids,
                ctx.path,
                soul_pos,
                ctx.env.world_map,
                ctx.pf_context,
            );
            (
                reachable,
                is_near_blueprint(soul_pos, &blueprint.occupied_grids),
            )
        }
        WheelbarrowDestination::Mixer { entity, .. } => {
            let Ok((mixer_transform, _, _)) = ctx.queries.storage.mixers.get(entity) else {
                debug!("WB_HAUL: Destination mixer not found, dropping items");
                return cancel::drop_items_and_cancel(ctx, &data, commands);
            };

            let mixer_pos = mixer_transform.translation.truncate();
            match navigation_progress(navigate_to_pos(ctx, mixer_pos, soul_pos, ctx.env.world_map))
            {
                Ok(progress) => progress,
                Err(control) => return control,
            }
        }
    };

    if !reachable {
        return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
    }

    if arrived {
        *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
            phase: HaulWithWheelbarrowPhase::Unloading,
            ..data
        });
        ctx.path.waypoints.clear();
    }

    TaskHandlerControl::Continue
}
