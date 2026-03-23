mod core;

pub use core::{
    MOVE_COST_DIAGONAL, MOVE_COST_STRAIGHT, PathGoalPolicy, PathNode, PathWorld, PathfindingContext,
};
use core::{find_path_with_policy, path_cost_heuristic};

use hw_core::GridPos;

pub fn find_path(
    world_map: &impl PathWorld,
    context: &mut PathfindingContext,
    start: GridPos,
    goal: GridPos,
    goal_policy: PathGoalPolicy,
) -> Option<Vec<GridPos>> {
    let start_idx = world_map.pos_to_idx(start.0, start.1)?;
    let goal_idx = world_map.pos_to_idx(goal.0, goal.1)?;

    if matches!(goal_policy, PathGoalPolicy::RespectGoalWalkability)
        && !world_map.is_walkable(goal.0, goal.1)
    {
        return None;
    }

    let heuristic = |idx| path_cost_heuristic(world_map, idx, goal_idx);
    find_path_with_policy(
        world_map,
        context,
        start_idx,
        heuristic,
        move |pos| pos == goal,
        |_from, to| world_map.is_walkable(to.0, to.1),
        |from, to| {
            let dx = to.0 - from.0;
            let dy = to.1 - from.1;
            let is_diagonal = dx.abs() == 1 && dy.abs() == 1;
            if !is_diagonal {
                return true;
            }
            world_map.is_walkable(from.0 + dx, from.1) && world_map.is_walkable(from.0, from.1 + dy)
        },
        |x, y, _is_diagonal| world_map.get_door_cost(x, y),
    )
}

pub fn find_path_to_adjacent(
    world_map: &impl PathWorld,
    context: &mut PathfindingContext,
    start: GridPos,
    target: GridPos,
    allow_goal_blocked: bool,
) -> Option<Vec<GridPos>> {
    let allow_goal_blocked = allow_goal_blocked || !world_map.is_walkable(start.0, start.1);
    let policy = if allow_goal_blocked {
        PathGoalPolicy::AllowBlockedGoal
    } else {
        PathGoalPolicy::RespectGoalWalkability
    };

    let mut path = find_path(world_map, context, target, start, policy)?;
    path.reverse();
    path.pop();

    if path.is_empty() {
        Some(vec![start])
    } else {
        Some(path)
    }
}

pub fn can_reach_target(
    world_map: &impl PathWorld,
    context: &mut PathfindingContext,
    start: GridPos,
    target: GridPos,
    target_walkable: bool,
) -> bool {
    if target_walkable {
        find_path(
            world_map,
            context,
            target,
            start,
            PathGoalPolicy::RespectGoalWalkability,
        )
        .is_some()
            || find_path_to_adjacent(world_map, context, start, target, true).is_some()
    } else {
        find_path_to_adjacent(world_map, context, start, target, true).is_some()
    }
}

pub fn find_path_to_boundary(
    world_map: &impl PathWorld,
    context: &mut PathfindingContext,
    start: GridPos,
    target_grids: &[GridPos],
) -> Option<Vec<GridPos>> {
    if target_grids.is_empty() {
        return None;
    }

    // context から一時退避することで find_path_with_policy への &mut context との borrow 競合を回避する
    let mut target_grid_set = std::mem::take(&mut context.target_grid_set);
    target_grid_set.clear();
    target_grid_set.extend(target_grids.iter().copied());

    if target_grid_set.contains(&start) {
        let directions = [
            (0, 1),
            (0, -1),
            (1, 0),
            (-1, 0),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        for (dx, dy) in directions {
            let nx = start.0 + dx;
            let ny = start.1 + dy;
            if !target_grid_set.contains(&(nx, ny)) && world_map.is_walkable(nx, ny) {
                context.target_grid_set = target_grid_set;
                return Some(vec![start, (nx, ny)]);
            }
        }
    }

    let sum_x: f32 = target_grids.iter().map(|g| g.0 as f32).sum();
    let sum_y: f32 = target_grids.iter().map(|g| g.1 as f32).sum();
    let center_x = (sum_x / target_grids.len() as f32).round() as i32;
    let center_y = (sum_y / target_grids.len() as f32).round() as i32;
    let goal_grid = (center_x, center_y);
    let goal_idx = world_map.pos_to_idx(goal_grid.0, goal_grid.1);

    if goal_idx.is_none() {
        context.target_grid_set = target_grid_set;
        return None;
    }
    let goal_idx = goal_idx.unwrap();

    let start_idx = world_map.pos_to_idx(start.0, start.1);
    if start_idx.is_none() {
        context.target_grid_set = target_grid_set;
        return None;
    }
    let start_idx = start_idx.unwrap();

    let heuristic = |idx| path_cost_heuristic(world_map, idx, goal_idx);
    let path_result = find_path_with_policy(
        world_map,
        context,
        start_idx,
        heuristic,
        |pos| target_grid_set.contains(&pos),
        |_, to| {
            let is_in_target = target_grid_set.contains(&to);
            is_in_target || world_map.is_walkable(to.0, to.1)
        },
        |from, to| {
            let dx = to.0 - from.0;
            let dy = to.1 - from.1;
            (world_map.is_walkable(from.0 + dx, from.1)
                || target_grid_set.contains(&(from.0 + dx, from.1)))
                && (world_map.is_walkable(from.0, from.1 + dy)
                    || target_grid_set.contains(&(from.0, from.1 + dy)))
        },
        |x, y, _is_diagonal| {
            if target_grid_set.contains(&(x, y)) {
                0
            } else {
                world_map.get_door_cost(x, y)
            }
        },
    );

    context.target_grid_set = target_grid_set;

    let mut path = path_result?;
    path.pop();
    if path.is_empty() {
        Some(vec![start])
    } else {
        Some(path)
    }
}

/// A* でパスを探索し、ワールド座標 waypoint 列として返す。
/// 直接到達不可なターゲットには隣接マスへの探索を fallback する。
///
/// `find_path` → `find_path_to_adjacent` の fallback + grid→world 変換を1関数に集約。
pub fn find_path_world_waypoints(
    world_map: &crate::map::WorldMap,
    pf_context: &mut PathfindingContext,
    start_grid: GridPos,
    goal_grid: GridPos,
) -> Option<Vec<bevy::math::Vec2>> {
    find_path(
        world_map,
        pf_context,
        start_grid,
        goal_grid,
        PathGoalPolicy::RespectGoalWalkability,
    )
    .or_else(|| find_path_to_adjacent(world_map, pf_context, start_grid, goal_grid, true))
    .map(|grid_path| {
        grid_path
            .iter()
            .map(|&(x, y)| crate::map::WorldMap::grid_to_world(x, y))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
    use std::collections::HashSet;

    struct TestWorld {
        obstacles: HashSet<(i32, i32)>,
    }

    impl Default for TestWorld {
        fn default() -> Self {
            Self {
                obstacles: HashSet::new(),
            }
        }
    }

    impl PathWorld for TestWorld {
        fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
            if x < 0 || x >= MAP_WIDTH || y < 0 || y >= MAP_HEIGHT {
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
}
