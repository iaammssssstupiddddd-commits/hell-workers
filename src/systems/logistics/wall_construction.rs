//! Wall construction destination demand helpers.

use bevy::prelude::*;

use crate::systems::jobs::wall_construction::WallTileBlueprint;
use crate::systems::logistics::ResourceType;

use hw_core::constants::{WALL_MUD_PER_TILE, WALL_WOOD_PER_TILE};

/// Wall construction site の特定リソースに対する基礎需要（incoming控除前）を返す。
///
/// 返り値は以下を差し引く前の値:
/// - IncomingDeliveries / ReservationShadow
/// - 近傍地面資材数 (`count_nearby_ground_resources`)
/// - リソース別の地面資材在庫補正
pub fn wall_site_tile_demand<'a>(
    wall_tiles: impl Iterator<Item = &'a WallTileBlueprint>,
    site_entity: Entity,
    resource_type: ResourceType,
) -> usize {
    wall_tiles
        .filter(|tile| tile.parent_site == site_entity)
        .map(|tile| match resource_type {
            ResourceType::Wood
                if tile.state
                    == crate::systems::jobs::wall_construction::WallTileState::WaitingWood =>
            {
                WALL_WOOD_PER_TILE.saturating_sub(tile.wood_delivered) as usize
            }
            ResourceType::StasisMud
                if tile.state
                    == crate::systems::jobs::wall_construction::WallTileState::WaitingMud =>
            {
                WALL_MUD_PER_TILE.saturating_sub(tile.mud_delivered) as usize
            }
            _ => 0,
        })
        .sum()
}

pub fn wall_site_tile_demand_from_index(
    tile_entities: &[Entity],
    q_wall_tiles: &Query<&WallTileBlueprint>,
    resource_type: ResourceType,
) -> usize {
    tile_entities
        .iter()
        .filter_map(|tile_entity| q_wall_tiles.get(*tile_entity).ok())
        .map(|tile| match resource_type {
            ResourceType::Wood
                if tile.state
                    == crate::systems::jobs::wall_construction::WallTileState::WaitingWood =>
            {
                WALL_WOOD_PER_TILE.saturating_sub(tile.wood_delivered) as usize
            }
            ResourceType::StasisMud
                if tile.state
                    == crate::systems::jobs::wall_construction::WallTileState::WaitingMud =>
            {
                WALL_MUD_PER_TILE.saturating_sub(tile.mud_delivered) as usize
            }
            _ => 0,
        })
        .sum()
}
