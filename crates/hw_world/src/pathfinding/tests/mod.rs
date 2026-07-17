use super::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use std::collections::HashSet;

#[derive(Default)]
struct TestWorld {
    obstacles: HashSet<(i32, i32)>,
}

impl PathWorld for TestWorld {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        if !(0..MAP_WIDTH).contains(&x) || !(0..MAP_HEIGHT).contains(&y) {
            return None;
        }
        Some((y * MAP_WIDTH + x) as usize)
    }

    fn idx_to_pos(&self, idx: usize) -> GridPos {
        let x = idx as i32 % MAP_WIDTH;
        let y = idx as i32 / MAP_WIDTH;
        (x, y)
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.pos_to_idx(x, y).is_some() && !self.obstacles.contains(&(x, y))
    }

    fn get_door_cost(&self, _x: i32, _y: i32) -> i32 {
        0
    }
}

#[test]
fn test_path_to_boundary_1x1_open() {
    let map = TestWorld::default();
    let mut ctx = PathfindingContext::default();
    let target = vec![(5, 5)];

    let path = find_path_to_boundary(&map, &mut ctx, (1, 1), &target);
    assert!(path.is_some(), "Path should be found");
    let path = path.expect("path should be found");

    let last = path.last().expect("path is non-empty");
    let dx = (last.0 - 5).abs();
    let dy = (last.1 - 5).abs();
    assert!(
        dx <= 1 && dy <= 1,
        "Last {:?} should be adjacent to (5,5)",
        last
    );
    assert!(*last != (5, 5), "Last {:?} should not be target", last);
}

#[test]
fn budgeted_waypoint_search_charges_direct_and_adjacent_attempts_separately() {
    let mut map = crate::map::WorldMap::default();
    for y in 0..MAP_HEIGHT {
        map.add_grid_obstacle((50, y));
    }

    let start = (25, 50);
    let goal = (75, 50);
    let mut context = PathfindingContext::default();
    let mut one_slot = RuntimePathSearchBudget::new(1);

    assert!(matches!(
        find_path_world_waypoints_with_budget(
            &map,
            &mut context,
            &mut one_slot,
            PathSearchCaller::ActorNew,
            start,
            goal,
        ),
        PathSearchResult::Deferred
    ));
    assert_eq!(one_slot.used(), 1);

    let mut context = PathfindingContext::default();
    let mut two_slots = RuntimePathSearchBudget::new(2);
    assert!(matches!(
        find_path_world_waypoints_with_budget(
            &map,
            &mut context,
            &mut two_slots,
            PathSearchCaller::ActorNew,
            start,
            goal,
        ),
        PathSearchResult::Unreachable
    ));
    assert_eq!(two_slots.used(), 2);
}

#[test]
fn invalid_waypoint_search_does_not_consume_a_core_search_slot() {
    let map = crate::map::WorldMap::default();
    let mut context = PathfindingContext::default();
    let mut budget = RuntimePathSearchBudget::new(1);

    assert!(matches!(
        find_path_world_waypoints_with_budget(
            &map,
            &mut context,
            &mut budget,
            PathSearchCaller::ActorNew,
            (10, 10),
            (-1, 10),
        ),
        PathSearchResult::Unreachable
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn budgeted_boundary_search_preserves_zero_cost_inside_target_exit() {
    let map = TestWorld::default();
    let mut context = PathfindingContext::default();
    let mut budget = RuntimePathSearchBudget::new(0);

    assert!(matches!(
        find_path_to_boundary_with_budget(
            &map,
            &mut context,
            &mut budget,
            PathSearchCaller::ActorNew,
            (10, 10),
            &[(10, 10)],
        ),
        PathSearchResult::Found(path) if path.first() == Some(&(10, 10))
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn budgeted_boundary_search_defers_without_consuming_when_no_slot_remains() {
    let map = TestWorld::default();
    let mut context = PathfindingContext::default();
    let mut budget = RuntimePathSearchBudget::new(0);

    assert!(matches!(
        find_path_to_boundary_with_budget(
            &map,
            &mut context,
            &mut budget,
            PathSearchCaller::ActorNew,
            (10, 10),
            &[(20, 20)],
        ),
        PathSearchResult::Deferred
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn invalid_boundary_search_does_not_consume_a_core_search_slot() {
    let map = TestWorld::default();
    let mut context = PathfindingContext::default();
    let mut budget = RuntimePathSearchBudget::new(1);

    assert!(matches!(
        find_path_to_boundary_with_budget(
            &map,
            &mut context,
            &mut budget,
            PathSearchCaller::ActorNew,
            (10, 10),
            &[(MAP_WIDTH + 1, MAP_HEIGHT + 1)],
        ),
        PathSearchResult::Unreachable
    ));
    assert_eq!(budget.used(), 0);
}
