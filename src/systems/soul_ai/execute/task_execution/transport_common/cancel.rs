//! 運搬タスクの中断処理
//!
//! 失敗経路での予約解放とタスククリアを共通化する。

use crate::systems::soul_ai::execute::task_execution::common::clear_task_and_path;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::world::map::WorldMap;
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

/// Blueprint運搬の中断: 目的地＋ソース解放、タスククリア
pub fn cancel_haul_to_blueprint(
    ctx: &mut TaskExecutionContext,
    item: Entity,
    blueprint: Entity,
) {
    reservation::release_destination(ctx, blueprint);
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

/// ミキサー運搬（未ピックアップ段階）の中断:
/// ソース＋ミキサー目的地解放、タスククリア
pub fn cancel_haul_to_mixer_before_pickup(
    ctx: &mut TaskExecutionContext,
    item: Entity,
    mixer: Entity,
    resource_type: crate::systems::logistics::ResourceType,
) {
    reservation::release_source(ctx, item, 1);
    cancel_haul_to_mixer(ctx, mixer, resource_type);
}

/// バケツを足元グリッドへドロップし、運搬関連の管理コンポーネントを除去する。
pub fn drop_bucket_with_cleanup(commands: &mut Commands, bucket_entity: Entity, pos: Vec2) {
    let drop_grid = WorldMap::world_to_grid(pos);
    let drop_pos = WorldMap::grid_to_world(drop_grid.0, drop_grid.1);
    commands.entity(bucket_entity).insert((
        Visibility::Visible,
        Transform::from_xyz(
            drop_pos.x,
            drop_pos.y,
            crate::constants::Z_ITEM_PICKUP,
        ),
    ));
    commands
        .entity(bucket_entity)
        .remove::<crate::relationships::StoredIn>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::logistics::InStockpile>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::IssuedBy>();
    commands
        .entity(bucket_entity)
        .remove::<crate::relationships::TaskWorkers>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::Designation>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::TaskSlots>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::TargetMixer>();
}
