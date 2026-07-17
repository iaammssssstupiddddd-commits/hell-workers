mod budget;
mod connectivity;
mod core;

#[cfg(feature = "profiling")]
pub use budget::RuntimePathSearchMetrics;
pub use budget::{PathSearchCaller, PathSearchResult, RuntimePathSearchBudget};
pub use connectivity::WalkabilityConnectivityCache;
pub use core::{
    MOVE_COST_DIAGONAL, MOVE_COST_STRAIGHT, PathGoalPolicy, PathNode, PathWorld, PathfindingContext,
};
use core::{PathPolicy, can_cross_diagonal_move, find_path_with_policy, path_cost_heuristic};

use hw_core::GridPos;

fn has_valid_find_path_input(
    world_map: &impl PathWorld,
    start: GridPos,
    goal: GridPos,
    goal_policy: PathGoalPolicy,
) -> bool {
    world_map.pos_to_idx(start.0, start.1).is_some()
        && world_map.pos_to_idx(goal.0, goal.1).is_some()
        && (!matches!(goal_policy, PathGoalPolicy::RespectGoalWalkability)
            || world_map.is_walkable(goal.0, goal.1))
}

pub(crate) fn find_path(
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
        PathPolicy {
            heuristic,
            is_goal_reached: move |pos: (i32, i32)| pos == goal,
            can_enter: |_from: (i32, i32), to: (i32, i32)| world_map.is_walkable(to.0, to.1),
            can_cross_diagonal: |from: (i32, i32), to: (i32, i32)| {
                can_cross_diagonal_move(world_map, from, to)
            },
            move_penalty: |x, y, _is_diagonal| world_map.get_door_cost(x, y),
        },
    )
}

/// Runs one core A* search when the frame budget permits it.
///
/// Invalid endpoints and a disallowed blocked goal never reach core A*, so
/// they are reported as `Unreachable` without consuming a budget slot.
pub fn find_path_with_budget(
    world_map: &impl PathWorld,
    context: &mut PathfindingContext,
    budget: &mut RuntimePathSearchBudget,
    caller: PathSearchCaller,
    start: GridPos,
    goal: GridPos,
    goal_policy: PathGoalPolicy,
) -> PathSearchResult<Vec<GridPos>> {
    if !has_valid_find_path_input(world_map, start, goal, goal_policy) {
        return PathSearchResult::Unreachable;
    }
    if !budget.try_claim_for(caller) {
        return PathSearchResult::Deferred;
    }

    let result = find_path(world_map, context, start, goal, goal_policy)
        .map_or(PathSearchResult::Unreachable, PathSearchResult::Found);
    #[cfg(feature = "profiling")]
    budget.record_expanded_nodes(context.expanded_nodes());
    result
}

