//! 運搬タスクの実行処理（ストックパイルへ）

use crate::soul_ai::execute::task_execution::common::*;
use crate::soul_ai::execute::task_execution::transport_common::{cancel, reservation};
use crate::soul_ai::execute::task_execution::{
    context::{TaskExecutionContext, TaskHandlerControl},
    types::{AssignedTask, HaulData, HaulPhase},
};
use bevy::prelude::*;
use hw_jobs::BuildingType;
use hw_world::WorldMap;

mod dropping;

use dropping::handle_dropping_phase;

fn set_haul_phase(task: &mut AssignedTask, item: Entity, stockpile: Entity, phase: HaulPhase) {
    *task = AssignedTask::Haul(HaulData {
        item,
        stockpile,
        phase,
    });
}

fn cancel_haul_with_reason(
    ctx: &mut TaskExecutionContext,
    item: Entity,
    stockpile: Entity,
    commands: &mut Commands,
    reason: &str,
) -> TaskHandlerControl {
    debug!(
        "HAUL: Soul {:?} {}, canceling (item={:?}, stockpile={:?})",
        ctx.soul_entity, reason, item, stockpile
    );
    cancel::cancel_haul_to_stockpile(ctx, item, stockpile, commands)
}

pub fn handle_haul_task(
    ctx: &mut TaskExecutionContext,
    data: HaulData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let HaulData {
        item,
        stockpile,
        phase,
    } = data;
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.designation.targets;
    match phase {
        HaulPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _, _res_item_opt, _, stored_in_opt)) =
                q_targets.get(item)
            {
                let res_pos = res_transform.translation.truncate();
                let stored_in_entity = stored_in_opt.map(|stored_in| stored_in.0);
                match navigate_to_pos(ctx, res_pos, soul_pos, ctx.env.world_map) {
                    NavOutcome::Moving => {}
                    NavOutcome::Ended(control) => return control,
                    NavOutcome::Deferred => return TaskHandlerControl::Continue,
                    NavOutcome::Unreachable => {
                        return cancel_haul_with_reason(
                            ctx,
                            item,
                            stockpile,
                            commands,
                            "cannot reach pickup item",
                        );
                    }
                    NavOutcome::Arrived => {
                        if !can_pickup_item(soul_pos, res_pos) {
                            return TaskHandlerControl::Continue;
                        }
                        pickup_item(commands, ctx.soul_entity, item, &mut ctx.inventory);
                        release_mixer_mud_storage_for_item(ctx, item, commands);

                        if let Some(stored_in) = stored_in_entity {
                            update_stockpile_on_item_removal(
                                stored_in,
                                &mut ctx.queries.storage.stockpiles,
                            );
                        }

                        if let Ok((_, stock_transform, _, _)) =
                            ctx.queries.storage.stockpiles.get(stockpile)
                        {
                            let stock_pos = stock_transform.translation.truncate();
                            let stock_grid = WorldMap::world_to_grid(stock_pos);
                            let stock_dest = WorldMap::grid_to_world(stock_grid.0, stock_grid.1);
                            ctx.path.waypoints.clear();
                            update_destination_if_needed(&mut ctx.dest, stock_dest, &mut ctx.path);
                        }

                        set_haul_phase(&mut ctx.task, item, stockpile, HaulPhase::GoingToStockpile);
                        reservation::record_picked_source(ctx, item, 1);
                        debug!("HAUL: Soul {:?} picked up item {:?}", ctx.soul_entity, item);
                    }
                }
            } else {
                return cancel_haul_with_reason(
                    ctx,
                    item,
                    stockpile,
                    commands,
                    "pickup item disappeared",
                );
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok((_, stock_transform, _, _)) = ctx.queries.storage.stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                let stock_grid = WorldMap::world_to_grid(stock_pos);
                let stock_dest = WorldMap::grid_to_world(stock_grid.0, stock_grid.1);
                update_destination_if_needed(&mut ctx.dest, stock_dest, &mut ctx.path);

                if is_near_target(soul_pos, stock_pos) {
                    set_haul_phase(&mut ctx.task, item, stockpile, HaulPhase::Dropping);
                    ctx.path.waypoints.clear();
                }
            } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(stockpile) {
                let site_pos = site.material_center;
                match update_task_destination_to_adjacent(ctx, site_pos) {
                    PathSearchResult::Found(()) => {}
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        return cancel_haul_with_reason(
                            ctx,
                            item,
                            stockpile,
                            commands,
                            "cannot reach floor site",
                        );
                    }
                }

                if is_near_target_or_dest(soul_pos, site_pos, ctx.dest.0) {
                    set_haul_phase(&mut ctx.task, item, stockpile, HaulPhase::Dropping);
                    ctx.path.waypoints.clear();
                }
            } else if let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(stockpile) {
                let site_pos = site.material_center;
                match update_task_destination_to_adjacent(ctx, site_pos) {
                    PathSearchResult::Found(()) => {}
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        return cancel_haul_with_reason(
                            ctx,
                            item,
                            stockpile,
                            commands,
                            "cannot reach wall site",
                        );
                    }
                }

                if is_near_target_or_dest(soul_pos, site_pos, ctx.dest.0) {
                    set_haul_phase(&mut ctx.task, item, stockpile, HaulPhase::Dropping);
                    ctx.path.waypoints.clear();
                }
            } else if let Ok((wall_transform, building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(stockpile)
            {
                let can_deliver_to_wall = building.kind == BuildingType::Wall
                    && building.is_provisional
                    && provisional_opt
                        .as_ref()
                        .is_some_and(|provisional| !provisional.mud_delivered);
                if !can_deliver_to_wall {
                    return cancel_haul_with_reason(
                        ctx,
                        item,
                        stockpile,
                        commands,
                        "destination became invalid provisional wall",
                    );
                }

                let wall_pos = wall_transform.translation.truncate();
                match update_task_destination_to_adjacent(ctx, wall_pos) {
                    PathSearchResult::Found(()) => {}
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        return cancel_haul_with_reason(
                            ctx,
                            item,
                            stockpile,
                            commands,
                            "cannot reach provisional wall",
                        );
                    }
                }

                if is_near_target_or_dest(soul_pos, wall_pos, ctx.dest.0) {
                    set_haul_phase(&mut ctx.task, item, stockpile, HaulPhase::Dropping);
                    ctx.path.waypoints.clear();
                }
            } else {
                return cancel_haul_with_reason(
                    ctx,
                    item,
                    stockpile,
                    commands,
                    "destination disappeared",
                );
            }
        }
        HaulPhase::Dropping => {
            return handle_dropping_phase(ctx, item, stockpile, commands, soul_pos);
        }
    }

    TaskHandlerControl::Continue
}
