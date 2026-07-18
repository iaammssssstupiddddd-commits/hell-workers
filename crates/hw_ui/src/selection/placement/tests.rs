use bevy::prelude::*;
use hw_jobs::BuildingType;
use std::collections::{HashMap, HashSet};

use super::*;

#[derive(Default)]
struct TestWorld {
    buildings: HashSet<(i32, i32)>,
    stockpiles: HashSet<(i32, i32)>,
    raw_obstacles: HashSet<(i32, i32)>,
    walkable: HashSet<(i32, i32)>,
    river: HashSet<(i32, i32)>,
    bounds: HashSet<(i32, i32)>,
    building_entities: HashMap<(i32, i32), Entity>,
}

impl WorldReadApi for TestWorld {
    fn has_building(&self, grid: (i32, i32)) -> bool {
        self.buildings.contains(&grid)
    }

    fn has_stockpile(&self, grid: (i32, i32)) -> bool {
        self.stockpiles.contains(&grid)
    }

    fn has_raw_obstacle(&self, grid: (i32, i32)) -> bool {
        self.raw_obstacles.contains(&grid)
    }

    fn is_walkable(&self, gx: i32, gy: i32) -> bool {
        self.walkable.contains(&(gx, gy))
    }

    fn is_river_tile(&self, gx: i32, gy: i32) -> bool {
        self.river.contains(&(gx, gy))
    }

