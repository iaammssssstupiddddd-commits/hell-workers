use bevy::prelude::*;
use hw_core::constants::{FLOOR_MAX_AREA_SIZE, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};
use hw_jobs::{BuildingCategory, BuildingType};
use std::collections::HashSet;

pub const TANK_NEARBY_BUCKET_STORAGE_TILES: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementRejectReason {
    NotWalkable,
    OccupiedByBuilding,
    OccupiedByStockpile,
    OutOfBounds,
    NotRiverTile,
    NoDoorAdjacentWall,
    NotInSite,
    NotInYard,
    AlreadyHasFloorBlueprint,
    AlreadyHasCompletedFloor,
    NoCompletedFloor,
    AreaTooLarge,
    TooFarFromParent,
    NotStraightLine,
}

impl PlacementRejectReason {
    pub fn message(&self, gx: i32, gy: i32) -> String {
        match self {
            Self::NotWalkable => format!("Tile ({},{}) is not walkable", gx, gy),
            Self::OccupiedByBuilding => {
                format!("Tile ({},{}) is already occupied by a building", gx, gy)
            }
            Self::OccupiedByStockpile => {
                format!("Tile ({},{}) is already occupied by a stockpile", gx, gy)
            }
            Self::OutOfBounds => format!("Tile ({},{}) is out of bounds", gx, gy),
            Self::NotRiverTile => format!("Tile ({},{}) is not a river tile", gx, gy),
            Self::NoDoorAdjacentWall => {
                format!("Tile ({},{}) has no adjacent wall pair for door", gx, gy)
            }
            Self::NotInSite => {
                format!("Tile ({},{}) is not inside a construction site", gx, gy)
            }
            Self::NotInYard => format!("Tile ({},{}) is not inside a yard", gx, gy),
            Self::AlreadyHasFloorBlueprint => {
                format!("Tile ({},{}) already has a floor blueprint", gx, gy)
            }
            Self::AlreadyHasCompletedFloor => {
                format!("Tile ({},{}) already has a completed floor", gx, gy)
            }
            Self::NoCompletedFloor => {
                format!("Tile ({},{}) has no completed floor", gx, gy)
            }
            Self::AreaTooLarge => {
                format!("Placement area starting at ({},{}) is too large", gx, gy)
            }
            Self::TooFarFromParent => {
                format!("Tile ({},{}) is too far from parent building", gx, gy)
            }
            Self::NotStraightLine => {
                format!(
                    "Wall must be placed as a straight 1xn line (tile {},{} is in a non-linear area)",
                    gx, gy
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlacementValidation {
    pub can_place: bool,
    pub reject_reason: Option<PlacementRejectReason>,
}

impl PlacementValidation {
    pub fn ok() -> Self {
        Self {
            can_place: true,
            reject_reason: None,
        }
    }

    pub fn rejected(reason: PlacementRejectReason) -> Self {
        Self {
            can_place: false,
            reject_reason: Some(reason),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlacementGeometry {
    pub occupied_grids: Vec<(i32, i32)>,
    pub draw_pos: Vec2,
    pub size: Vec2,
}

pub trait WorldReadApi {
    fn has_building(&self, grid: (i32, i32)) -> bool;
    fn has_stockpile(&self, grid: (i32, i32)) -> bool;
    fn is_walkable(&self, gx: i32, gy: i32) -> bool;
    fn is_river_tile(&self, gx: i32, gy: i32) -> bool;
    fn building_entity(&self, grid: (i32, i32)) -> Option<Entity>;
    fn stockpile_entity(&self, grid: (i32, i32)) -> Option<Entity>;
    fn pos_to_idx(&self, gx: i32, gy: i32) -> Option<usize>;
}

pub struct BuildingPlacementContext<'a, World>
where
    World: WorldReadApi,
{
    pub world: &'a World,
    pub in_site: bool,
    pub in_yard: bool,
    pub is_wall_or_door_at: &'a dyn Fn((i32, i32)) -> bool,
    pub is_replaceable_wall_at: &'a dyn Fn((i32, i32)) -> bool,
}

fn grid_to_world(x: i32, y: i32) -> Vec2 {
    Vec2::new(
        (x as f32 - (MAP_WIDTH as f32 - 1.0) / 2.0) * TILE_SIZE,
        (y as f32 - (MAP_HEIGHT as f32 - 1.0) / 2.0) * TILE_SIZE,
    )
}

fn world_to_grid(pos: Vec2) -> (i32, i32) {
    let x = (pos.x / TILE_SIZE + (MAP_WIDTH as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
    let y = (pos.y / TILE_SIZE + (MAP_HEIGHT as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
    (x, y)
}

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

/// Returns the anchor grid for a building move operation.
/// For 2×2 buildings the cursor is treated as the building center, so the anchor is
/// shifted by half a tile to the bottom-left.
pub fn move_anchor_grid(kind: BuildingType, world_pos: Vec2) -> (i32, i32) {
    match kind {
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => {
            world_to_grid(world_pos - Vec2::splat(TILE_SIZE * 0.5))
        }
        _ => world_to_grid(world_pos),
    }
}

/// Returns the occupied grid tiles for a moved building given its anchor.
/// Equivalent to `building_occupied_grids` but without the bridge special-case.
pub fn move_occupied_grids(kind: BuildingType, anchor: (i32, i32)) -> Vec<(i32, i32)> {
    match kind {
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => vec![
            anchor,
            (anchor.0 + 1, anchor.1),
            (anchor.0, anchor.1 + 1),
            (anchor.0 + 1, anchor.1 + 1),
        ],
        _ => vec![anchor],
    }
}

/// Returns the draw/spawn position for a moved building given its anchor grid.
pub fn move_spawn_pos(kind: BuildingType, anchor: (i32, i32)) -> Vec2 {
    let base = grid_to_world(anchor.0, anchor.1);
    match kind {
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => base + Vec2::splat(TILE_SIZE * 0.5),
        _ => base,
    }
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

pub fn building_geometry(
    building_type: BuildingType,
    grid: (i32, i32),
    river_y_min: i32,
) -> PlacementGeometry {
    let occupied_grids = building_occupied_grids(building_type, grid, river_y_min);
    let draw_pos = building_spawn_pos(building_type, grid, river_y_min);
    let size = building_size(building_type);
    PlacementGeometry {
        occupied_grids,
        draw_pos,
        size,
    }
}

pub fn bucket_storage_geometry(anchor_grid: (i32, i32)) -> PlacementGeometry {
    PlacementGeometry {
        occupied_grids: vec![anchor_grid, (anchor_grid.0 + 1, anchor_grid.1)],
        draw_pos: grid_to_world(anchor_grid.0, anchor_grid.1) + Vec2::new(TILE_SIZE * 0.5, 0.0),
        size: Vec2::new(TILE_SIZE * 2.0, TILE_SIZE),
    }
}

pub fn building_occupied_grids(
    building_type: BuildingType,
    grid: (i32, i32),
    river_y_min: i32,
) -> Vec<(i32, i32)> {
    match building_type {
        BuildingType::Bridge => (0..5)
            .flat_map(|dy| [(grid.0, river_y_min + dy), (grid.0 + 1, river_y_min + dy)])
            .collect(),
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => vec![
            grid,
            (grid.0 + 1, grid.1),
            (grid.0, grid.1 + 1),
            (grid.0 + 1, grid.1 + 1),
        ],
        _ => vec![grid],
    }
}

pub fn building_spawn_pos(building_type: BuildingType, grid: (i32, i32), river_y_min: i32) -> Vec2 {
    let base_pos = grid_to_world(grid.0, grid.1);
    match building_type {
        BuildingType::Bridge => {
            let base = grid_to_world(grid.0, river_y_min);
            base + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 2.0)
        }
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => {
            base_pos + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 0.5)
        }
        _ => base_pos,
    }
}

pub fn building_size(building_type: BuildingType) -> Vec2 {
    match building_type {
        BuildingType::Bridge => Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 5.0),
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => Vec2::splat(TILE_SIZE * 2.0),
        _ => Vec2::splat(TILE_SIZE),
    }
}

pub fn grid_is_nearby(base: (i32, i32), target: (i32, i32), tiles: i32) -> bool {
    (target.0 - base.0).abs() <= tiles && (target.1 - base.1).abs() <= tiles
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestWorld {
        buildings: HashSet<(i32, i32)>,
        stockpiles: HashSet<(i32, i32)>,
        walkable: HashSet<(i32, i32)>,
        river: HashSet<(i32, i32)>,
        bounds: HashSet<(i32, i32)>,
    }

    impl WorldReadApi for TestWorld {
        fn has_building(&self, grid: (i32, i32)) -> bool {
            self.buildings.contains(&grid)
        }

        fn has_stockpile(&self, grid: (i32, i32)) -> bool {
            self.stockpiles.contains(&grid)
        }

        fn is_walkable(&self, gx: i32, gy: i32) -> bool {
            self.walkable.contains(&(gx, gy))
        }

        fn is_river_tile(&self, gx: i32, gy: i32) -> bool {
            self.river.contains(&(gx, gy))
        }

        fn building_entity(&self, _grid: (i32, i32)) -> Option<Entity> {
            None
        }

        fn stockpile_entity(&self, _grid: (i32, i32)) -> Option<Entity> {
            None
        }

        fn pos_to_idx(&self, gx: i32, gy: i32) -> Option<usize> {
            self.bounds.contains(&(gx, gy)).then_some(0)
        }
    }

    #[test]
    fn door_requires_adjacent_wall_pair() {
        let mut world = TestWorld::default();
        world.bounds.insert((0, 0));
        world.walkable.insert((0, 0));

        let geometry = building_geometry(BuildingType::Door, (0, 0), 0);
        let ctx = BuildingPlacementContext {
            world: &world,
            in_site: true,
            in_yard: true,
            is_wall_or_door_at: &|_| false,
            is_replaceable_wall_at: &|_| false,
        };

        let validation = validate_building_placement(&ctx, BuildingType::Door, (0, 0), &geometry);
        assert_eq!(
            validation.reject_reason,
            Some(PlacementRejectReason::NoDoorAdjacentWall)
        );
    }

    #[test]
    fn structure_requires_site() {
        let mut world = TestWorld::default();
        world.bounds.insert((0, 0));
        world.walkable.insert((0, 0));

        let geometry = building_geometry(BuildingType::Wall, (0, 0), 0);
        let ctx = BuildingPlacementContext {
            world: &world,
            in_site: false,
            in_yard: true,
            is_wall_or_door_at: &|_| false,
            is_replaceable_wall_at: &|_| false,
        };

        let validation = validate_building_placement(&ctx, BuildingType::Wall, (0, 0), &geometry);
        assert_eq!(
            validation.reject_reason,
            Some(PlacementRejectReason::NotInSite)
        );
    }

    #[test]
    fn moved_bucket_storage_allows_existing_owned_stockpile() {
        let mut world = TestWorld::default();
        for grid in [(2, 0), (3, 0), (0, 0), (1, 0), (0, 1), (1, 1)] {
            world.bounds.insert(grid);
            world.walkable.insert(grid);
        }
        world.stockpiles.insert((2, 0));
        world.stockpiles.insert((3, 0));

        let geometry = bucket_storage_geometry((2, 0));
        let validation = validate_moved_bucket_storage_placement(
            &world,
            &geometry,
            &[(0, 0), (1, 0), (0, 1), (1, 1)],
            &[],
            &[(2, 0), (3, 0)],
            TANK_NEARBY_BUCKET_STORAGE_TILES,
        );

        assert!(validation.can_place);
    }
}
