//! GoingToSource phase: バケツを持ってソース（川 or タンク）へ向かう

use hw_logistics::ResourceType;
use crate::soul_ai::execute::task_execution::common::update_destination_to_adjacent;
use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::soul_ai::execute::task_execution::types::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource,
};
use hw_world::WorldMap;
use bevy::prelude::*;

use super::super::{abort, guards};

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    commands: &mut Commands,
    world_map: &WorldMap,
) {
    if ctx.inventory.0 != Some(data.bucket) {
        warn!(
            "GoingToSource: Bucket not in inventory for soul {:?}",
            ctx.soul_entity
        );
        abort::abort_without_bucket(commands, ctx, data, world_map);
        return;
    }

    let soul_pos = ctx.soul_transform.translation.truncate();

    match data.source {
        BucketTransportSource::River => {
            // 川の隣接グリッドに近い場合: タンク容量チェック後に Filling へ
            if soul_pos.distance(ctx.dest.0) < 30.0 {
                let tank_entity = match data.destination {
                    BucketTransportDestination::Tank(tank) => tank,
                    _ => {
                        abort::abort_with_bucket(commands, ctx, data, world_map);
                        return;
                    }
                };

                if !guards::tank_can_accept_full_bucket(ctx, tank_entity) {
                    // タンクが満杯: バケツをドロップして auto haul に任せる
                    super::super::helpers::drop_bucket_for_auto_haul(
                        commands,
                        ctx,
                        data.bucket,
                        world_map,
                    );
                    return;
                }

                *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
                    phase: BucketTransportPhase::Filling { progress: 0.0 },
                    ..data.clone()
                });
            }
        }
        BucketTransportSource::Tank { tank, .. } => {
            // タンク境界に到達したら FillingFromTank (Filling) へ
            if let Ok(tank_data) = ctx.queries.storage.stockpiles.get(tank) {
                let (_, tank_transform, _, _) = tank_data;
                let tank_pos = tank_transform.translation.truncate();

                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    tank_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );

                if !reachable {
                    let mixer = match data.destination {
                        BucketTransportDestination::Mixer(m) => m,
                        _ => {
                            abort::abort_with_bucket(commands, ctx, data, world_map);
                            return;
                        }
                    };
                    abort::abort_and_drop_bucket_mixer(
                        commands,
                        ctx,
                        data.bucket,
                        tank,
                        mixer,
                        soul_pos,
                    );
                    return;
                }

                use crate::soul_ai::execute::task_execution::common::is_near_target_or_dest;
                if is_near_target_or_dest(soul_pos, tank_pos, ctx.dest.0) {
                    *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
                        phase: BucketTransportPhase::Filling { progress: 0.0 },
                        source: BucketTransportSource::Tank {
                            tank,
                            needs_fill: true,
                        },
                        ..data.clone()
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                // タンクが見つからない
                let mixer = match data.destination {
                    BucketTransportDestination::Mixer(m) => m,
                    _ => {
                        abort::abort_with_bucket(commands, ctx, data, world_map);
                        return;
                    }
                };
                abort::abort_and_drop_bucket_mixer(
                    commands,
                    ctx,
                    data.bucket,
                    tank,
                    mixer,
                    soul_pos,
                );
            }
        }
    }

    // タンク容量の再確認 (River→Tank 経路のみ)
    if let BucketTransportSource::River = data.source {
        if let BucketTransportDestination::Tank(tank_entity) = data.destination {
            if let Ok(res_item) = ctx.queries.reservation.resources.get(data.bucket) {
                if res_item.0 == ResourceType::BucketWater {
                    // バケツが既に水入りならソースには行かずデスティネーションへ
                    super::super::routing::transition_to_destination(
                        commands, ctx, data, soul_pos, world_map,
                    );
                    return;
                }
            }

            if !guards::tank_can_accept_full_bucket(ctx, tank_entity) {
                super::super::helpers::drop_bucket_for_auto_haul(
                    commands,
                    ctx,
                    data.bucket,
                    world_map,
                );
            }
        }
    }
}
