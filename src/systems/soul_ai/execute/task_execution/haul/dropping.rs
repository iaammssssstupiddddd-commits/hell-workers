use hw_core::constants::Z_ITEM_PICKUP;
use crate::relationships::WorkingOn;
use crate::systems::jobs::BuildingType;
use crate::systems::logistics::{
    count_nearby_ground_resources, floor_site_tile_demand, provisional_wall_mud_demand,
    wall_site_tile_demand,
    ResourceType,
};
use crate::systems::soul_ai::execute::task_execution::common::{clear_task_and_path, drop_item};
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::transport_common::{cancel, reservation};
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMap;
use bevy::prelude::*;

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
        ctx.queries.storage.floor_tiles.iter(),
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
        ctx.queries.storage.wall_tiles.iter(),
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
    building: &crate::systems::jobs::Building,
    provisional_opt: Option<&crate::systems::jobs::ProvisionalWall>,
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
    world_map: &Res<WorldMap>,
    soul_pos: Vec2,
) {
    let q_targets = &ctx.queries.designation.targets;
    let q_belongs = &ctx.queries.designation.belongs;
    let item_resource_type = q_targets
        .get(item)
        .ok()
        .and_then(|(_, _, _, _, resource_item_opt, _, _)| resource_item_opt.map(|resource_item| resource_item.0));

    if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
        ctx.queries.storage.stockpiles.get_mut(stockpile)
    {
        let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
        let is_bucket_storage = ctx.queries.storage.bucket_storages.get(stockpile).is_ok();
        let stock_belongs = q_belongs.get(stockpile).ok().map(|b| b.0);

        let item_info = q_targets.get(item).ok().map(|(_, _, _, _, ri, _, _)| {
            let res_type = ri.map(|r| r.0);
            let belongs = q_belongs.get(item).ok().map(|b| b.0);
            (res_type, belongs)
        });
        let can_drop = if let Some((Some(res_type), item_belongs)) = item_info {
            let belongs_match = item_belongs == stock_belongs;
            let accepts_unowned_for_owned = item_belongs.is_none() && stock_belongs.is_some();

            let is_bucket_item = matches!(
                res_type,
                crate::systems::logistics::ResourceType::BucketEmpty
                    | crate::systems::logistics::ResourceType::BucketWater
            );
            let type_match = stockpile_comp.resource_type.is_none()
                || stockpile_comp.resource_type == Some(res_type);

            let ownership_ok = if is_bucket_storage {
                stock_belongs.is_some() && item_belongs.is_some() && belongs_match
            } else {
                belongs_match || accepts_unowned_for_owned
            };

            let type_allowed = if is_bucket_storage {
                let bucket_storage_type_ok = matches!(
                    stockpile_comp.resource_type,
                    None | Some(crate::systems::logistics::ResourceType::BucketEmpty)
                        | Some(crate::systems::logistics::ResourceType::BucketWater)
                );
                is_bucket_item && bucket_storage_type_ok
            } else {
                type_match && res_type.can_store_in_stockpile()
            };

            let incoming_count = ctx
                .queries
                .reservation
                .incoming_deliveries_query
                .get(stockpile)
                .ok()
            .map(|(_, incoming)| incoming.len())
                .unwrap_or(0);
            let capacity_ok = (current_count + incoming_count) <= stockpile_comp.capacity;

            ownership_ok && type_allowed && capacity_ok
        } else {
            false
        };

        if can_drop {
            if !is_bucket_storage && stockpile_comp.resource_type.is_none() {
                if let Some((res_type, _)) = item_info {
                    stockpile_comp.resource_type = res_type;
                }
            }

            if !is_bucket_storage
                && q_belongs.get(item).is_err()
                && let Some(owner) = stock_belongs
            {
                // owner未設定資源を owner 付きストックパイルに入れたときは ownership を確定する。
                commands
                    .entity(item)
                    .try_insert(crate::systems::logistics::BelongsTo(owner));
            }

            commands.entity(item).try_insert((
                Visibility::Visible,
                Transform::from_xyz(
                    stock_transform.translation.x,
                    stock_transform.translation.y,
                    0.6,
                ),
                crate::relationships::StoredIn(stockpile),
            ));
            commands
                .entity(item)
                .remove::<crate::relationships::DeliveringTo>();
            commands
                .entity(item)
                .remove::<crate::systems::jobs::IssuedBy>();
            commands
                .entity(item)
                .remove::<crate::relationships::TaskWorkers>();

            reservation::record_stored_destination(ctx, stockpile);
            info!(
                "TASK_EXEC: Soul {:?} dropped item at stockpile. Count ~ {}",
                ctx.soul_entity, current_count
            );
        } else {
            unassign_task(
                commands,
                ctx.soul_entity,
                soul_pos,
                ctx.task,
                ctx.path,
                Some(ctx.inventory),
                item_info.and_then(|(it, _)| it),
                ctx.queries,
                world_map,
                true,
            );
        }
    } else if let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(stockpile) {
        if !item_resource_type
            .is_some_and(|resource_type| floor_site_can_accept(ctx, stockpile, resource_type, item))
        {
            cancel::cancel_haul_to_stockpile(ctx, item, stockpile, commands);
            return;
        }
        commands.entity(item).try_insert((
            Visibility::Visible,
            Transform::from_xyz(
                site.material_center.x,
                site.material_center.y,
                Z_ITEM_PICKUP,
            ),
        ));
        commands
            .entity(item)
            .remove::<crate::relationships::StoredIn>();
        commands
            .entity(item)
            .remove::<crate::relationships::DeliveringTo>();
        commands
            .entity(item)
            .remove::<crate::systems::jobs::IssuedBy>();
        commands
            .entity(item)
            .remove::<crate::relationships::TaskWorkers>();
    } else if let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(stockpile) {
        if !item_resource_type
            .is_some_and(|resource_type| wall_site_can_accept(ctx, stockpile, resource_type, item))
        {
            cancel::cancel_haul_to_stockpile(ctx, item, stockpile, commands);
            return;
        }
        commands.entity(item).try_insert((
            Visibility::Visible,
            Transform::from_xyz(
                site.material_center.x,
                site.material_center.y,
                Z_ITEM_PICKUP,
            ),
        ));
        commands
            .entity(item)
            .remove::<crate::relationships::StoredIn>();
        commands
            .entity(item)
            .remove::<crate::relationships::DeliveringTo>();
        commands
            .entity(item)
            .remove::<crate::systems::jobs::IssuedBy>();
        commands
            .entity(item)
            .remove::<crate::relationships::TaskWorkers>();
    } else if let Ok((wall_transform, building, provisional_opt)) =
        ctx.queries.storage.buildings.get(stockpile)
    {
        let wall_pos = wall_transform.translation.truncate();
        let can_deliver_to_wall = provisional_wall_can_accept(
            ctx,
            item_resource_type,
            stockpile,
            &building,
            provisional_opt.as_deref(),
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
                .remove::<crate::relationships::StoredIn>();
            commands
                .entity(item)
                .remove::<crate::relationships::DeliveringTo>();
            commands
                .entity(item)
                .remove::<crate::systems::jobs::IssuedBy>();
            commands
                .entity(item)
                .remove::<crate::relationships::TaskWorkers>();
        } else if ctx.inventory.0.is_some() {
            cancel::cancel_haul_to_stockpile(ctx, item, stockpile, commands);
            return;
        }
    } else if ctx.inventory.0.is_some() {
        drop_item(commands, ctx.soul_entity, item, soul_pos);
    }

    ctx.inventory.0 = None;
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);
    ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
}
