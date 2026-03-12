use crate::world::map::{WorldMap, WorldMapRef};
use hw_ui::selection::{
    PlacementRejectReason, validate_floor_tile as shared_validate_floor_tile,
    validate_wall_tile as shared_validate_wall_tile,
};
use std::collections::HashSet;

/// Validate a single tile for floor placement. Returns `None` if valid, or a reject reason.
pub(crate) fn validate_floor_tile(
    gx: i32,
    gy: i32,
    world_map: &WorldMap,
    existing_floor_tile_grids: &HashSet<(i32, i32)>,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<PlacementRejectReason> {
    shared_validate_floor_tile(
        &WorldMapRef(world_map),
        (gx, gy),
        existing_floor_tile_grids,
        existing_floor_building_grids,
    )
}

/// Validate a single tile for wall placement. Returns `None` if valid, or a reject reason.
pub(crate) fn validate_wall_tile(
    gx: i32,
    gy: i32,
    world_map: &WorldMap,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<PlacementRejectReason> {
    shared_validate_wall_tile(
        &WorldMapRef(world_map),
        (gx, gy),
        existing_floor_building_grids,
    )
}
