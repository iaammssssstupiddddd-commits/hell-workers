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
pub fn validate_moved_building_placement<W>(
    world: &W,
    building_entity: Entity,
    old_occupied: &[(i32, i32)],
    dest_occupied: &[(i32, i32)],
) -> PlacementValidation
where
    W: WorldReadApi,
{
    for &(gx, gy) in dest_occupied {
        if world.pos_to_idx(gx, gy).is_none() {
            return PlacementValidation::rejected_at(PlacementRejectReason::OutOfBounds, (gx, gy));
        }
        let occupied_by_other = world
            .building_entity((gx, gy))
            .is_some_and(|e| e != building_entity);
        if occupied_by_other {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::OccupiedByBuilding,
                (gx, gy),
            );
        }
        if world.has_stockpile((gx, gy)) {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::OccupiedByStockpile,
                (gx, gy),
            );
        }
        if !world.is_walkable(gx, gy) && !old_occupied.contains(&(gx, gy)) {
            return PlacementValidation::rejected_at(PlacementRejectReason::NotWalkable, (gx, gy));
        }
    }
    PlacementValidation::ok()
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

/// One live-terrain crossing, using the existing 2x5 RiverYMin shape.
/// No terrain, walkability, or owner state is changed by this resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeCrossing {
    pub anchor: (i32, i32),
    pub occupied_grids: Vec<(i32, i32)>,
    pub banks: [(i32, i32); 4],
}

