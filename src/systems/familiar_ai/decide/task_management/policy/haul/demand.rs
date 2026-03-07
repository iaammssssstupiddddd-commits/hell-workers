//! 残需要計算: 「必要量 - 流入予約」の共通 API

use bevy::prelude::*;

use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::{
    floor_site_tile_demand, provisional_wall_mud_demand, wall_site_tile_demand,
};

type TaskAssignmentQueries<'w, 's> =
    crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries<'w, 's>;

pub fn compute_remaining_blueprint_amount(
    blueprint: Entity,
    resource_type: ResourceType,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    let Ok((_, blueprint_comp, _)) = queries.storage.blueprints.get(blueprint) else {
        return 0;
    };

    if let Some(flexible) = &blueprint_comp.flexible_material_requirement
        && flexible.accepts(resource_type)
    {
        let incoming = count_matching_incoming_deliveries(
            blueprint,
            queries,
            |incoming_resource_type| flexible.accepted_types.contains(&incoming_resource_type),
        ) + flexible
            .accepted_types
            .iter()
            .map(|accepted_type| shadow.destination_reserved_resource(blueprint, *accepted_type) as u32)
            .sum::<u32>();

        return flexible.remaining().saturating_sub(incoming);
    }

    let needed_material = blueprint_comp.remaining_material_amount(resource_type);
    if needed_material == 0 {
        return 0;
    }

    let incoming = count_exact_incoming_deliveries(blueprint, resource_type, queries)
        + shadow.destination_reserved_resource(blueprint, resource_type) as u32;
    needed_material.saturating_sub(incoming)
}

/// Blueprint への猫車向け資材の残需要（必要量 - 搬入予約）
pub fn compute_remaining_blueprint_wheelbarrow_amount(
    blueprint: Entity,
    resource_type: ResourceType,
    _task_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    compute_remaining_blueprint_amount(blueprint, resource_type, queries, shadow)
}

/// 床建設サイトへの骨の残需要
pub fn compute_remaining_floor_bones(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    let base_demand =
        floor_site_tile_demand(queries.storage.floor_tiles.iter(), site_entity, ResourceType::Bone);
    compute_remaining_from_incoming(site_entity, base_demand, ResourceType::Bone, queries, shadow)
}

/// 床建設サイトへの泥の残需要
pub fn compute_remaining_floor_mud(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    let base_demand =
        floor_site_tile_demand(queries.storage.floor_tiles.iter(), site_entity, ResourceType::StasisMud);
    compute_remaining_from_incoming(site_entity, base_demand, ResourceType::StasisMud, queries, shadow)
}

/// 壁建設サイトへの木材の残需要
pub fn compute_remaining_wall_wood(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    let base_demand =
        wall_site_tile_demand(queries.storage.wall_tiles.iter(), site_entity, ResourceType::Wood);
    compute_remaining_from_incoming(site_entity, base_demand, ResourceType::Wood, queries, shadow)
}

/// 壁建設サイトへの泥の残需要
pub fn compute_remaining_wall_mud(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    let base_demand =
        wall_site_tile_demand(queries.storage.wall_tiles.iter(), site_entity, ResourceType::StasisMud);
    compute_remaining_from_incoming(site_entity, base_demand, ResourceType::StasisMud, queries, shadow)
}

pub fn compute_remaining_stockpile_capacity(
    stockpile_entity: Entity,
    resource_type: ResourceType,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    let Ok((_, _, stockpile, stored_items_opt)) = queries.storage.stockpiles.get(stockpile_entity) else {
        return 0;
    };
    if stockpile.resource_type.is_some() && stockpile.resource_type != Some(resource_type) {
        return 0;
    }

    let stored = stored_items_opt.map(|items| items.len()).unwrap_or(0);
    let incoming = queries
        .reservation
        .incoming_deliveries_query
        .get(stockpile_entity)
        .map(|incoming| incoming.len())
        .unwrap_or(0);
    let shadow_incoming = shadow.destination_reserved_total(stockpile_entity);
    stockpile
        .capacity
        .saturating_sub(stored + incoming + shadow_incoming) as u32
}

pub fn compute_remaining_provisional_wall_mud(
    wall_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    let Ok((_, building, provisional_opt)) = queries.storage.buildings.get(wall_entity) else {
        return 0;
    };
    let base_demand = provisional_wall_mud_demand(&building, provisional_opt.as_deref()) as u32;
    compute_remaining_from_incoming(wall_entity, base_demand as usize, ResourceType::StasisMud, queries, shadow)
}

fn compute_remaining_from_incoming(
    anchor_entity: Entity,
    base_demand: usize,
    resource_type: ResourceType,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    let incoming = count_exact_incoming_deliveries(anchor_entity, resource_type, queries)
        + shadow.destination_reserved_resource(anchor_entity, resource_type) as u32;

    base_demand.saturating_sub(incoming as usize) as u32
}

fn count_exact_incoming_deliveries(
    target: Entity,
    resource_type: ResourceType,
    queries: &TaskAssignmentQueries<'_, '_>,
) -> u32 {
    count_matching_incoming_deliveries(target, queries, |incoming_resource_type| {
        incoming_resource_type == resource_type
    })
}

fn count_matching_incoming_deliveries(
    target: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    mut predicate: impl FnMut(ResourceType) -> bool,
) -> u32 {
    queries
        .reservation
        .incoming_deliveries_query
        .get(target)
        .map(|incoming| {
            incoming
                .iter()
                .filter(|&&item| {
                    queries
                        .reservation
                        .resources
                        .get(item)
                        .is_ok_and(|resource_item| predicate(resource_item.0))
                })
                .count() as u32
        })
        .unwrap_or(0)
}
