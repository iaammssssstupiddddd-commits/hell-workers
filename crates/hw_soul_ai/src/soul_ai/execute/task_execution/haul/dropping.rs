use crate::soul_ai::execute::task_execution::chain;
use crate::soul_ai::execute::task_execution::common::drop_item;
use crate::soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskHandlerControl};
use crate::soul_ai::execute::task_execution::stockpile_policy::{
    RuntimeStockpileInboundInput, evaluate_runtime_stockpile_inbound, inbound_reservation_snapshot,
};
use crate::soul_ai::execute::task_execution::transport_common::{cancel, reservation};
use bevy::prelude::*;
use hw_core::constants::Z_ITEM_PICKUP;
use hw_jobs::BuildingType;
use hw_logistics::{
    ResourceType, count_nearby_ground_resources, floor_site_tile_demand,
    provisional_wall_mud_demand, wall_site_tile_demand,
};
use std::collections::HashSet;

fn floor_site_can_accept(
    ctx: &TaskExecutionContext,
    site_entity: Entity,
    resource_type: ResourceType,
    exclude_item: Entity,
) -> bool {
    let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(site_entity) else {
        return false;
    };

    let needed = floor_site_tile_demand(
        ctx.queries.storage.floor_tiles.iter().map(|(_, t, _)| t),
        site_entity,
        resource_type,
    );
    let nearby = count_nearby_ground_resources(
        ctx.queries.resource_items.iter(),
        site.material_center,
        (hw_core::constants::TILE_SIZE * 2.0).powi(2),
        resource_type,
        Some(exclude_item),
    );
    needed > nearby
}

fn wall_site_can_accept(
    ctx: &TaskExecutionContext,
    site_entity: Entity,
    resource_type: ResourceType,
    exclude_item: Entity,
) -> bool {
    let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(site_entity) else {
        return false;
    };

    let needed = wall_site_tile_demand(
        ctx.queries.storage.wall_tiles.iter().map(|(_, t, _)| t),
        site_entity,
        resource_type,
    );
    let nearby = count_nearby_ground_resources(
        ctx.queries.resource_items.iter(),
        site.material_center,
        (hw_core::constants::TILE_SIZE * 2.0).powi(2),
        resource_type,
        Some(exclude_item),
    );
    needed > nearby
}

fn provisional_wall_can_accept(
    ctx: &TaskExecutionContext,
    resource_type: Option<ResourceType>,
    wall_entity: Entity,
    building: &hw_jobs::Building,
    provisional_opt: Option<&hw_jobs::ProvisionalWall>,
    wall_pos: Vec2,
    exclude_item: Entity,
) -> bool {
    if wall_entity == Entity::PLACEHOLDER {
        return false;
    }
    if !matches!(resource_type, Some(ResourceType::StasisMud)) {
        return false;
    }
    if building.kind != BuildingType::Wall || !building.is_provisional {
        return false;
    }
    if provisional_wall_mud_demand(building, provisional_opt) == 0 {
        return false;
    }
    count_nearby_ground_resources(
        ctx.queries.resource_items.iter(),
        wall_pos,
        (hw_core::constants::TILE_SIZE * 1.5).powi(2),
        ResourceType::StasisMud,
        Some(exclude_item),
    ) == 0
}

