//! 荷下ろしフェーズ

use crate::constants::Z_ITEM_PICKUP;
use crate::relationships::{LoadedIn, StoredIn};
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use crate::systems::soul_ai::execute::task_execution::{
    context::TaskExecutionContext,
    transport_common::reservation,
    types::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use super::super::cancel;
use bevy::prelude::*;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    soul_pos: Vec2,
) {
    let item_types: Vec<(Entity, Option<crate::systems::logistics::ResourceType>)> = data
        .items
        .iter()
        .filter_map(|&item_entity| {
            let Ok((_, _, _, _, res_item_opt, _, _)) =
                ctx.queries.designation.targets.get(item_entity)
            else {
                return None;
            };
            Some((item_entity, res_item_opt.map(|r| r.0)))
        })
        .collect();

    let mut unloaded_count = 0usize;
    let mut destination_store_count = 0usize;
    let mut mixer_release_types = Vec::new();

    match data.destination {
        WheelbarrowDestination::Stockpile(dest_stockpile) => {
            if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
                ctx.queries.storage.stockpiles.get_mut(dest_stockpile)
            {
                let stock_pos = stock_transform.translation;
                let incoming_total = ctx
                    .queries
                    .reservation
                    .incoming_deliveries_query
                    .get(dest_stockpile)
                    .ok()
                    .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
                    .unwrap_or(0);
                // `incoming_total` には自分が運んでいるアイテムも含まれるため、
                // 他タスク分だけを容量判定に使う。
                let incoming_self = incoming_total.min(item_types.len());
                let incoming_other = incoming_total.saturating_sub(incoming_self);
                let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

                for (item_entity, res_type_opt) in &item_types {
                    if current_count + incoming_other + unloaded_count >= stockpile_comp.capacity {
                        break;
                    }
                    let Some(res_type) = res_type_opt else {
                        continue;
                    };

                    if stockpile_comp.resource_type.is_none() {
                        stockpile_comp.resource_type = Some(*res_type);
                    } else if stockpile_comp.resource_type != Some(*res_type) {
                        continue;
                    }
                    if !res_type.can_store_in_stockpile() {
                        continue;
                    }

                    commands.entity(*item_entity).insert((
                        Visibility::Visible,
                        Transform::from_xyz(stock_pos.x, stock_pos.y, Z_ITEM_PICKUP),
                        StoredIn(dest_stockpile),
                    ));
                    commands.entity(*item_entity).remove::<LoadedIn>();
                    commands.entity(*item_entity).remove::<crate::relationships::DeliveringTo>();
                    commands
                        .entity(*item_entity)
                        .remove::<crate::systems::jobs::IssuedBy>();
                    commands
                        .entity(*item_entity)
                        .remove::<crate::relationships::TaskWorkers>();

                    destination_store_count += 1;
                    unloaded_count += 1;
                }
            } else {
                cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                return;
            }
        }
        WheelbarrowDestination::Blueprint(blueprint_entity) => {
            if let Ok((_, mut blueprint, _)) =
                ctx.queries.storage.blueprints.get_mut(blueprint_entity)
            {
                for (item_entity, res_type_opt) in &item_types {
                    let Some(res_type) = res_type_opt else {
                        continue;
                    };

                    blueprint.deliver_material(*res_type, 1);
                    commands.entity(*item_entity).despawn();
                    destination_store_count += 1;
                    unloaded_count += 1;
                }

                if blueprint.materials_complete() {
                    commands
                        .entity(blueprint_entity)
                        .remove::<crate::relationships::ManagedBy>();
                    commands
                        .entity(blueprint_entity)
                        .insert(crate::systems::jobs::Priority(10));
                }
            } else {
                info!("WB_HAUL: Blueprint destroyed during unloading, dropping items");
                cancel::drop_items_and_cancel(ctx, &data, commands);
                return;
            }
        }
        WheelbarrowDestination::Mixer {
            entity: mixer_entity,
            resource_type,
        } => {
            if let Ok((_, mut storage, _)) = ctx.queries.storage.mixers.get_mut(mixer_entity) {
                for (item_entity, res_type_opt) in &item_types {
                    let res_type = (*res_type_opt).unwrap_or(resource_type);

                    if storage.add_material(res_type).is_ok() {
                        commands.entity(*item_entity).despawn();
                        unloaded_count += 1;
                    } else {
                        commands.entity(*item_entity).remove::<LoadedIn>();
                        commands
                            .entity(*item_entity)
                            .remove::<crate::relationships::DeliveringTo>();
                        commands.entity(*item_entity).insert((
                            Visibility::Visible,
                            Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
                        ));
                    }

                    mixer_release_types.push(res_type);
                }
            } else {
                info!("WB_HAUL: Mixer destroyed during unloading, dropping items");
                cancel::drop_items_and_cancel(ctx, &data, commands);
                return;
            }
        }
    }

    match data.destination {
        WheelbarrowDestination::Stockpile(target) | WheelbarrowDestination::Blueprint(target) => {
            for _ in 0..destination_store_count {
                reservation::record_stored_destination(ctx, target);
            }
        }
        WheelbarrowDestination::Mixer { entity: target, .. } => {
            for res_type in mixer_release_types {
                reservation::release_mixer_destination(ctx, target, res_type);
            }
        }
    }

    info!(
        "WB_HAUL: Soul {:?} unloaded {} items",
        ctx.soul_entity, unloaded_count
    );

    *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        phase: HaulWithWheelbarrowPhase::ReturningWheelbarrow,
        ..data
    });
}
