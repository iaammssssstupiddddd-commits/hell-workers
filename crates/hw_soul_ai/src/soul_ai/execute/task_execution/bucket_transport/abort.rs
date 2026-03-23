//! バケツ搬送共通 abort/cleanup ヘルパー

use crate::soul_ai::execute::task_execution::common::clear_task_and_path;
use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::soul_ai::execute::task_execution::transport_common::{cancel, reservation};
use crate::soul_ai::execute::task_execution::types::{
    BucketTransportData, BucketTransportDestination, BucketTransportSource,
};
use bevy::prelude::*;
use hw_logistics::ResourceType;
use hw_world::WorldMap;

/// Mixer 搬送（Tank→Mixer経路）の中断: バケツドロップ + 予約解放 + タスククリア
pub fn abort_and_drop_bucket_mixer(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    pos: Vec2,
) {
    reservation::release_mixer_destination(ctx, mixer_entity, ResourceType::Water);
    let should_release_tank_lock = ctx
        .task
        .bucket_transport_data()
        .is_some_and(|task_data| task_data.should_reserve_tank_source());
    if should_release_tank_lock {
        reservation::release_source(ctx, tank_entity, 1);
    }

    cancel::drop_bucket_with_cleanup(commands, bucket_entity, pos);

    ctx.inventory.0 = None;
    clear_task_and_path(ctx.task, ctx.path);
}

/// バケツなし abort（インベントリにバケツが存在しない状態でのタスク中断）
pub fn abort_without_bucket(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    world_map: &WorldMap,
) {
    match &data.destination {
        BucketTransportDestination::Mixer(mixer_entity) => {
            let mixer = *mixer_entity;
            let tank_opt = match data.source {
                BucketTransportSource::Tank { tank, .. } => Some(tank),
                BucketTransportSource::River => None,
            };
            reservation::release_mixer_destination(ctx, mixer, ResourceType::Water);
            if let Some(tank) = tank_opt {
                if data.should_reserve_tank_source() {
                    reservation::release_source(ctx, tank, 1);
                }
            }
            clear_task_and_path(ctx.task, ctx.path);
        }
        BucketTransportDestination::Tank(_) => {
            let soul_pos = ctx.soul_pos();
            crate::soul_ai::helpers::work::cleanup_task_assignment(
                commands,
                ctx.soul_entity,
                soul_pos,
                ctx.task,
                ctx.path,
                None,
                None,
                ctx.queries,
                world_map,
                true,
            );
        }
    }
}

/// バケツあり abort（インベントリにバケツが存在する状態でのタスク中断）
pub fn abort_with_bucket(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    world_map: &WorldMap,
) {
    match &data.destination {
        BucketTransportDestination::Mixer(mixer_entity) => {
            let mixer = *mixer_entity;
            let tank_opt = match data.source {
                BucketTransportSource::Tank { tank, .. } => Some(tank),
                BucketTransportSource::River => None,
            };
            reservation::release_mixer_destination(ctx, mixer, ResourceType::Water);
            if let Some(tank) = tank_opt {
                if data.should_reserve_tank_source() {
                    reservation::release_source(ctx, tank, 1);
                }
            }
            let soul_pos = ctx.soul_pos();
            cancel::drop_bucket_with_cleanup(commands, data.bucket, soul_pos);
            ctx.inventory.0 = None;
            clear_task_and_path(ctx.task, ctx.path);
        }
        BucketTransportDestination::Tank(_) => {
            let soul_pos = ctx.soul_pos();
            crate::soul_ai::helpers::work::cleanup_task_assignment(
                commands,
                ctx.soul_entity,
                soul_pos,
                ctx.task,
                ctx.path,
                Some(ctx.inventory),
                None,
                ctx.queries,
                world_map,
                true,
            );
        }
    }
}
