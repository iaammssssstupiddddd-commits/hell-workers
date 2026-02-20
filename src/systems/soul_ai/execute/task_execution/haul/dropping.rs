use crate::constants::Z_ITEM_PICKUP;
use crate::relationships::WorkingOn;
use crate::systems::jobs::BuildingType;
use crate::systems::soul_ai::execute::task_execution::common::{clear_task_and_path, drop_item};
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::transport_common::reservation;
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMap;
use bevy::prelude::*;

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

    if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
        ctx.queries.storage.stockpiles.get_mut(stockpile)
    {
        let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
        let is_bucket_storage = ctx.queries.storage.bucket_storages.get(stockpile).is_ok();

        let item_info = q_targets.get(item).ok().map(|(_, _, _, _, ri, _, _)| {
            let res_type = ri.map(|r| r.0);
            let belongs = q_belongs.get(item).ok().map(|b| b.0);
            (res_type, belongs)
        });
        let can_drop = if let Some((Some(res_type), item_belongs)) = item_info {
            let stock_belongs = q_belongs.get(stockpile).ok().map(|b| b.0);
            let belongs_match = item_belongs == stock_belongs;

            let is_bucket_item = matches!(
                res_type,
                crate::systems::logistics::ResourceType::BucketEmpty
                    | crate::systems::logistics::ResourceType::BucketWater
            );
            let type_match =
                stockpile_comp.resource_type.is_none() || stockpile_comp.resource_type == Some(res_type);

            let ownership_ok = if is_bucket_storage {
                stock_belongs.is_some() && item_belongs.is_some() && belongs_match
            } else {
                belongs_match
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
                .map(|incoming: &crate::relationships::IncomingDeliveries| incoming.len())
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

            commands.entity(item).insert((
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
        commands.entity(item).insert((
            Visibility::Visible,
            Transform::from_xyz(site.material_center.x, site.material_center.y, Z_ITEM_PICKUP),
        ));
        commands.entity(item).remove::<crate::relationships::StoredIn>();
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
        commands.entity(item).insert((
            Visibility::Visible,
            Transform::from_xyz(site.material_center.x, site.material_center.y, Z_ITEM_PICKUP),
        ));
        commands.entity(item).remove::<crate::relationships::StoredIn>();
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
        ctx.queries.storage.buildings.get_mut(stockpile)
    {
        let can_deliver_to_wall = building.kind == BuildingType::Wall
            && building.is_provisional
            && provisional_opt
                .as_ref()
                .is_some_and(|provisional| !provisional.mud_delivered);
        if can_deliver_to_wall {
            let wall_pos = wall_transform.translation.truncate();
            commands.entity(item).insert((
                Visibility::Visible,
                Transform::from_xyz(wall_pos.x, wall_pos.y, Z_ITEM_PICKUP),
            ));
            commands.entity(item).remove::<crate::relationships::StoredIn>();
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
            drop_item(commands, ctx.soul_entity, item, soul_pos);
        }
    } else if ctx.inventory.0.is_some() {
        drop_item(commands, ctx.soul_entity, item, soul_pos);
    }

    ctx.inventory.0 = None;
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);
    ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
}