pub fn resolve_bridge_crossing<W: WorldReadApi>(
    world: &W,
    clicked_grid: (i32, i32),
) -> Result<BridgeCrossing, super::PlacementTileRejection> {
    use super::{PlacementRejectReason as Reason, PlacementTileRejection};
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
    let reject = |reason, grid| PlacementTileRejection { grid, reason };
    let x = clicked_grid.0;
    if !(0..MAP_WIDTH - 1).contains(&x) {
        return Err(reject(Reason::OutOfBounds, clicked_grid));
    }
    let mut intervals = [(0, 0); 2];
    for (column, interval) in intervals.iter_mut().enumerate() {
        let gx = x + column as i32;
        let mut rows = (0..MAP_HEIGHT).filter(|&y| world.is_river_tile(gx, y));
        let Some(lo) = rows.next() else {
            return Err(reject(Reason::NotRiverTile, (gx, clicked_grid.1)));
        };
        let hi = rows.next_back().unwrap_or(lo);
        for y in lo..=hi {
            if !world.is_river_tile(gx, y) {
                return Err(reject(Reason::NotRiverTile, (gx, y)));
            }
        }
        *interval = (lo, hi);
    }
    let lo = intervals[0].0.min(intervals[1].0);
    let hi = intervals[0].1.max(intervals[1].1);
    let span = hi - lo + 1;
    if span > 5 {
        return Err(reject(Reason::NotWalkable, (x, lo)));
    }
    let anchor = (x, lo - (5 - span) / 2);
    let occupied_grids: Vec<_> = hw_jobs::building_shape(BuildingType::Bridge)
        .ordered_relative_tiles
        .iter()
        .map(|&(dx, dy)| (x + dx, anchor.1 + dy))
        .collect();
    let banks = [
        (x, anchor.1 - 1),
        (x + 1, anchor.1 - 1),
        (x, anchor.1 + 5),
        (x + 1, anchor.1 + 5),
    ];
    for &grid in occupied_grids.iter().chain(&banks) {
        let reason = if world.pos_to_idx(grid.0, grid.1).is_none() {
            Some(Reason::OutOfBounds)
        } else if world.has_building(grid) {
            Some(Reason::OccupiedByBuilding)
        } else if world.has_stockpile(grid) {
            Some(Reason::OccupiedByStockpile)
        } else if world.has_raw_obstacle(grid)
            || (banks.contains(&grid) && world.is_river_tile(grid.0, grid.1))
            || (!world.is_river_tile(grid.0, grid.1) && !world.is_walkable(grid.0, grid.1))
        {
            Some(Reason::NotWalkable)
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(reject(reason, grid));
        }
    }
    Ok(BridgeCrossing {
        anchor,
        occupied_grids,
        banks,
    })
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
        BuildingType::Bridge => match resolve_bridge_crossing(world, grid) {
            Ok(crossing) if crossing.occupied_grids == geometry.occupied_grids => {}
            Ok(_) => {
                return PlacementValidation::rejected_at(PlacementRejectReason::NotWalkable, grid);
            }
            Err(rejection) => {
                return PlacementValidation::rejected_at(rejection.reason, rejection.grid);
            }
        },
        BuildingType::Door => {
            let replaceable_wall = (ctx.is_replaceable_wall_at)(grid);
            if replaceable_wall {
                if world.has_stockpile(grid) {
                    return PlacementValidation::rejected_at(
                        PlacementRejectReason::OccupiedByStockpile,
                        grid,
                    );
                }
            } else if let Some(reason) = reject_for_walkable_empty_tile(world, grid) {
                return PlacementValidation::rejected_at(reason, grid);
            }

            if (!(ctx.is_wall_or_door_at)((grid.0 - 1, grid.1))
                || !(ctx.is_wall_or_door_at)((grid.0 + 1, grid.1)))
                && (!(ctx.is_wall_or_door_at)((grid.0, grid.1 + 1))
                    || !(ctx.is_wall_or_door_at)((grid.0, grid.1 - 1)))
            {
                return PlacementValidation::rejected_at(
                    PlacementRejectReason::NoDoorAdjacentWall,
                    grid,
                );
            }
        }
        _ => {
            for &candidate in &geometry.occupied_grids {
                if let Some(reason) = reject_for_walkable_empty_tile(world, candidate) {
                    return PlacementValidation::rejected_at(reason, candidate);
                }
            }
        }
    }

    match building_type.category() {
        BuildingCategory::Structure if !ctx.in_site => {
            PlacementValidation::rejected_at(PlacementRejectReason::NotInSite, grid)
        }
        BuildingCategory::Plant | BuildingCategory::Temporary if !ctx.in_yard => {
            PlacementValidation::rejected_at(PlacementRejectReason::NotInYard, grid)
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
        return PlacementValidation::rejected_at(
            PlacementRejectReason::TooFarFromParent,
            geometry.occupied_grids.first().copied().unwrap_or_default(),
        );
    }

    for &storage_grid in &geometry.occupied_grids {
        if parent_occupied_grids.contains(&storage_grid) {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::OccupiedByBuilding,
                storage_grid,
            );
        }
        if !parent_occupied_grids
            .iter()
            .any(|&parent_grid| grid_is_nearby(parent_grid, storage_grid, nearby_tiles))
        {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::TooFarFromParent,
                storage_grid,
            );
        }

        if let Some(reason) = reject_for_walkable_empty_tile(world, storage_grid) {
            return PlacementValidation::rejected_at(reason, storage_grid);
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
        if parent_occupied_grids.contains(&storage_grid) {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::OccupiedByBuilding,
                storage_grid,
            );
        }
        if !parent_occupied_grids
            .iter()
            .any(|&parent_grid| grid_is_nearby(parent_grid, storage_grid, nearby_tiles))
        {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::TooFarFromParent,
                storage_grid,
            );
        }

        if world.pos_to_idx(storage_grid.0, storage_grid.1).is_none() {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::OutOfBounds,
                storage_grid,
            );
        }
        if world.has_building(storage_grid) && !old_building_occupied.contains(&storage_grid) {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::OccupiedByBuilding,
                storage_grid,
            );
        }
        if world.has_stockpile(storage_grid) && !own_companion_grids.contains(&storage_grid) {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::OccupiedByStockpile,
                storage_grid,
            );
        }
        if !world.is_walkable(storage_grid.0, storage_grid.1)
            && !old_building_occupied.contains(&storage_grid)
            && !own_companion_grids.contains(&storage_grid)
        {
            return PlacementValidation::rejected_at(
                PlacementRejectReason::NotWalkable,
                storage_grid,
            );
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
    if world.pos_to_idx(grid.0, grid.1).is_none() {
        return Some(PlacementRejectReason::OutOfBounds);
    }
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
    if world.pos_to_idx(grid.0, grid.1).is_none() {
        return Some(PlacementRejectReason::OutOfBounds);
    }
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
