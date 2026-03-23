use bevy::prelude::*;
use hw_core::constants::FLOOR_MAX_AREA_SIZE;
use hw_jobs::{BuildingCategory, BuildingType};
use std::collections::HashSet;

use super::geometry::grid_is_nearby;
use super::{
    BuildingPlacementContext, PlacementGeometry, PlacementRejectReason, PlacementValidation,
    WorldReadApi,
};

/// Validates floor/wall area size. Returns `AreaTooLarge` if either dimension exceeds the limit.
pub fn validate_area_size(width: i32, height: i32) -> Option<PlacementRejectReason> {
    if width > FLOOR_MAX_AREA_SIZE || height > FLOOR_MAX_AREA_SIZE {
        Some(PlacementRejectReason::AreaTooLarge)
    } else {
        None
    }
}

/// Validates that wall area forms a straight 1×n line.
/// Returns `AreaTooLarge` if too large, `NotStraightLine` if not a 1×n strip.
pub fn validate_wall_area(width: i32, height: i32) -> Option<PlacementRejectReason> {
    if let Some(reason) = validate_area_size(width, height) {
        return Some(reason);
    }
    if width < 1 || height < 1 || (width != 1 && height != 1) {
        return Some(PlacementRejectReason::NotStraightLine);
    }
    None
}

/// Validates whether a building can be placed at `dest_occupied` given its current
/// `old_occupied` footprint. Ignores self-occupancy.
pub fn can_place_moved_building<W>(
    world: &W,
    building_entity: Entity,
    old_occupied: &[(i32, i32)],
    dest_occupied: &[(i32, i32)],
) -> bool
where
    W: WorldReadApi,
{
    dest_occupied.iter().all(|&(gx, gy)| {
        if world.pos_to_idx(gx, gy).is_none() {
            return false;
        }
        let occupied_by_other = world
            .building_entity((gx, gy))
            .is_some_and(|e| e != building_entity);
        if occupied_by_other {
            return false;
        }
        if world.has_stockpile((gx, gy)) {
            return false;
        }
        world.is_walkable(gx, gy) || old_occupied.contains(&(gx, gy))
    })
}

fn reject_for_walkable_empty_tile<World>(
    world: &World,
    grid: (i32, i32),
) -> Option<PlacementRejectReason>
where
    World: WorldReadApi,
{
    if world.pos_to_idx(grid.0, grid.1).is_none() {
        return Some(PlacementRejectReason::OutOfBounds);
    }
    if world.has_building(grid) {
        return Some(PlacementRejectReason::OccupiedByBuilding);
    }
    if world.has_stockpile(grid) {
        return Some(PlacementRejectReason::OccupiedByStockpile);
    }
    if !world.is_walkable(grid.0, grid.1) {
        return Some(PlacementRejectReason::NotWalkable);
    }
    None
}

fn reject_for_bridge_tile<World>(world: &World, grid: (i32, i32)) -> Option<PlacementRejectReason>
where
    World: WorldReadApi,
{
    if world.pos_to_idx(grid.0, grid.1).is_none() {
        return Some(PlacementRejectReason::OutOfBounds);
    }
    if world.has_building(grid) {
        return Some(PlacementRejectReason::OccupiedByBuilding);
    }
    if world.has_stockpile(grid) {
        return Some(PlacementRejectReason::OccupiedByStockpile);
    }
    if !world.is_river_tile(grid.0, grid.1) {
        return Some(PlacementRejectReason::NotRiverTile);
    }
    None
}

pub fn validate_building_placement<World>(
    ctx: &BuildingPlacementContext<'_, World>,
    building_type: BuildingType,
    grid: (i32, i32),
    geometry: &PlacementGeometry,
) -> PlacementValidation
where
    World: WorldReadApi,
{
    let world = ctx.world;
    match building_type {
        BuildingType::Bridge => {
            for &candidate in &geometry.occupied_grids {
                if let Some(reason) = reject_for_bridge_tile(world, candidate) {
                    return PlacementValidation::rejected(reason);
                }
            }
        }
        BuildingType::Door => {
            let replaceable_wall = (ctx.is_replaceable_wall_at)(grid);
            if replaceable_wall {
                if world.has_stockpile(grid) {
                    return PlacementValidation::rejected(
                        PlacementRejectReason::OccupiedByStockpile,
                    );
                }
            } else if let Some(reason) = reject_for_walkable_empty_tile(world, grid) {
                return PlacementValidation::rejected(reason);
            }

            if !(ctx.is_wall_or_door_at)((grid.0 - 1, grid.1))
                || !(ctx.is_wall_or_door_at)((grid.0 + 1, grid.1))
            {
                if !(ctx.is_wall_or_door_at)((grid.0, grid.1 + 1))
                    || !(ctx.is_wall_or_door_at)((grid.0, grid.1 - 1))
                {
                    return PlacementValidation::rejected(
                        PlacementRejectReason::NoDoorAdjacentWall,
                    );
                }
            }
        }
        _ => {
            for &candidate in &geometry.occupied_grids {
                if let Some(reason) = reject_for_walkable_empty_tile(world, candidate) {
                    return PlacementValidation::rejected(reason);
                }
            }
        }
    }

    match building_type.category() {
        BuildingCategory::Structure if !ctx.in_site => {
            PlacementValidation::rejected(PlacementRejectReason::NotInSite)
        }
        BuildingCategory::Plant | BuildingCategory::Temporary if !ctx.in_yard => {
            PlacementValidation::rejected(PlacementRejectReason::NotInYard)
        }
        _ => PlacementValidation::ok(),
    }
}

