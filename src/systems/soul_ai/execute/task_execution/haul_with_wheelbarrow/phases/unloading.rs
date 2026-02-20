//! 荷下ろしフェーズ

use super::super::cancel;
use crate::constants::Z_ITEM_PICKUP;
use crate::relationships::{LoadedIn, StoredIn};
use crate::systems::logistics::transport_request::{
    TransportRequestKind, TransportRequestState, WheelbarrowDestination,
};
use crate::systems::soul_ai::execute::task_execution::{
    common::clear_task_and_path,
    context::TaskExecutionContext,
    transport_common::{reservation, wheelbarrow as wheelbarrow_common},
    types::HaulWithWheelbarrowData,
};
use bevy::prelude::*;

fn has_pending_wheelbarrow_task(ctx: &TaskExecutionContext) -> bool {
    ctx.queries.transport_request_status.iter().any(
        |(request, demand, state, lease_opt, workers_opt)| {
            let worker_count = workers_opt.map(|workers| workers.len()).unwrap_or(0);
            if *state != TransportRequestState::Pending
                || demand.remaining() == 0
                || lease_opt.is_some()
                || worker_count > 0
            {
                return false;
            }

            match request.kind {
                TransportRequestKind::DepositToStockpile
                | TransportRequestKind::DeliverToFloorConstruction
                | TransportRequestKind::DeliverToWallConstruction
                | TransportRequestKind::DeliverToMixerSolid => true,
                TransportRequestKind::DeliverToBlueprint => {
                    request.resource_type.requires_wheelbarrow()
                }
                _ => false,
            }
        },
    )
}

fn try_drop_item(
    commands: &mut Commands,
    item_entity: Entity,
    drop_pos: Vec2,
    store_in: Option<Entity>,
) -> bool {
    let Ok(mut item_commands) = commands.get_entity(item_entity) else {
        return false;
    };

    if let Some(stockpile) = store_in {
        item_commands.try_insert((
            Visibility::Visible,
            Transform::from_xyz(drop_pos.x, drop_pos.y, Z_ITEM_PICKUP),
            StoredIn(stockpile),
        ));
    } else {
        item_commands.try_insert((
            Visibility::Visible,
            Transform::from_xyz(drop_pos.x, drop_pos.y, Z_ITEM_PICKUP),
        ));
        item_commands.try_remove::<StoredIn>();
    }
    item_commands.try_remove::<LoadedIn>();
    item_commands.try_remove::<crate::relationships::DeliveringTo>();
    item_commands.try_remove::<crate::systems::jobs::IssuedBy>();
    item_commands.try_remove::<crate::relationships::TaskWorkers>();
    true
}

fn try_despawn_item(commands: &mut Commands, item_entity: Entity) -> bool {
    let Ok(mut item_commands) = commands.get_entity(item_entity) else {
        return false;
    };
    item_commands.try_despawn();
    true
}

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    soul_pos: Vec2,
) {
    let unique_items: std::collections::HashSet<Entity> = data.items.iter().copied().collect();
    if unique_items.len() < data.items.len() {
        warn!(
            "WB_HAUL: duplicate items detected in unload list (total={}, unique={})",
            data.items.len(),
            unique_items.len()
        );
    }

    let mut deduped_items = std::collections::HashSet::new();
    let item_types: Vec<(Entity, Option<crate::systems::logistics::ResourceType>)> = data
        .items
        .iter()
        .filter_map(|&item_entity| {
            if !deduped_items.insert(item_entity) {
                return None;
            }
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
                let stock_pos = stock_transform.translation.truncate();
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

                    if try_drop_item(commands, *item_entity, stock_pos, Some(dest_stockpile)) {
                        destination_store_count += 1;
                        unloaded_count += 1;
                    }
                }
            } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(dest_stockpile) {
                let site_pos = site.material_center;
                for (index, (item_entity, _)) in item_types.iter().enumerate() {
                    let offset = Vec2::new((index as f32) * 2.0, 0.0);
                    if try_drop_item(commands, *item_entity, site_pos + offset, None) {
                        destination_store_count += 1;
                        unloaded_count += 1;
                    }
                }
            } else if let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(dest_stockpile) {
                let site_pos = site.material_center;
                for (index, (item_entity, _)) in item_types.iter().enumerate() {
                    let offset = Vec2::new((index as f32) * 2.0, 0.0);
                    if try_drop_item(commands, *item_entity, site_pos + offset, None) {
                        destination_store_count += 1;
                        unloaded_count += 1;
                    }
                }
            } else if let Ok((wall_transform, building, _)) =
                ctx.queries.storage.buildings.get(dest_stockpile)
            {
                if building.kind == crate::systems::jobs::BuildingType::Wall
                    && building.is_provisional
                {
                    let site_pos = wall_transform.translation.truncate();
                    for (index, (item_entity, _)) in item_types.iter().enumerate() {
                        let offset = Vec2::new((index as f32) * 2.0, 0.0);
                        if try_drop_item(commands, *item_entity, site_pos + offset, None) {
                            destination_store_count += 1;
                            unloaded_count += 1;
                        }
                    }
                } else {
                    cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                    return;
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
                    if try_despawn_item(commands, *item_entity) {
                        destination_store_count += 1;
                        unloaded_count += 1;
                    }
                }

                if blueprint.materials_complete() {
                    if let Ok(mut blueprint_commands) = commands.get_entity(blueprint_entity) {
                        blueprint_commands.try_remove::<crate::relationships::ManagedBy>();
                        blueprint_commands.try_insert(crate::systems::jobs::Priority(10));
                    }
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
                        if try_despawn_item(commands, *item_entity) {
                            unloaded_count += 1;
                        }
                    } else if let Ok(mut item_commands) = commands.get_entity(*item_entity) {
                        item_commands.try_remove::<LoadedIn>();
                        item_commands.try_remove::<crate::relationships::DeliveringTo>();
                        item_commands.try_insert((
                            Visibility::Visible,
                            Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
                        ));

                        if matches!(
                            Some(res_type),
                            Some(crate::systems::logistics::ResourceType::Sand)
                                | Some(crate::systems::logistics::ResourceType::StasisMud)
                        ) {
                            item_commands.try_insert(
                                crate::systems::logistics::item_lifetime::ItemDespawnTimer::new(
                                    5.0,
                                ),
                            );
                        }
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

    reservation::release_source(ctx, data.wheelbarrow, 1);
    // unloading 内で despawn 済みの積載物へ追加入力しないよう、loaded cleanup はスキップする。
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
        soul_commands.try_remove::<crate::relationships::WorkingOn>();
    }
    clear_task_and_path(ctx.task, ctx.path);

    if has_pending_wheelbarrow_task(ctx) {
        info!(
            "WB_HAUL: Soul {:?} kept wheelbarrow {:?} for next assignment",
            ctx.soul_entity, data.wheelbarrow
        );
    } else {
        info!(
            "WB_HAUL: Soul {:?} parked wheelbarrow {:?} and waits low-priority return",
            ctx.soul_entity, data.wheelbarrow
        );
    }
}
