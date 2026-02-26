use crate::world::map::WorldMap;
use std::collections::HashSet;

pub(super) enum TileRejectReason {
    NotWalkable,
    OccupiedByBuilding,
    OccupiedByStockpile,
    AlreadyHasFloorBlueprint,
    AlreadyHasCompletedFloor,
    NoCompletedFloor,
}

impl TileRejectReason {
    pub(super) fn message(&self, gx: i32, gy: i32) -> String {
        match self {
            TileRejectReason::NotWalkable => format!("Tile ({},{}) is not walkable", gx, gy),
            TileRejectReason::OccupiedByBuilding => {
                format!("Tile ({},{}) is already occupied by a building", gx, gy)
            }
            TileRejectReason::OccupiedByStockpile => {
                format!("Tile ({},{}) is already occupied by a stockpile", gx, gy)
            }
            TileRejectReason::AlreadyHasFloorBlueprint => {
                format!("Tile ({},{}) already has a floor blueprint", gx, gy)
            }
            TileRejectReason::AlreadyHasCompletedFloor => {
                format!("Tile ({},{}) already has a completed floor", gx, gy)
            }
            TileRejectReason::NoCompletedFloor => {
                format!("Tile ({},{}) has no completed floor", gx, gy)
            }
        }
    }
}

/// Validate a single tile for floor placement. Returns `None` if valid, or a reject reason.
pub(super) fn validate_floor_tile(
    gx: i32,
    gy: i32,
    world_map: &WorldMap,
    existing_floor_tile_grids: &HashSet<(i32, i32)>,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<TileRejectReason> {
    if !world_map.is_walkable(gx, gy) {
        return Some(TileRejectReason::NotWalkable);
    }
    if world_map.buildings.contains_key(&(gx, gy)) {
        return Some(TileRejectReason::OccupiedByBuilding);
    }
    if world_map.stockpiles.contains_key(&(gx, gy)) {
        return Some(TileRejectReason::OccupiedByStockpile);
    }
    if existing_floor_tile_grids.contains(&(gx, gy)) {
        return Some(TileRejectReason::AlreadyHasFloorBlueprint);
    }
    if existing_floor_building_grids.contains(&(gx, gy)) {
        return Some(TileRejectReason::AlreadyHasCompletedFloor);
    }
    None
}

/// Validate a single tile for wall placement. Returns `None` if valid, or a reject reason.
pub(super) fn validate_wall_tile(
    gx: i32,
    gy: i32,
    world_map: &WorldMap,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<TileRejectReason> {
    if !world_map.is_walkable(gx, gy) {
        return Some(TileRejectReason::NotWalkable);
    }
    if world_map.buildings.contains_key(&(gx, gy)) {
        return Some(TileRejectReason::OccupiedByBuilding);
    }
    if world_map.stockpiles.contains_key(&(gx, gy)) {
        return Some(TileRejectReason::OccupiedByStockpile);
    }
    if !existing_floor_building_grids.contains(&(gx, gy)) {
        return Some(TileRejectReason::NoCompletedFloor);
    }
    None
}
