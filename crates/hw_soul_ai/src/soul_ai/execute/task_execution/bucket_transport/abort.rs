//! バケツ搬送共通 abort/cleanup ヘルパー

use crate::soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskHandlerControl};
use crate::soul_ai::execute::task_execution::transport_common::cancel;
use crate::soul_ai::execute::task_execution::types::{
    BucketTransportData, BucketTransportDestination,
};
use bevy::prelude::*;
use hw_world::WorldMap;

/// Mixer 搬送（Tank→Mixer経路）の中断: バケツドロップ後にタスクを閉じる。
pub fn abort_and_drop_bucket_mixer(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    _tank_entity: Entity,
    _mixer_entity: Entity,
    pos: Vec2,
) -> TaskHandlerControl {
    cancel::drop_bucket_with_cleanup(commands, bucket_entity, pos);

    ctx.inventory.0 = None;
    ctx.abort_retryable_after_custom_cleanup(commands, "bucket transport mixer abort")
}

/// バケツなし abort（インベントリにバケツが存在しない状態でのタスク中断）
pub fn abort_without_bucket(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    _world_map: &WorldMap,
) -> TaskHandlerControl {
    match &data.destination {
        BucketTransportDestination::Mixer(_) => {
            ctx.abort_retryable_after_custom_cleanup(commands, "bucket transport mixer abort")
        }
        BucketTransportDestination::Tank(_) => {
            ctx.abort_retryable(commands, "bucket transport tank abort")
        }
    }
}

/// バケツあり abort（インベントリにバケツが存在する状態でのタスク中断）
pub fn abort_with_bucket(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    _world_map: &WorldMap,
) -> TaskHandlerControl {
    match &data.destination {
        BucketTransportDestination::Mixer(_) => {
            let soul_pos = ctx.soul_pos();
            cancel::drop_bucket_with_cleanup(commands, data.bucket, soul_pos);
            ctx.inventory.0 = None;
            ctx.abort_retryable_after_custom_cleanup(commands, "bucket transport mixer abort")
        }
        BucketTransportDestination::Tank(_) => {
            ctx.abort_retryable(commands, "bucket transport tank abort")
        }
    }
}