    fn building_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.building_entities.get(&grid).copied()
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
fn bridge_rejects_non_building_obstacle_on_river() {
    let mut world = TestWorld::default();
    world.bounds.insert((0, 0));
    world.river.insert((0, 0));
    // Natural, reservation, and construction blockers do not necessarily
    // have a WorldMap building owner.
    world.raw_obstacles.insert((0, 0));

    let geometry = building_geometry(BuildingType::Bridge, (0, 0), 0);
    let ctx = BuildingPlacementContext {
        world: &world,
        in_site: true,
        in_yard: true,
        is_wall_or_door_at: &|_| false,
        is_replaceable_wall_at: &|_| false,
    };

    let validation = validate_building_placement(&ctx, BuildingType::Bridge, (0, 0), &geometry);
    assert_eq!(
        validation.reject_reason,
        Some(PlacementRejectReason::NotWalkable)
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

#[test]
fn bucket_storage_rejects_parent_footprint_overlap_during_preview() {
    let mut world = TestWorld::default();
    for grid in [(0, 0), (1, 0)] {
        world.bounds.insert(grid);
        world.walkable.insert(grid);
    }
    let geometry = bucket_storage_geometry((0, 0));

    let validation = validate_bucket_storage_placement(
        &world,
        &geometry,
        &[(0, 0), (1, 0)],
        true,
        TANK_NEARBY_BUCKET_STORAGE_TILES,
    );

    assert_eq!(
        validation,
        PlacementValidation::rejected_at(PlacementRejectReason::OccupiedByBuilding, (0, 0))
    );
}

#[test]
fn every_reject_reason_has_non_empty_display_text() {
    assert_eq!(PlacementRejectReason::ALL.len(), 14);
    for reason in PlacementRejectReason::ALL {
        assert!(!reason.message(3, 4).trim().is_empty());
    }
}

#[test]
fn moved_building_allows_self_occupancy_but_rejects_other_owners() {
    let mut world = TestWorld::default();
    let moved = Entity::from_bits(1);
    let other = Entity::from_bits(2);
    for grid in [(0, 0), (1, 0)] {
        world.bounds.insert(grid);
        world.walkable.insert(grid);
        world.buildings.insert(grid);
    }
    world.building_entities.insert((0, 0), moved);
    world.building_entities.insert((1, 0), other);

    assert_eq!(
        validate_moved_building_placement(&world, moved, &[(0, 0)], &[(0, 0)]),
        PlacementValidation::ok()
    );
    assert_eq!(
        validate_moved_building_placement(&world, moved, &[(0, 0)], &[(1, 0)]),
        PlacementValidation::rejected_at(PlacementRejectReason::OccupiedByBuilding, (1, 0))
    );
}

#[test]
fn moved_building_reports_stockpile_bounds_and_walkability() {
    let moved = Entity::from_bits(1);
    let mut stockpile_world = TestWorld::default();
    stockpile_world.bounds.insert((2, 0));
    stockpile_world.walkable.insert((2, 0));
    stockpile_world.stockpiles.insert((2, 0));
    assert_eq!(
        validate_moved_building_placement(&stockpile_world, moved, &[], &[(2, 0)]),
        PlacementValidation::rejected_at(PlacementRejectReason::OccupiedByStockpile, (2, 0))
    );

    assert_eq!(
        validate_moved_building_placement(&TestWorld::default(), moved, &[], &[(3, 0)]),
        PlacementValidation::rejected_at(PlacementRejectReason::OutOfBounds, (3, 0))
    );

    let mut blocked_world = TestWorld::default();
    blocked_world.bounds.insert((4, 0));
    assert_eq!(
        validate_moved_building_placement(&blocked_world, moved, &[], &[(4, 0)]),
        PlacementValidation::rejected_at(PlacementRejectReason::NotWalkable, (4, 0))
    );
}

#[test]
fn floor_and_wall_report_out_of_bounds_before_walkability() {
    let world = TestWorld::default();
    assert_eq!(
        validate_floor_tile(&world, (99, 99), &HashSet::new(), &HashSet::new()),
        Some(PlacementRejectReason::OutOfBounds)
    );
    assert_eq!(
        validate_wall_tile(&world, (99, 99), &HashSet::new()),
        Some(PlacementRejectReason::OutOfBounds)
    );
}

#[test]
fn area_structure_validation_distinguishes_size_and_line_shape() {
    assert_eq!(
        validate_area_size(hw_core::constants::FLOOR_MAX_AREA_SIZE + 1, 1),
        Some(PlacementRejectReason::AreaTooLarge)
    );
    assert_eq!(
        validate_wall_area(2, 2),
        Some(PlacementRejectReason::NotStraightLine)
    );
    assert_eq!(validate_wall_area(1, 5), None);
}

#[test]
fn area_plan_distinguishes_all_valid_partial_and_rejected() {
    let all_valid = build_area_placement_plan((0, 0), (1, 0), None, |_| None);
    assert_eq!(all_valid.valid_tiles, vec![(0, 0), (1, 0)]);
    assert!(all_valid.feedback().is_none());

    let partial = build_area_placement_plan((0, 0), (1, 0), None, |grid| {
        (grid == (1, 0)).then_some(PlacementRejectReason::OccupiedByBuilding)
    });
    let partial_feedback = partial.feedback().unwrap();
    assert_eq!(partial_feedback.status, PlacementFeedbackStatus::Partial);
    assert_eq!(partial_feedback.valid_tile_count, 1);
    assert_eq!(partial_feedback.rejected_tile_count, 1);

    let rejected = build_area_placement_plan((0, 0), (1, 0), None, |_| {
        Some(PlacementRejectReason::NoCompletedFloor)
    });
    assert_eq!(
        rejected.feedback().unwrap().status,
        PlacementFeedbackStatus::Rejected
    );

    let structural = build_area_placement_plan(
        (0, 0),
        (2, 1),
        Some(PlacementRejectReason::NotStraightLine),
        |_| None,
    );
    assert!(structural.valid_tiles.is_empty());
    assert_eq!(structural.rejected_tile_count(), 6);
    assert_eq!(
        structural.feedback().unwrap().reason,
        PlacementRejectReason::NotStraightLine
    );
}

#[test]
fn live_feedback_precedes_recent_failure_and_recent_uses_real_elapsed_time() {
    let mut state = PlacementFeedbackState::default();
    state.show_recent_rejection(
        PlacementRejectReason::OutOfBounds,
        (0, 0),
        std::time::Duration::ZERO,
    );
    state.live = Some(PlacementFeedback::rejected(
        PlacementRejectReason::NotInYard,
        (1, 1),
    ));

    assert_eq!(
        state
            .visible(std::time::Duration::from_secs(1))
            .unwrap()
            .reason,
        PlacementRejectReason::NotInYard
    );
    state.live = None;
    assert_eq!(
        state
            .visible(std::time::Duration::from_secs(1))
            .unwrap()
            .reason,
        PlacementRejectReason::OutOfBounds
    );
    assert!(state.visible(std::time::Duration::from_secs(2)).is_none());
}

#[test]
fn frame_start_clears_live_feedback_without_dropping_fresh_recent_failure() {
    let mut state = PlacementFeedbackState {
        live: Some(PlacementFeedback::rejected(
            PlacementRejectReason::NotInSite,
            (0, 0),
        )),
        ..default()
    };
    state.show_recent_rejection(
        PlacementRejectReason::OutOfBounds,
        (1, 1),
        std::time::Duration::ZERO,
    );

    state.begin_frame(std::time::Duration::from_secs(1));

    assert!(state.live.is_none());
    assert_eq!(
        state
            .visible(std::time::Duration::from_secs(1))
            .unwrap()
            .reason,
        PlacementRejectReason::OutOfBounds
    );
}

#[test]
fn soul_spa_geometry_matches_its_spawn_footprint() {
    let geometry = building_geometry(BuildingType::SoulSpa, (10, 10), 0);
    assert_eq!(
        geometry.occupied_grids,
        vec![(10, 10), (11, 10), (10, 9), (11, 9)]
    );
    assert_eq!(
        geometry.size,
        Vec2::splat(hw_core::constants::TILE_SIZE * 2.0)
    );
}
