//! GoingToBucket phase: バケツを拾いに行く / バケツ状態確認

use hw_logistics::ResourceType;
use crate::soul_ai::execute::task_execution::common;
use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::soul_ai::execute::task_execution::transport_common::reservation;
use crate::soul_ai::execute::task_execution::types::{
    BucketTransportData, BucketTransportDestination, BucketTransportSource,
};
use hw_world::WorldMap;
use bevy::prelude::*;

use super::super::{abort, routing};

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    commands: &mut Commands,
    world_map: &WorldMap,
) {
    let bucket_entity = data.bucket;
    let soul_pos = ctx.soul_pos();

    // すでにバケツを所持している場合: バケツ状態を確認してルーティング
    if ctx.inventory.0 == Some(bucket_entity) {
        match &data.destination {
            BucketTransportDestination::Mixer(mixer_entity) => {
                let mixer = *mixer_entity;
                // バケツの状態を確認
                if let Ok(res_item) = ctx.queries.reservation.resources.get(bucket_entity) {
                    if res_item.0 == ResourceType::BucketWater {
                        // 水入りなら直接ミキサーへ
                        let tank = match data.source {
                            BucketTransportSource::Tank { tank, .. } => tank,
                            _ => bucket_entity,
                        };
                        reservation::release_source(ctx, tank, 1);
                        routing::transition_to_destination(
                            commands, ctx, data, soul_pos, world_map,
                        );
                    } else {
                        // 空ならタンクへ
                        routing::transition_to_source(commands, ctx, data, soul_pos, world_map);
                    }
                } else {
                    // バケツが見つからない場合は中断
                    let tank = match data.source {
                        BucketTransportSource::Tank { tank, .. } => tank,
                        _ => bucket_entity,
                    };
                    reservation::release_mixer_destination(
                        ctx,
                        mixer,
                        hw_logistics::ResourceType::Water,
                    );
                    let _ = tank;
                    common::clear_task_and_path(ctx.task, ctx.path);
                }
            }
            BucketTransportDestination::Tank(tank_entity) => {
                let tank = *tank_entity;
                // River→Tank 経路: 既にバケツ保持済みならソースへ向かう
                if let Ok(res_item) = ctx.queries.reservation.resources.get(bucket_entity) {
                    if res_item.0 == ResourceType::BucketWater {
                        // 既に水入り→直接タンクへ
                        if let Ok((tank_transform, _, _, _, _, _, _)) =
                            ctx.queries.designation.targets.get(tank)
                        {
                            let tank_pos = tank_transform.translation.truncate();
                            if routing::set_path_to_tank_boundary(
                                ctx,
                                world_map,
                                tank_pos,
                                data,
                                crate::soul_ai::execute::task_execution::types::BucketTransportPhase::GoingToDestination,
                            )
                            .is_some()
                            {
                                commands
                                    .entity(bucket_entity)
                                    .try_insert(hw_core::relationships::DeliveringTo(tank));
                                return;
                            }
                        }
                    }
                }
                // 空バケツ: 川へ
                if routing::set_path_to_river(ctx, world_map, data).is_none() {
                    abort::abort_with_bucket(commands, ctx, data, world_map);
                } else {
                    commands
                        .entity(bucket_entity)
                        .remove::<hw_core::relationships::DeliveringTo>();
                }
            }
        }
        return;
    }

    // バケツを所持していない場合: バケツ位置を確認して移動または拾得
    let Ok((bucket_transform, _, _, _, res_item_opt, _, stored_in_opt)) =
        ctx.queries.designation.targets.get(bucket_entity)
    else {
        abort::abort_without_bucket(commands, ctx, data, world_map);
        return;
    };

    let res_type = res_item_opt.map(|res| res.0);
    let stored_in_entity = stored_in_opt.map(|stored| stored.0);

    if let Some(rt) = res_type {
        if !matches!(rt, ResourceType::BucketEmpty | ResourceType::BucketWater) {
            abort::abort_without_bucket(commands, ctx, data, world_map);
            return;
        }
    }

    let bucket_pos = bucket_transform.translation.truncate();

    if common::can_pickup_item(soul_pos, bucket_pos) {
        if !common::try_pickup_item(
            commands,
            ctx.soul_entity,
            bucket_entity,
            ctx.inventory,
            soul_pos,
            bucket_pos,
            ctx.task,
            ctx.path,
        ) {
            return;
        }

        ctx.queue_reservation(hw_core::events::ResourceReservationOp::RecordPickedSource {
            source: bucket_entity,
            amount: 1,
        });

        if let Some(stored_in) = stored_in_entity {
            let q_stockpiles = &mut ctx.queries.storage.stockpiles;
            common::update_stockpile_on_item_removal(stored_in, q_stockpiles);
        }

        let is_already_full = res_type == Some(ResourceType::BucketWater);
        if is_already_full {
            match &data.destination {
                BucketTransportDestination::Tank(tank_entity) => {
                    let tank = *tank_entity;
                    if let Ok((tank_transform, _, _, _, _, _, _)) =
                        ctx.queries.designation.targets.get(tank)
                    {
                        let tank_pos = tank_transform.translation.truncate();
                        if routing::set_path_to_tank_boundary(
                            ctx,
                            world_map,
                            tank_pos,
                            data,
                            crate::soul_ai::execute::task_execution::types::BucketTransportPhase::GoingToDestination,
                        )
                        .is_some()
                        {
                            commands
                                .entity(bucket_entity)
                                .try_insert(hw_core::relationships::DeliveringTo(tank));
                            return;
                        }
                    }
                    // タンクが見つからない / パスが取れない場合は川へ
                }
                BucketTransportDestination::Mixer(_) => {
                    // 水入りなら直接ミキサーへ
                    let tank = match data.source {
                        BucketTransportSource::Tank { tank, .. } => tank,
                        _ => bucket_entity,
                    };
                    reservation::release_source(ctx, tank, 1);
                    routing::transition_to_destination(commands, ctx, data, soul_pos, world_map);
                    return;
                }
            }
        }

        // 空バケツ or タンクパス失敗: ソースへ向かう
        routing::transition_to_source(commands, ctx, data, soul_pos, world_map);
        return;
    }

    // まだバケツに近づいていない: 移動中
    let bucket_grid = WorldMap::world_to_grid(bucket_pos);
    if ctx.path.waypoints.is_empty() {
        if routing::set_path_to_grid_boundary(ctx, world_map, bucket_grid, bucket_pos).is_none() {
            ctx.dest.0 = bucket_pos;
        }
    }
}
