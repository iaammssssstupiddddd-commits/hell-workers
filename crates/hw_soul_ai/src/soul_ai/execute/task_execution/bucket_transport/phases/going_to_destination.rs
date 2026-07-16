//! GoingToDestination phase: 水入りバケツを持ってデスティネーション（タンク or ミキサー）へ向かう

use crate::soul_ai::execute::task_execution::common::{
    PathSearchResult, is_near_target_or_dest, update_bucket_destination_to_adjacent,
};
use crate::soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskHandlerControl};
use crate::soul_ai::execute::task_execution::types::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource,
};
use bevy::prelude::*;
use hw_logistics::ResourceType;

use super::super::{abort, guards};

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    if ctx.inventory.0 != Some(data.bucket) {
        warn!(
            "GoingToDestination: Bucket not in inventory for soul {:?}",
            ctx.soul_entity
        );
        return abort::abort_without_bucket(commands, ctx, data, ctx.env.world_map);
    }

    let soul_pos = ctx.soul_transform.translation.truncate();

    match data.destination {
        BucketTransportDestination::Tank(tank_entity) => {
            if !guards::tank_can_accept_full_bucket(ctx, tank_entity) {
                // タンクが満杯: バケツをドロップして auto haul に任せる
                return super::super::helpers::drop_bucket_for_auto_haul(
                    commands,
                    ctx,
                    data.bucket,
                    ctx.env.world_map,
                );
            }

            // タンク境界に近づいたら Pouring へ
            if soul_pos.distance(ctx.dest.0) < 60.0 {
                *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
                    phase: BucketTransportPhase::Pouring { progress: 0.0 },
                    ..data.clone()
                });
            }
        }
        BucketTransportDestination::Mixer(mixer_entity) => {
            // バケツの水を確認
            if let Ok(res_item) = ctx.queries.reservation.resources.get(data.bucket) {
                if res_item.0 != ResourceType::BucketWater {
                    // 空バケツ → タンクへ戻る
                    let tank = match data.source {
                        BucketTransportSource::Tank { tank, .. } => tank,
                        BucketTransportSource::River => {
                            return abort::abort_with_bucket(
                                commands,
                                ctx,
                                data,
                                ctx.env.world_map,
                            );
                        }
                    };
                    return transition_to_tank_internal(
                        commands,
                        ctx,
                        data.bucket,
                        tank,
                        mixer_entity,
                        soul_pos,
                        data,
                    );
                }
            } else {
                let tank = match data.source {
                    BucketTransportSource::Tank { tank, .. } => tank,
                    BucketTransportSource::River => data.bucket,
                };
                return abort::abort_and_drop_bucket_mixer(
                    commands,
                    ctx,
                    data.bucket,
                    tank,
                    mixer_entity,
                    soul_pos,
                );
            }

            // amount チェック
            if data.amount == 0 {
                let tank = match data.source {
                    BucketTransportSource::Tank { tank, .. } => tank,
                    BucketTransportSource::River => {
                        return abort::abort_with_bucket(commands, ctx, data, ctx.env.world_map);
                    }
                };
                return transition_to_tank_internal(
                    commands,
                    ctx,
                    data.bucket,
                    tank,
                    mixer_entity,
                    soul_pos,
                    data,
                );
            }

            if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
                let (mixer_transform, _, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();

                match update_bucket_destination_to_adjacent(ctx, mixer_pos) {
                    PathSearchResult::Found(()) => {}
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        let tank = match data.source {
                            BucketTransportSource::Tank { tank, .. } => tank,
                            BucketTransportSource::River => data.bucket,
                        };
                        return abort::abort_and_drop_bucket_mixer(
                            commands,
                            ctx,
                            data.bucket,
                            tank,
                            mixer_entity,
                            soul_pos,
                        );
                    }
                }

                if is_near_target_or_dest(soul_pos, mixer_pos, ctx.dest.0) {
                    *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
                        phase: BucketTransportPhase::Pouring { progress: 0.0 },
                        ..data.clone()
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                let tank = match data.source {
                    BucketTransportSource::Tank { tank, .. } => tank,
                    BucketTransportSource::River => data.bucket,
                };
                return abort::abort_and_drop_bucket_mixer(
                    commands,
                    ctx,
                    data.bucket,
                    tank,
                    mixer_entity,
                    soul_pos,
                );
            }
        }
    }

    TaskHandlerControl::Continue
}

fn transition_to_tank_internal(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    _mixer_entity: Entity,
    soul_pos: Vec2,
    data: &BucketTransportData,
) -> TaskHandlerControl {
    if let Ok(tank_data) = ctx.queries.storage.stockpiles.get(tank_entity) {
        let (_, tank_transform, _, _) = tank_data;
        let tank_pos = tank_transform.translation.truncate();
        commands
            .entity(bucket_entity)
            .remove::<hw_core::relationships::DeliveringTo>();

        *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
            phase: BucketTransportPhase::GoingToSource,
            source: BucketTransportSource::Tank {
                tank: tank_entity,
                needs_fill: true,
            },
            ..data.clone()
        });
        ctx.dest.0 = tank_pos;
        ctx.path.waypoints.clear();
    } else {
        let mixer = match data.destination {
            BucketTransportDestination::Mixer(m) => m,
            _ => return TaskHandlerControl::Continue,
        };
        return abort::abort_and_drop_bucket_mixer(
            commands,
            ctx,
            bucket_entity,
            tank_entity,
            mixer,
            soul_pos,
        );
    }

    TaskHandlerControl::Continue
}
