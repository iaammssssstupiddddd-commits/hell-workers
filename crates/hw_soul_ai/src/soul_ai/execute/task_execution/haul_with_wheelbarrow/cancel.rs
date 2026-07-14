//! 手押し車タスクのキャンセル・予約解放

use crate::soul_ai::execute::task_execution::{
    context::{TaskExecutionContext, TaskHandlerControl},
    transport_common::wheelbarrow as wheelbarrow_common,
    types::HaulWithWheelbarrowData,
};
use bevy::prelude::*;
use hw_core::constants::Z_ITEM_PICKUP;

/// 手押し車タスクのキャンセル処理（全フェーズ共通）
/// 積載済みアイテムを地面にドロップし、猫車を駐車に戻す。
pub fn cancel_wheelbarrow_task(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let soul_pos = ctx.soul_pos();
    // 積載済みアイテムを地面にドロップ
    if let Ok(loaded_items) = ctx.queries.storage.loaded_items.get(data.wheelbarrow) {
        for item_entity in loaded_items.iter() {
            if let Ok(mut item_commands) = commands.get_entity(item_entity) {
                item_commands.try_insert((
                    Visibility::Visible,
                    Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
                ));
                item_commands.try_remove::<hw_core::relationships::DeliveringTo>();
                item_commands.try_remove::<hw_core::relationships::LoadedIn>();
            }
        }
    }
    for &item_entity in &data.items {
        if let Ok(mut item_commands) = commands.get_entity(item_entity) {
            item_commands.try_remove::<hw_core::relationships::DeliveringTo>();
        }
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
    if let Ok(mut wheelbarrow_commands) = commands.get_entity(data.wheelbarrow) {
        wheelbarrow_commands.try_remove::<hw_core::relationships::DeliveringTo>();
    }

    debug!(
        "WB_HAUL: Soul {:?} canceled wheelbarrow task",
        ctx.soul_entity
    );
    ctx.inventory.0 = None;
    ctx.abort_retryable_after_custom_cleanup(commands, "wheelbarrow haul canceled")
}

/// 宛先が破壊された場合: アイテムを地面にドロップしてキャンセル。
pub fn drop_items_and_cancel(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    cancel_wheelbarrow_task(ctx, data, commands)
}
