//! 運搬タスクの中断処理
//!
//! 失敗経路での予約解放とタスククリアを共通化する。

use crate::systems::soul_ai::execute::task_execution::common::clear_task_and_path;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;

use super::reservation;

/// ストックパイル運搬の中断: 目的地＋ソース解放、タスククリア
pub fn cancel_haul_to_stockpile(
    ctx: &mut TaskExecutionContext,
    item: Entity,
    stockpile: Entity,
) {
    reservation::release_destination(ctx, stockpile);
    reservation::release_source(ctx, item, 1);
    clear_task_and_path(ctx.task, ctx.path);
}

/// ミキサー運搬の中断: ミキサー目的地解放、タスククリア
pub fn cancel_haul_to_mixer(
    ctx: &mut TaskExecutionContext,
    mixer: Entity,
    resource_type: crate::systems::logistics::ResourceType,
) {
    reservation::release_mixer_destination(ctx, mixer, resource_type);
    clear_task_and_path(ctx.task, ctx.path);
}
