//! 残需要計算: 「必要量 - 流入予約」の共通 API

use bevy::prelude::*;

use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use crate::systems::logistics::ResourceType;

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
    compute_remaining_with_incoming(
        site_entity,
        ResourceType::Bone,
        queries,
        shadow,
        |tile| {
            if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingBones
            {
                crate::constants::FLOOR_BONES_PER_TILE.saturating_sub(tile.bones_delivered)
            } else {
                0
            }
        },
    )
}

/// 床建設サイトへの泥の残需要
pub fn compute_remaining_floor_mud(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    compute_remaining_with_incoming(
        site_entity,
        ResourceType::StasisMud,
        queries,
        shadow,
        |tile| {
            if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingMud {
                crate::constants::FLOOR_MUD_PER_TILE.saturating_sub(tile.mud_delivered)
            } else {
                0
            }
        },
    )
}

/// 壁建設サイトへの木材の残需要
pub fn compute_remaining_wall_wood(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    compute_remaining_wall_with_incoming(site_entity, ResourceType::Wood, queries, shadow, |tile| {
        if tile.state == crate::systems::jobs::wall_construction::WallTileState::WaitingWood {
            crate::constants::WALL_WOOD_PER_TILE.saturating_sub(tile.wood_delivered)
        } else {
            0
        }
    })
}

/// 壁建設サイトへの泥の残需要
pub fn compute_remaining_wall_mud(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> u32 {
    compute_remaining_wall_with_incoming(
        site_entity,
        ResourceType::StasisMud,
        queries,
        shadow,
        |tile| {
            if tile.state == crate::systems::jobs::wall_construction::WallTileState::WaitingMud {
                crate::constants::WALL_MUD_PER_TILE.saturating_sub(tile.mud_delivered)
            } else {
                0
            }
        },
    )
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
    if building.kind != crate::systems::jobs::BuildingType::Wall
        || !building.is_provisional
        || provisional_opt.is_none_or(|provisional| provisional.mud_delivered)
    {
        return 0;
    }

    let incoming = count_exact_incoming_deliveries(wall_entity, ResourceType::StasisMud, queries)
        + shadow.destination_reserved_resource(wall_entity, ResourceType::StasisMud) as u32;
    1u32.saturating_sub(incoming)
}

fn compute_remaining_with_incoming(
    anchor_entity: Entity,
    resource_type: ResourceType,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
    needed_per_tile: impl Fn(&crate::systems::jobs::floor_construction::FloorTileBlueprint) -> u32,
) -> u32 {
    let mut needed = 0u32;

    for tile in queries
        .storage
        .floor_tiles
        .iter()
        .filter(|tile| tile.parent_site == anchor_entity)
    {
        needed += needed_per_tile(tile);
    }

    let incoming = count_exact_incoming_deliveries(anchor_entity, resource_type, queries)
        + shadow.destination_reserved_resource(anchor_entity, resource_type) as u32;

    needed.saturating_sub(incoming)
}

fn compute_remaining_wall_with_incoming(
    anchor_entity: Entity,
    resource_type: ResourceType,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
    needed_per_tile: impl Fn(&crate::systems::jobs::wall_construction::WallTileBlueprint) -> u32,
) -> u32 {
    let mut needed = 0u32;

    for tile in queries
        .storage
        .wall_tiles
        .iter()
        .filter(|tile| tile.parent_site == anchor_entity)
    {
        needed += needed_per_tile(tile);
    }

    let incoming = count_exact_incoming_deliveries(anchor_entity, resource_type, queries)
        + shadow.destination_reserved_resource(anchor_entity, resource_type) as u32;

    needed.saturating_sub(incoming)
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