pub fn validate_bucket_storage_placement<World>(
    world: &World,
    geometry: &PlacementGeometry,
    parent_occupied_grids: &[(i32, i32)],
    within_radius: bool,
    nearby_tiles: i32,
) -> PlacementValidation
where
    World: WorldReadApi,
{
    if !within_radius {
        return PlacementValidation::rejected(PlacementRejectReason::TooFarFromParent);
    }

    for &storage_grid in &geometry.occupied_grids {
        if !parent_occupied_grids
            .iter()
            .any(|&parent_grid| grid_is_nearby(parent_grid, storage_grid, nearby_tiles))
        {
            return PlacementValidation::rejected(PlacementRejectReason::TooFarFromParent);
        }

        if let Some(reason) = reject_for_walkable_empty_tile(world, storage_grid) {
            return PlacementValidation::rejected(reason);
        }
    }

    PlacementValidation::ok()
}

pub fn validate_moved_bucket_storage_placement<World>(
    world: &World,
    geometry: &PlacementGeometry,
    parent_occupied_grids: &[(i32, i32)],
    old_building_occupied: &[(i32, i32)],
    own_companion_grids: &[(i32, i32)],
    nearby_tiles: i32,
) -> PlacementValidation
where
    World: WorldReadApi,
{
    for &storage_grid in &geometry.occupied_grids {
        if !parent_occupied_grids
            .iter()
            .any(|&parent_grid| grid_is_nearby(parent_grid, storage_grid, nearby_tiles))
        {
            return PlacementValidation::rejected(PlacementRejectReason::TooFarFromParent);
        }

        if world.pos_to_idx(storage_grid.0, storage_grid.1).is_none() {
            return PlacementValidation::rejected(PlacementRejectReason::OutOfBounds);
        }
        if world.has_building(storage_grid) && !old_building_occupied.contains(&storage_grid) {
            return PlacementValidation::rejected(PlacementRejectReason::OccupiedByBuilding);
        }
        if world.has_stockpile(storage_grid) && !own_companion_grids.contains(&storage_grid) {
            return PlacementValidation::rejected(PlacementRejectReason::OccupiedByStockpile);
        }
        if !world.is_walkable(storage_grid.0, storage_grid.1)
            && !old_building_occupied.contains(&storage_grid)
            && !own_companion_grids.contains(&storage_grid)
        {
            return PlacementValidation::rejected(PlacementRejectReason::NotWalkable);
        }
    }

    PlacementValidation::ok()
}

pub fn validate_floor_tile<World>(
    world: &World,
    grid: (i32, i32),
    existing_floor_tile_grids: &HashSet<(i32, i32)>,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<PlacementRejectReason>
where
    World: WorldReadApi,
{
    if !world.is_walkable(grid.0, grid.1) {
        return Some(PlacementRejectReason::NotWalkable);
    }
    if world.has_building(grid) {
        return Some(PlacementRejectReason::OccupiedByBuilding);
    }
    if world.has_stockpile(grid) {
        return Some(PlacementRejectReason::OccupiedByStockpile);
    }
    if existing_floor_tile_grids.contains(&grid) {
        return Some(PlacementRejectReason::AlreadyHasFloorBlueprint);
    }
    if existing_floor_building_grids.contains(&grid) {
        return Some(PlacementRejectReason::AlreadyHasCompletedFloor);
    }
    None
}

pub fn validate_wall_tile<World>(
    world: &World,
    grid: (i32, i32),
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<PlacementRejectReason>
where
    World: WorldReadApi,
{
    if !world.is_walkable(grid.0, grid.1) {
        return Some(PlacementRejectReason::NotWalkable);
    }
    if world.has_building(grid) {
        return Some(PlacementRejectReason::OccupiedByBuilding);
    }
    if world.has_stockpile(grid) {
        return Some(PlacementRejectReason::OccupiedByStockpile);
    }
    if !existing_floor_building_grids.contains(&grid) {
        return Some(PlacementRejectReason::NoCompletedFloor);
    }
    None
}
