//! 運搬タスクの実行処理（ストックパイルへ）

use crate::systems::jobs::BuildingType;
use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::transport_common::{cancel, reservation};
use crate::systems::soul_ai::execute::task_execution::{
    context::TaskExecutionContext,
    types::{AssignedTask, HaulData, HaulPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

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
) {
    info!(
        "HAUL: Soul {:?} {}, canceling (item={:?}, stockpile={:?})",
        ctx.soul_entity, reason, item, stockpile
    );
    cancel::cancel_haul_to_stockpile(ctx, item, stockpile, commands);
}

pub fn handle_haul_task(
    ctx: &mut TaskExecutionContext,
    item: Entity,
    stockpile: Entity,
    phase: HaulPhase,
    commands: &mut Commands,
    // haul_cache is now accessed via ctx.queries.resource_cache
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.designation.targets;
    match phase {
        HaulPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _, _res_item_opt, _, stored_in_opt)) =
                q_targets.get(item)
            {
                let res_pos = res_transform.translation.truncate();
                let stored_in_entity = stored_in_opt.map(|stored_in| stored_in.0);
                // アイテムが障害物の上にある可能性があるため、隣接マスを目的地として設定
                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    res_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );

                if !reachable {
                    cancel_haul_with_reason(
                        ctx,
                        item,
                        stockpile,
                        commands,
                        "cannot reach pickup item",
                    );
                    return;
                }

                let is_near = can_pickup_item(soul_pos, res_pos);

                if is_near {
                    if !try_pickup_item(
                        commands,
                        ctx.soul_entity,
                        item,
                        ctx.inventory,
                        soul_pos,
                        res_pos,
                        ctx.task,
                        ctx.path,
                    ) {
                        return;
                    }
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
                        update_destination_if_needed(ctx.dest, stock_dest, ctx.path);
                    } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(stockpile)
                    {
                        let reachable = update_destination_to_adjacent(
                            ctx.dest,
                            site.material_center,
                            ctx.path,
                            soul_pos,
                            world_map,
                            ctx.pf_context,
                        );
                        if !reachable {
                            cancel_haul_with_reason(
                                ctx,
                                item,
                                stockpile,
                                commands,
                                "cannot reach floor site",
                            );
                            return;
                        }
                    } else if let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(stockpile) {
                        let reachable = update_destination_to_adjacent(
                            ctx.dest,
                            site.material_center,
                            ctx.path,
                            soul_pos,
                            world_map,
                            ctx.pf_context,
                        );
                        if !reachable {
                            cancel_haul_with_reason(
                                ctx,
                                item,
                                stockpile,
                                commands,
                                "cannot reach wall site",
                            );
                            return;
                        }
                    } else if let Ok((wall_transform, building, provisional_opt)) =
                        ctx.queries.storage.buildings.get_mut(stockpile)
                    {
                        let can_deliver_to_wall = building.kind == BuildingType::Wall
                            && building.is_provisional
                            && provisional_opt
                                .as_ref()
                                .is_some_and(|provisional| !provisional.mud_delivered);
                        if can_deliver_to_wall {
                            let reachable = update_destination_to_adjacent(
                                ctx.dest,
                                wall_transform.translation.truncate(),
                                ctx.path,
                                soul_pos,
                                world_map,
                                ctx.pf_context,
                            );
                            if !reachable {
                                cancel_haul_with_reason(
                                    ctx,
                                    item,
                                    stockpile,
                                    commands,
                                    "cannot reach provisional wall",
                                );
                                return;
                            }
                        } else {
                            cancel_haul_with_reason(
                                ctx,
                                item,
                                stockpile,
                                commands,
                                "destination became invalid provisional wall",
                            );
                            return;
                        }
                    }

                    set_haul_phase(ctx.task, item, stockpile, HaulPhase::GoingToStockpile);
                    reservation::record_picked_source(ctx, item, 1);
                    info!("HAUL: Soul {:?} picked up item {:?}", ctx.soul_entity, item);
                }
            } else {
                cancel_haul_with_reason(ctx, item, stockpile, commands, "pickup item disappeared");
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok((_, stock_transform, _, _)) = ctx.queries.storage.stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                let stock_grid = WorldMap::world_to_grid(stock_pos);
                let stock_dest = WorldMap::grid_to_world(stock_grid.0, stock_grid.1);
                update_destination_if_needed(ctx.dest, stock_dest, ctx.path);

                if is_near_target(soul_pos, stock_pos) {
                    set_haul_phase(ctx.task, item, stockpile, HaulPhase::Dropping);
                    ctx.path.waypoints.clear();
                }
            } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(stockpile) {
                let site_pos = site.material_center;
                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    site_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );
                if !reachable {
                    cancel_haul_with_reason(
                        ctx,
                        item,
                        stockpile,
                        commands,
                        "cannot reach floor site",
                    );
                    return;
                }

                if is_near_target_or_dest(soul_pos, site_pos, ctx.dest.0) {
                    set_haul_phase(ctx.task, item, stockpile, HaulPhase::Dropping);
                    ctx.path.waypoints.clear();
                }
            } else if let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(stockpile) {
                let site_pos = site.material_center;
                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    site_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );
                if !reachable {
                    cancel_haul_with_reason(
                        ctx,
                        item,
                        stockpile,
                        commands,
                        "cannot reach wall site",
                    );
                    return;
                }

                if is_near_target_or_dest(soul_pos, site_pos, ctx.dest.0) {
                    set_haul_phase(ctx.task, item, stockpile, HaulPhase::Dropping);
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
                    cancel_haul_with_reason(
                        ctx,
                        item,
                        stockpile,
                        commands,
                        "destination became invalid provisional wall",
                    );
                    return;
                }

                let wall_pos = wall_transform.translation.truncate();
                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    wall_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );
                if !reachable {
                    cancel_haul_with_reason(
                        ctx,
                        item,
                        stockpile,
                        commands,
                        "cannot reach provisional wall",
                    );
                    return;
                }

                if is_near_target_or_dest(soul_pos, wall_pos, ctx.dest.0) {
                    set_haul_phase(ctx.task, item, stockpile, HaulPhase::Dropping);
                    ctx.path.waypoints.clear();
                }
            } else {
                cancel_haul_with_reason(ctx, item, stockpile, commands, "destination disappeared");
            }
        }
        HaulPhase::Dropping => {
            handle_dropping_phase(ctx, item, stockpile, commands, world_map, soul_pos);
        }
    }
}