pub(crate) fn find_path_to_adjacent(
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

/// Runs the adjacent-goal core A* search when the frame budget permits it.
pub fn find_path_to_adjacent_with_budget(
    world_map: &impl PathWorld,
    context: &mut PathfindingContext,
    budget: &mut RuntimePathSearchBudget,
    caller: PathSearchCaller,
    start: GridPos,
    target: GridPos,
    allow_goal_blocked: bool,
) -> PathSearchResult<Vec<GridPos>> {
    let allow_goal_blocked = allow_goal_blocked || !world_map.is_walkable(start.0, start.1);
    let policy = if allow_goal_blocked {
        PathGoalPolicy::AllowBlockedGoal
    } else {
        PathGoalPolicy::RespectGoalWalkability
    };

    // `find_path_to_adjacent` searches target -> start internally.
    if !has_valid_find_path_input(world_map, target, start, policy) {
        return PathSearchResult::Unreachable;
    }
    if !budget.try_claim_for(caller) {
        return PathSearchResult::Deferred;
    }

    let result = find_path_to_adjacent(world_map, context, start, target, allow_goal_blocked)
        .map_or(PathSearchResult::Unreachable, PathSearchResult::Found);
    #[cfg(feature = "profiling")]
    budget.record_expanded_nodes(context.expanded_nodes());
    result
}

pub(crate) fn can_reach_target(
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

fn try_exit_target_grids(
    world_map: &impl PathWorld,
    start: GridPos,
    target_grids: &[GridPos],
) -> Option<Vec<GridPos>> {
    if !target_grids.contains(&start) {
        return None;
    }

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
        let next = (start.0 + dx, start.1 + dy);
        if !target_grids.contains(&next) && world_map.is_walkable(next.0, next.1) {
            return Some(vec![start, next]);
        }
    }

    None
}

fn has_valid_boundary_search_input(
    world_map: &impl PathWorld,
    start: GridPos,
    target_grids: &[GridPos],
) -> bool {
    if target_grids.is_empty() || world_map.pos_to_idx(start.0, start.1).is_none() {
        return false;
    }

    let sum_x: f32 = target_grids.iter().map(|grid| grid.0 as f32).sum();
    let sum_y: f32 = target_grids.iter().map(|grid| grid.1 as f32).sum();
    let goal = (
        (sum_x / target_grids.len() as f32).round() as i32,
        (sum_y / target_grids.len() as f32).round() as i32,
    );
    world_map.pos_to_idx(goal.0, goal.1).is_some()
}

pub(crate) fn find_path_to_boundary(
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

    if let Some(path) = try_exit_target_grids(world_map, start, target_grids) {
        context.target_grid_set = target_grid_set;
        return Some(path);
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
        PathPolicy {
            heuristic,
            is_goal_reached: |pos: (i32, i32)| target_grid_set.contains(&pos),
            can_enter: |_: (i32, i32), to: (i32, i32)| {
                let is_in_target = target_grid_set.contains(&to);
                is_in_target || world_map.is_walkable(to.0, to.1)
            },
            can_cross_diagonal: |from: (i32, i32), to: (i32, i32)| {
                let dx = to.0 - from.0;
                let dy = to.1 - from.1;
                (world_map.is_walkable(from.0 + dx, from.1)
                    || target_grid_set.contains(&(from.0 + dx, from.1)))
                    && (world_map.is_walkable(from.0, from.1 + dy)
                        || target_grid_set.contains(&(from.0, from.1 + dy)))
            },
            move_penalty: |x, y, _is_diagonal| {
                if target_grid_set.contains(&(x, y)) {
                    0
                } else {
                    world_map.get_door_cost(x, y)
                }
            },
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

/// Runs the boundary-search core A* when the frame budget permits it.
///
/// An empty target set, invalid start/center, and the already-inside-target
/// walkable exit are resolved without starting core A*, so they do not claim a
/// slot.
pub fn find_path_to_boundary_with_budget(
    world_map: &impl PathWorld,
    context: &mut PathfindingContext,
    budget: &mut RuntimePathSearchBudget,
    caller: PathSearchCaller,
    start: GridPos,
    target_grids: &[GridPos],
) -> PathSearchResult<Vec<GridPos>> {
    if target_grids.is_empty() {
        return PathSearchResult::Unreachable;
    }
    if let Some(path) = try_exit_target_grids(world_map, start, target_grids) {
        return PathSearchResult::Found(path);
    }
    if !has_valid_boundary_search_input(world_map, start, target_grids) {
        return PathSearchResult::Unreachable;
    }
    if !budget.try_claim_for(caller) {
        return PathSearchResult::Deferred;
    }

    let result = find_path_to_boundary(world_map, context, start, target_grids)
        .map_or(PathSearchResult::Unreachable, PathSearchResult::Found);
    #[cfg(feature = "profiling")]
    budget.record_expanded_nodes(context.expanded_nodes());
    result
}

/// A direct route and the adjacent-goal fallback each claim an independent
/// core A* slot. Callers must preserve their state on `Deferred`.
pub fn find_path_world_waypoints_with_budget(
    world_map: &crate::map::WorldMap,
    pf_context: &mut PathfindingContext,
    budget: &mut RuntimePathSearchBudget,
    caller: PathSearchCaller,
    start_grid: GridPos,
    goal_grid: GridPos,
) -> PathSearchResult<Vec<bevy::math::Vec2>> {
    let direct = find_path_with_budget(
        world_map,
        pf_context,
        budget,
        caller,
        start_grid,
        goal_grid,
        PathGoalPolicy::RespectGoalWalkability,
    );

    let grid_path = match direct {
        PathSearchResult::Found(path) => PathSearchResult::Found(path),
        PathSearchResult::Deferred => return PathSearchResult::Deferred,
        PathSearchResult::Unreachable => find_path_to_adjacent_with_budget(
            world_map, pf_context, budget, caller, start_grid, goal_grid, true,
        ),
    };

    match grid_path {
        PathSearchResult::Found(path) => PathSearchResult::Found(
            path.iter()
                .map(|&(x, y)| crate::map::WorldMap::grid_to_world(x, y))
                .collect(),
        ),
        PathSearchResult::Unreachable => PathSearchResult::Unreachable,
        PathSearchResult::Deferred => PathSearchResult::Deferred,
    }
}

#[cfg(test)]
mod tests;