pub(super) fn handle_dropping_phase(
    ctx: &mut TaskExecutionContext,
    item: Entity,
    stockpile: Entity,
    commands: &mut Commands,
    soul_pos: Vec2,
) -> TaskHandlerControl {
    let q_targets = &ctx.queries.designation.targets;
    let q_belongs = &ctx.queries.designation.belongs;
    let item_resource_type =
        q_targets
            .get(item)
            .ok()
            .and_then(|(_, _, _, _, resource_item_opt, _, _)| {
                resource_item_opt.map(|resource_item| resource_item.0)
            });
    let is_bucket_storage = ctx.queries.storage.bucket_storages.get(stockpile).is_ok();
    let current_policy = ctx.queries.stockpile_policies.get(stockpile).ok().copied();
    let stock_belongs = q_belongs.get(stockpile).ok().map(|belongs| belongs.0);
    let item_belongs = q_belongs.get(item).ok().map(|belongs| belongs.0);
    let reservation_snapshot = item_resource_type.map(|resource_type| {
        inbound_reservation_snapshot(
            stockpile,
            resource_type,
            &HashSet::from([item]),
            &ctx.queries.reservation.incoming_deliveries_query,
            &ctx.queries.reservation.resources,
        )
    });

    if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
        ctx.queries.storage.stockpiles.get_mut(stockpile)
    {
        let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
        let item_info = item_resource_type.map(|resource_type| (resource_type, item_belongs));
        let can_drop = if let Some((res_type, item_belongs)) = item_info {
            let belongs_match = item_belongs == stock_belongs;
            let accepts_unowned_for_owned = item_belongs.is_none() && stock_belongs.is_some();

            let is_bucket_item = matches!(
                res_type,
                hw_logistics::ResourceType::BucketEmpty | hw_logistics::ResourceType::BucketWater
            );
            let type_match = stockpile_comp.resource_type.is_none()
                || stockpile_comp.resource_type == Some(res_type);

            let ownership_ok = if is_bucket_storage {
                stock_belongs.is_some() && item_belongs.is_some() && belongs_match
            } else {
                belongs_match || accepts_unowned_for_owned
            };

            let incoming_count = reservation_snapshot
                .map(|snapshot| snapshot.incoming_reserved)
                .unwrap_or(0);
            let type_and_capacity_allowed = if is_bucket_storage {
                let bucket_storage_type_ok = matches!(
                    stockpile_comp.resource_type,
                    None | Some(hw_logistics::ResourceType::BucketEmpty)
                        | Some(hw_logistics::ResourceType::BucketWater)
                );
                is_bucket_item
                    && bucket_storage_type_ok
                    && (current_count + incoming_count) <= stockpile_comp.capacity
            } else if let (Some(policy), Some(reservations)) =
                (current_policy, reservation_snapshot)
            {
                res_type.can_store_in_stockpile()
                    && evaluate_runtime_stockpile_inbound(RuntimeStockpileInboundInput {
                        policy,
                        capacity: stockpile_comp.capacity,
                        stored_amount: current_count,
                        stored_resource: stockpile_comp.resource_type,
                        transfer_resource: res_type,
                        requested_amount: 1,
                        reservations,
                        cycle_reserved: 0,
                        cycle_reserved_other_resource: 0,
                    })
                    .allowed_amount
                        == 1
            } else {
                type_match
                    && res_type.can_store_in_stockpile()
                    && (current_count + incoming_count) <= stockpile_comp.capacity
            };

            ownership_ok && type_and_capacity_allowed
        } else {
            false
        };

        if can_drop {
            if !is_bucket_storage
                && (current_count == 0 || stockpile_comp.resource_type.is_none())
                && let Some((res_type, _)) = item_info
            {
                stockpile_comp.resource_type = Some(res_type);
            }

            if !is_bucket_storage
                && q_belongs.get(item).is_err()
                && let Some(owner) = stock_belongs
            {
                // owner未設定資源を owner 付きストックパイルに入れたときは ownership を確定する。
                commands
                    .entity(item)
                    .try_insert(hw_logistics::BelongsTo(owner));
            }

            commands.entity(item).try_insert((
                Visibility::Visible,
                Transform::from_xyz(
                    stock_transform.translation.x,
                    stock_transform.translation.y,
                    0.6,
                ),
                hw_core::relationships::StoredIn(stockpile),
            ));
            commands
                .entity(item)
                .remove::<hw_core::relationships::DeliveringTo>();
            commands.entity(item).remove::<hw_jobs::IssuedBy>();

            reservation::record_stored_destination(ctx, stockpile);
            debug!(
                "TASK_EXEC: Soul {:?} dropped item at stockpile. Count ~ {}",
                ctx.soul_entity, current_count
            );
        } else {
            return ctx.abort_retryable(commands, "stockpile no longer accepts item");
        }
    } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(stockpile) {
        if !item_resource_type
            .is_some_and(|resource_type| floor_site_can_accept(ctx, stockpile, resource_type, item))
        {
            return cancel::cancel_haul_to_stockpile(ctx, item, stockpile, commands);
        }
        let material_center = site.material_center;
        // `site` はここで最後に使用される → NLL によりこれ以降の借用は解放される
        commands.entity(item).try_insert((
            Visibility::Visible,
            Transform::from_xyz(material_center.x, material_center.y, Z_ITEM_PICKUP),
        ));
        commands
            .entity(item)
            .remove::<hw_core::relationships::StoredIn>();
        commands
            .entity(item)
            .remove::<hw_core::relationships::DeliveringTo>();
        commands.entity(item).remove::<hw_jobs::IssuedBy>();

        // チェーン判定: そのまま床工事タスクに移行できるか確認
        if let Some(resource_type) = item_resource_type
            && let Some(opp) = chain::find_chain_opportunity(stockpile, resource_type, None, ctx)
        {
            ctx.inventory.0 = None;
            chain::execute_chain(opp, ctx, commands);
            return TaskHandlerControl::Continue;
        }
    } else if let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(stockpile) {
        if !item_resource_type
            .is_some_and(|resource_type| wall_site_can_accept(ctx, stockpile, resource_type, item))
        {
            return cancel::cancel_haul_to_stockpile(ctx, item, stockpile, commands);
        }
        let material_center = site.material_center;
        // `site` はここで最後に使用される → NLL によりこれ以降の借用は解放される
        commands.entity(item).try_insert((
            Visibility::Visible,
            Transform::from_xyz(material_center.x, material_center.y, Z_ITEM_PICKUP),
        ));
        commands
            .entity(item)
            .remove::<hw_core::relationships::StoredIn>();
        commands
            .entity(item)
            .remove::<hw_core::relationships::DeliveringTo>();
        commands.entity(item).remove::<hw_jobs::IssuedBy>();

        // チェーン判定: そのまま壁工事タスクに移行できるか確認
        if let Some(resource_type) = item_resource_type
            && let Some(opp) = chain::find_chain_opportunity(stockpile, resource_type, None, ctx)
        {
            ctx.inventory.0 = None;
            chain::execute_chain(opp, ctx, commands);
            return TaskHandlerControl::Continue;
        }
    } else if let Ok((wall_transform, building, provisional_opt)) =
        ctx.queries.storage.buildings.get(stockpile)
    {
        let wall_pos = wall_transform.translation.truncate();
        let can_deliver_to_wall = provisional_wall_can_accept(
            ctx,
            item_resource_type,
            stockpile,
            building,
            provisional_opt,
            wall_pos,
            item,
        );
        if can_deliver_to_wall {
            commands.entity(item).try_insert((
                Visibility::Visible,
                Transform::from_xyz(wall_pos.x, wall_pos.y, Z_ITEM_PICKUP),
            ));
            commands
                .entity(item)
                .remove::<hw_core::relationships::StoredIn>();
            commands
                .entity(item)
                .remove::<hw_core::relationships::DeliveringTo>();
            commands.entity(item).remove::<hw_jobs::IssuedBy>();
        } else if ctx.inventory.0.is_some() {
            return cancel::cancel_haul_to_stockpile(ctx, item, stockpile, commands);
        }
    } else if ctx.inventory.0.is_some() {
        drop_item(commands, ctx.soul_entity, item, soul_pos);
    }

    ctx.inventory.0 = None;
    ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
    ctx.complete_task(commands, "haul to stockpile done")
}
