use crate::world::map::WorldMap;
use bevy::prelude::Entity;
use hw_ui::selection::{
    PlacementRejectReason, WorldReadApi, validate_floor_tile as shared_validate_floor_tile,
    validate_wall_tile as shared_validate_wall_tile,
};
use std::collections::HashSet;

struct FloorPlacementWorld<'a>(&'a WorldMap);

impl WorldReadApi for FloorPlacementWorld<'_> {
    fn has_building(&self, grid: (i32, i32)) -> bool {
        self.0.has_building(grid)
    }

    fn has_stockpile(&self, grid: (i32, i32)) -> bool {
        self.0.has_stockpile(grid)
    }

    fn is_walkable(&self, gx: i32, gy: i32) -> bool {
        self.0.is_walkable(gx, gy)
    }

    fn is_river_tile(&self, gx: i32, gy: i32) -> bool {
        self.0.is_river_tile(gx, gy)
    }

    fn building_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.0.building_entity(grid)
    }

    fn stockpile_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.0.stockpile_entity(grid)
    }

    fn pos_to_idx(&self, gx: i32, gy: i32) -> Option<usize> {
        self.0.pos_to_idx(gx, gy)
    }
}

/// Validate a single tile for floor placement. Returns `None` if valid, or a reject reason.
pub(crate) fn validate_floor_tile(
    gx: i32,
    gy: i32,
    world_map: &WorldMap,
    existing_floor_tile_grids: &HashSet<(i32, i32)>,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<PlacementRejectReason> {
    shared_validate_floor_tile(
        &FloorPlacementWorld(world_map),
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
        &FloorPlacementWorld(world_map),
        (gx, gy),
        existing_floor_building_grids,
    )
}
