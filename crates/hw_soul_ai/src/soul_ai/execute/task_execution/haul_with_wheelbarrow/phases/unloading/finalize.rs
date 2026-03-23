use bevy::prelude::*;
use hw_core::constants::Z_ITEM_PICKUP;
use hw_core::relationships::LoadedIn;
use hw_logistics::ResourceType;
use hw_logistics::transport_request::WheelbarrowDestination;
use std::collections::HashSet;

use crate::soul_ai::execute::task_execution::{
    common::clear_task_and_path,
    context::TaskExecutionContext,
    transport_common::{reservation, wheelbarrow as wheelbarrow_common},
    types::HaulWithWheelbarrowData,
};

pub(super) fn finalize_unload_task(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
    soul_pos: Vec2,
) {
    reservation::release_source(ctx, data.wheelbarrow, 1);
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
    ctx.inventory.0 = None;
    if let Ok(mut soul_commands) = commands.get_entity(ctx.soul_entity) {
        soul_commands.try_remove::<hw_core::relationships::WorkingOn>();
    }
    clear_task_and_path(ctx.task, ctx.path);
}

pub(super) fn finish_partial_unload(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
    soul_pos: Vec2,
    delivered_items: &HashSet<Entity>,
    destination_store_count: usize,
    mixer_release_types: &[ResourceType],
) {
    for &item_entity in &data.items {
        if delivered_items.contains(&item_entity) {
            continue;
        }
        if let Ok(mut item_commands) = commands.get_entity(item_entity) {
            item_commands.try_insert((
                Visibility::Visible,
                Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
            ));
            item_commands.try_remove::<LoadedIn>();
            item_commands.try_remove::<hw_core::relationships::DeliveringTo>();
        }
    }

    match data.destination {
        WheelbarrowDestination::Stockpile(target) | WheelbarrowDestination::Blueprint(target) => {
            for _ in 0..destination_store_count {
                reservation::record_stored_destination(ctx, target);
            }
        }
        WheelbarrowDestination::Mixer { entity: target, .. } => {
            for &res_type in mixer_release_types {
                reservation::release_mixer_destination(ctx, target, res_type);
            }
        }
    }

    finalize_unload_task(ctx, data, commands, soul_pos);
}
