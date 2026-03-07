//! Floor construction destination demand helpers.

use bevy::prelude::*;

use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::logistics::ResourceType;

use crate::constants::{FLOOR_BONES_PER_TILE, FLOOR_MUD_PER_TILE};

/// Floor construction site の特定リソースに対する基礎需要（incoming控除前）を返す。
///
/// 返り値は以下を差し引く前の値:
/// - IncomingDeliveries / ReservationShadow
/// - 近傍地面資材数 (`count_nearby_ground_resources`)
/// - リソース別の地面資材在庫補正
pub fn floor_site_tile_demand<'a>(
    floor_tiles: impl Iterator<Item = &'a FloorTileBlueprint>,
    site_entity: Entity,
    resource_type: ResourceType,
) -> usize {
    floor_tiles
        .filter(|tile| tile.parent_site == site_entity)
        .map(|tile| match resource_type {
            ResourceType::Bone if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingBones => {
                FLOOR_BONES_PER_TILE.saturating_sub(tile.bones_delivered) as usize
            }
            ResourceType::StasisMud
                if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingMud =>
            {
                FLOOR_MUD_PER_TILE.saturating_sub(tile.mud_delivered) as usize
            }
            _ => 0,
        })
        .sum()
}

pub fn floor_site_tile_demand_from_index(
    tile_entities: &[Entity],
    q_floor_tiles: &Query<&FloorTileBlueprint>,
    resource_type: ResourceType,
) -> usize {
    tile_entities
        .iter()
        .filter_map(|tile_entity| q_floor_tiles.get(*tile_entity).ok())
        .map(|tile| match resource_type {
            ResourceType::Bone if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingBones => {
                FLOOR_BONES_PER_TILE.saturating_sub(tile.bones_delivered) as usize
            }
            ResourceType::StasisMud if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingMud => {
                FLOOR_MUD_PER_TILE.saturating_sub(tile.mud_delivered) as usize
            }
            _ => 0,
        })
        .sum()
}
