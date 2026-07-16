//! GoingToBucket phase: バケツを拾いに行く / バケツ状態確認

use crate::soul_ai::execute::task_execution::common;
use crate::soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskHandlerControl};
use crate::soul_ai::execute::task_execution::transport_common::reservation;
use crate::soul_ai::execute::task_execution::types::{
    AssignedTask, BucketTransportData, BucketTransportDestination,
};
use bevy::prelude::*;
use hw_core::constants::BUCKET_CAPACITY;
use hw_logistics::ResourceType;
use hw_world::WorldMap;

use super::super::{abort, routing};

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let bucket_entity = data.bucket;
    let soul_pos = ctx.soul_pos();

    // すでにバケツを所持している場合: バケツ状態を確認してルーティング
    if ctx.inventory.0 == Some(bucket_entity) {
        match &data.destination {
            BucketTransportDestination::Mixer(mixer_entity) => {
                let mixer = *mixer_entity;
                if let Ok(res_item) = ctx.queries.reservation.resources.get(bucket_entity) {
                    if res_item.0 == ResourceType::BucketWater {
                        let control = routing::transition_to_destination(
                            commands,
                            ctx,
                            data,
                            soul_pos,
                            ctx.env.world_map,
                        );
                        if control != TaskHandlerControl::Continue {
                            return control;
                        }

                        // Filling フェーズをスキップしたため amount が 0 のまま。
                        // GoingToDestination/Pouring フェーズが正しく動作するよう補正する。
                        if let AssignedTask::BucketTransport(ref mut task_data) = *ctx.task
                            && matches!(
                                task_data.phase,
                                crate::soul_ai::execute::task_execution::types::BucketTransportPhase::GoingToDestination
                            )
                            && task_data.amount == 0
                        {
                            task_data.amount = BUCKET_CAPACITY;
                        }
                    } else {
                        return routing::transition_to_source(
                            commands,
                            ctx,
                            data,
                            soul_pos,
                            ctx.env.world_map,
                        );
                    }
                } else {
                    reservation::release_mixer_destination(
                        ctx,
                        mixer,
                        hw_logistics::ResourceType::Water,
                    );
                    return ctx.abort_retryable(commands, "bucket transport bucket missing");
                }
            }
            BucketTransportDestination::Tank(tank_entity) => {
                let tank = *tank_entity;
                if let Ok(res_item) = ctx.queries.reservation.resources.get(bucket_entity)
                    && res_item.0 == ResourceType::BucketWater
                    && let Ok((tank_transform, _, _, _, _, _, _)) =
                        ctx.queries.designation.targets.get(tank)
                {
                    let tank_pos = tank_transform.translation.truncate();
                    match routing::set_path_to_tank_boundary(
                        ctx,
                        ctx.env.world_map,
                        tank_pos,
                        data,
                        crate::soul_ai::execute::task_execution::types::BucketTransportPhase::GoingToDestination,
                    ) {
                        common::PathSearchResult::Found(()) => {
                            commands
                                .entity(bucket_entity)
                                .try_insert(hw_core::relationships::DeliveringTo(tank));
                            return TaskHandlerControl::Continue;
                        }
                        common::PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                        common::PathSearchResult::Unreachable => {}
                    }
                }

                match routing::set_path_to_river(ctx, ctx.env.world_map, data) {
                    common::PathSearchResult::Found(()) => {
                        commands
                            .entity(bucket_entity)
                            .remove::<hw_core::relationships::DeliveringTo>();
                    }
                    common::PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    common::PathSearchResult::Unreachable => {
                        return abort::abort_with_bucket(commands, ctx, data, ctx.env.world_map);
                    }
                }
            }
        }
        return TaskHandlerControl::Continue;
    }

    // バケツを所持していない場合: バケツ位置を確認して移動または拾得
    let Ok((bucket_transform, _, _, _, res_item_opt, _, stored_in_opt)) =
        ctx.queries.designation.targets.get(bucket_entity)
    else {
        return abort::abort_without_bucket(commands, ctx, data, ctx.env.world_map);
    };

    let res_type = res_item_opt.map(|res| res.0);
    let stored_in_entity = stored_in_opt.map(|stored| stored.0);

    if let Some(resource_type) = res_type
        && !matches!(
            resource_type,
            ResourceType::BucketEmpty | ResourceType::BucketWater
        )
    {
        return abort::abort_without_bucket(commands, ctx, data, ctx.env.world_map);
    }

    let bucket_pos = bucket_transform.translation.truncate();
    if common::can_pickup_item(soul_pos, bucket_pos) {
        if let Err(control) = common::try_pickup_item(
            commands,
            ctx,
            common::PickupLocations {
                soul_entity: ctx.soul_entity,
                item_entity: bucket_entity,
                soul_pos,
                item_pos: bucket_pos,
            },
        ) {
            return control;
        }

        ctx.queue_reservation(hw_core::events::ResourceReservationOp::RecordPickedSource {
            source: bucket_entity,
            amount: 1,
        });

        if let Some(stored_in) = stored_in_entity {
            let q_stockpiles = &mut ctx.queries.storage.stockpiles;
            common::update_stockpile_on_item_removal(stored_in, q_stockpiles);
        }

        if res_type == Some(ResourceType::BucketWater) {
            match &data.destination {
                BucketTransportDestination::Tank(tank_entity) => {
                    let tank = *tank_entity;
                    if let Ok((tank_transform, _, _, _, _, _, _)) =
                        ctx.queries.designation.targets.get(tank)
                    {
                        let tank_pos = tank_transform.translation.truncate();
                        match routing::set_path_to_tank_boundary(
                            ctx,
                            ctx.env.world_map,
                            tank_pos,
                            data,
                            crate::soul_ai::execute::task_execution::types::BucketTransportPhase::GoingToDestination,
                        ) {
                            common::PathSearchResult::Found(()) => {
                                commands
                                    .entity(bucket_entity)
                                    .try_insert(hw_core::relationships::DeliveringTo(tank));
                                return TaskHandlerControl::Continue;
                            }
                            common::PathSearchResult::Deferred => {
                                return TaskHandlerControl::Continue;
                            }
                            common::PathSearchResult::Unreachable => {}
                        }
                    }
                }
                BucketTransportDestination::Mixer(_) => {
                    let control = routing::transition_to_destination(
                        commands,
                        ctx,
                        data,
                        soul_pos,
                        ctx.env.world_map,
                    );
                    if control != TaskHandlerControl::Continue {
                        return control;
                    }

                    if let AssignedTask::BucketTransport(ref mut task_data) = *ctx.task
                        && matches!(
                            task_data.phase,
                            crate::soul_ai::execute::task_execution::types::BucketTransportPhase::GoingToDestination
                        )
                        && task_data.amount == 0
                    {
                        task_data.amount = BUCKET_CAPACITY;
                    }
                    return TaskHandlerControl::Continue;
                }
            }
        }

        return routing::transition_to_source(commands, ctx, data, soul_pos, ctx.env.world_map);
    }

    let bucket_grid = WorldMap::world_to_grid(bucket_pos);
    if ctx.path.waypoints.is_empty() {
        match routing::set_path_to_grid_boundary(ctx, ctx.env.world_map, bucket_grid, bucket_pos) {
            common::PathSearchResult::Found(()) => {}
            common::PathSearchResult::Deferred => return TaskHandlerControl::Continue,
            common::PathSearchResult::Unreachable => ctx.dest.0 = bucket_pos,
        }
    }

    TaskHandlerControl::Continue
}
