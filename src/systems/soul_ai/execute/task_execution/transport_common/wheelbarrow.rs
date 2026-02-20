//! 手押し車の駐車・キャンセル時の共通処理
//!
//! park/cancel/reset を共通化し、予約解放漏れを防ぐ。

use crate::constants::Z_ITEM_PICKUP;
use crate::relationships::{ParkedAt, PushedBy};
use crate::systems::soul_ai::execute::task_execution::{
    common::clear_task_and_path, context::TaskExecutionContext, types::HaulWithWheelbarrowData,
};
use crate::systems::visual::haul::WheelbarrowMovement;
use bevy::prelude::*;

/// 手押し車を駐車状態に戻し、指定位置に配置
pub fn park_wheelbarrow_entity(
    commands: &mut Commands,
    wheelbarrow: Entity,
    parking_anchor: Option<Entity>,
    pos: Vec2,
) {
    let Ok(mut wheelbarrow_commands) = commands.get_entity(wheelbarrow) else {
        return;
    };
    if let Some(anchor) = parking_anchor {
        wheelbarrow_commands.try_insert(ParkedAt(anchor));
    }
    wheelbarrow_commands.try_remove::<(PushedBy, WheelbarrowMovement)>();
    wheelbarrow_commands.try_remove::<crate::relationships::DeliveringTo>();
    wheelbarrow_commands.try_insert((
        Visibility::Visible,
        Transform::from_xyz(pos.x, pos.y, Z_ITEM_PICKUP),
    ));
}

/// 手押し車タスクを完了（駐車 + コンテキストクリア）
pub fn complete_wheelbarrow_task(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    pos: Vec2,
) {
    if let Ok(loaded_items) = ctx.queries.storage.loaded_items.get(data.wheelbarrow) {
        let still_loaded: Vec<Entity> = loaded_items
            .iter()
            .filter(|item_entity| {
                ctx.queries
                    .storage
                    .loaded_in
                    .get(*item_entity)
                    .ok()
                    .is_some_and(|loaded_in| loaded_in.0 == data.wheelbarrow)
            })
            .collect();

        for item_entity in still_loaded {
            if let Ok(mut item_commands) = commands.get_entity(item_entity) {
                item_commands.try_remove::<crate::relationships::LoadedIn>();
                item_commands.try_remove::<crate::relationships::DeliveringTo>();
                item_commands.try_insert((
                    Visibility::Visible,
                    Transform::from_xyz(pos.x, pos.y, Z_ITEM_PICKUP),
                ));
            }
        }
    }

    let parking_anchor = ctx
        .queries
        .designation
        .belongs
        .get(data.wheelbarrow)
        .ok()
        .map(|b| b.0);

    park_wheelbarrow_entity(commands, data.wheelbarrow, parking_anchor, pos);
    ctx.inventory.0 = None;
    if let Ok(mut soul_commands) = commands.get_entity(ctx.soul_entity) {
        soul_commands.try_remove::<crate::relationships::WorkingOn>();
    }
    clear_task_and_path(ctx.task, ctx.path);
}
