//! 手押し車タスクのキャンセル・予約解放

use crate::constants::Z_ITEM_PICKUP;
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use crate::systems::soul_ai::execute::task_execution::{
    common::clear_task_and_path,
    context::TaskExecutionContext,
    transport_common::{reservation, wheelbarrow as wheelbarrow_common},
    types::HaulWithWheelbarrowData,
};
use bevy::prelude::*;

/// 手押し車タスクのキャンセル処理（全フェーズ共通）
/// 積載済みアイテムを地面にドロップし、猫車を駐車に戻し、全予約を解放する。
pub fn cancel_wheelbarrow_task(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
    let soul_pos = ctx.soul_pos();
    // 積載済みアイテムを地面にドロップ
    if let Some(loaded_items) = ctx.queries.storage.loaded_items.get(data.wheelbarrow).ok() {
        for item_entity in loaded_items.iter() {
            commands.entity(item_entity).insert((
                Visibility::Visible,
                Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
            ));
            commands
                .entity(item_entity)
                .remove::<crate::relationships::DeliveringTo>();
            commands
                .entity(item_entity)
                .remove::<crate::relationships::LoadedIn>();
        }
    }
    for &item_entity in &data.items {
        commands
            .entity(item_entity)
            .remove::<crate::relationships::DeliveringTo>();
    }

    // 手押し車を駐車状態に戻す
    let parking_anchor = ctx
        .queries
        .designation
        .belongs
        .get(data.wheelbarrow)
        .ok()
        .map(|b| b.0);
    wheelbarrow_common::park_wheelbarrow_entity(
        commands,
        data.wheelbarrow,
        parking_anchor,
        soul_pos,
    );
    commands
        .entity(data.wheelbarrow)
        .remove::<crate::relationships::DeliveringTo>();

    // 全予約を解放
    release_all_reservations(ctx, data);

    ctx.inventory.0 = None;
    commands
        .entity(ctx.soul_entity)
        .remove::<crate::relationships::WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);

    info!(
        "WB_HAUL: Soul {:?} canceled wheelbarrow task",
        ctx.soul_entity
    );
}

/// 宛先が破壊された場合: アイテムを地面にドロップしてキャンセル
/// cancel_wheelbarrow_task と同等だが、宛先の予約も確実に解放する。
pub fn drop_items_and_cancel(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
    cancel_wheelbarrow_task(ctx, data, commands);
}

/// 全アイテムの予約（ソース + 宛先）を解放
pub fn release_all_reservations(ctx: &mut TaskExecutionContext, data: &HaulWithWheelbarrowData) {
    reservation::release_source(ctx, data.wheelbarrow, 1);

    if let Some(source_entity) = data.collect_source {
        reservation::release_source(ctx, source_entity, 1);
    }

    for &item_entity in &data.items {
        reservation::release_source(ctx, item_entity, 1);

        match data.destination {
            WheelbarrowDestination::Stockpile(_) | WheelbarrowDestination::Blueprint(_) => {
                // DeliveringTo リレーションシップの削除は
                // cancel_wheelbarrow_task や unload 各所で行われる。
            }
            WheelbarrowDestination::Mixer {
                entity: target,
                resource_type,
            } => {
                let item_type = ctx
                    .queries
                    .reservation
                    .resources
                    .get(item_entity)
                    .ok()
                    .map(|r| r.0)
                    .unwrap_or(resource_type);
                reservation::release_mixer_destination(ctx, target, item_type);
            }
        }
    }
}
