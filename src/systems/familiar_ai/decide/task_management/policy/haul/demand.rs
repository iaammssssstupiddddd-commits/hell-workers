//! 残需要計算: 「必要量 - 流入予約」の共通 API

use bevy::prelude::*;

use crate::systems::logistics::ResourceType;

type TaskAssignmentQueries<'w, 's> =
    crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>;

/// Blueprint への猫車向け資材の残需要（必要量 - 搬入予約）
pub fn compute_remaining_blueprint_wheelbarrow_amount(
    blueprint: Entity,
    resource_type: ResourceType,
    _task_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
) -> u32 {
    let Ok((_, blueprint_comp, _)) = queries.storage.blueprints.get(blueprint) else {
        return 0;
    };

    let required = *blueprint_comp
        .required_materials
        .get(&resource_type)
        .unwrap_or(&0);
    let delivered = *blueprint_comp
        .delivered_materials
        .get(&resource_type)
        .unwrap_or(&0);
    let needed_material = required.saturating_sub(delivered);
    if needed_material == 0 {
        return 0;
    }

    let reserved_total = queries
        .reservation
        .incoming_deliveries_query
        .get(blueprint)
        .map(|inc| inc.len())
        .unwrap_or(0);

    needed_material.saturating_sub(reserved_total as u32)
}

/// 床建設サイトへの骨の残需要
pub fn compute_remaining_floor_bones(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
) -> u32 {
    compute_remaining_with_incoming(site_entity, queries, |tile| {
        if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingBones {
            crate::constants::FLOOR_BONES_PER_TILE.saturating_sub(tile.bones_delivered)
        } else {
            0
        }
    })
}

/// 床建設サイトへの泥の残需要
pub fn compute_remaining_floor_mud(
    site_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
) -> u32 {
    compute_remaining_with_incoming(site_entity, queries, |tile| {
        if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingMud {
            crate::constants::FLOOR_MUD_PER_TILE.saturating_sub(tile.mud_delivered)
        } else {
            0
        }
    })
}

fn compute_remaining_with_incoming(
    anchor_entity: Entity,
    queries: &TaskAssignmentQueries<'_, '_>,
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

    let incoming = queries
        .reservation
        .incoming_deliveries_query
        .get(anchor_entity)
        .map(|inc| inc.len() as u32)
        .unwrap_or(0);

    needed.saturating_sub(incoming)
}
