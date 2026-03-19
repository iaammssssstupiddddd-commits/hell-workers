use bevy::prelude::*;
use hw_jobs::BuildingType;
use std::collections::HashSet;

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
