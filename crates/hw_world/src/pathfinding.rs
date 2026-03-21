use hw_core::GridPos;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

/// `hw_world` が要求する最小限の通行判定 API。
pub trait PathWorld {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize>;
    fn idx_to_pos(&self, idx: usize) -> GridPos;
    fn is_walkable(&self, x: i32, y: i32) -> bool;
    fn get_door_cost(&self, x: i32, y: i32) -> i32;
}

/// 直線移動のコスト
pub const MOVE_COST_STRAIGHT: i32 = 10;
/// 斜め移動のコスト (10 * √2 ≈ 14.14)
pub const MOVE_COST_DIAGONAL: i32 = 14;

#[derive(Clone, Eq, PartialEq)]
pub struct PathNode {
    pub idx: usize,
    pub f_cost: i32,
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_cost.cmp(&self.f_cost)
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct PathfindingContext {
    pub g_scores: Vec<i32>,
    pub came_from: Vec<Option<usize>>,
    pub open_set: BinaryHeap<PathNode>,
    visited: Vec<usize>,
}

impl Default for PathfindingContext {
    fn default() -> Self {
        let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
        Self {
            g_scores: vec![i32::MAX; size],
            came_from: vec![None; size],
            open_set: BinaryHeap::with_capacity(size / 4),
            visited: Vec::with_capacity(512),
        }
    }
}

impl PathfindingContext {
    fn reset(&mut self) {
        for &idx in &self.visited {
            self.g_scores[idx] = i32::MAX;
            self.came_from[idx] = None;
        }
        self.visited.clear();
        self.open_set.clear();
    }
}

#[derive(Clone, Copy)]
pub enum PathGoalPolicy {
    RespectGoalWalkability,
    AllowBlockedGoal,
}

fn path_cost_heuristic(world_map: &impl PathWorld, idx: usize, goal_idx: usize) -> i32 {
    let p1 = world_map.idx_to_pos(idx);
    let p2 = world_map.idx_to_pos(goal_idx);
    let dx = (p1.0 - p2.0).abs();
    let dy = (p1.1 - p2.1).abs();
    let min_d = dx.min(dy);
    let max_d = dx.max(dy);
    MOVE_COST_DIAGONAL * min_d + MOVE_COST_STRAIGHT * (max_d - min_d)
}

fn build_path_from_came_from(
    world_map: &impl PathWorld,
    came_from: &[Option<usize>],
    mut current_idx: usize,
    start_idx: usize,
) -> Vec<(i32, i32)> {
    let mut path = vec![world_map.idx_to_pos(current_idx)];

    while current_idx != start_idx {
        let Some(prev_idx) = came_from[current_idx] else {
            break;
        };
        current_idx = prev_idx;
        path.push(world_map.idx_to_pos(current_idx));
    }

    path.reverse();
    path
}

fn find_path_with_policy<FG, FM, FD, FE>(
    world_map: &impl PathWorld,
    context: &mut PathfindingContext,
    start_idx: usize,
    heuristic: impl Fn(usize) -> i32,
    is_goal_reached: FG,
    can_enter: FM,
    can_cross_diagonal: FD,
    move_penalty: FE,
) -> Option<Vec<(i32, i32)>>
where
    FG: Fn((i32, i32)) -> bool,
    FM: Fn((i32, i32), (i32, i32)) -> bool,
    FD: Fn((i32, i32), (i32, i32)) -> bool,
    FE: Fn(i32, i32, bool) -> i32,
{
    context.reset();

    context.visited.push(start_idx);
    context.g_scores[start_idx] = 0;
    context.open_set.push(PathNode {
        idx: start_idx,
        f_cost: heuristic(start_idx),
    });

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

    while let Some(current) = context.open_set.pop() {
        let recorded_g = context.g_scores[current.idx];
        if recorded_g == i32::MAX {
            continue;
        }
        let expected_f = recorded_g.saturating_add(heuristic(current.idx));
        if current.f_cost > expected_f {
            continue;
        }

        let curr_pos = world_map.idx_to_pos(current.idx);
        if is_goal_reached(curr_pos) {
            return Some(build_path_from_came_from(
                world_map,
                &context.came_from,
                current.idx,
                start_idx,
            ));
        }

        for (dx, dy) in &directions {
            let nx = curr_pos.0 + dx;
            let ny = curr_pos.1 + dy;

            let n_idx = match world_map.pos_to_idx(nx, ny) {
                Some(idx) => idx,
                None => continue,
            };

            if !can_enter(curr_pos, (nx, ny)) {
                continue;
            }

            let is_diagonal = dx.abs() == 1 && dy.abs() == 1;
            if is_diagonal && !can_cross_diagonal(curr_pos, (nx, ny)) {
                continue;
            }

            let move_cost = if is_diagonal {
                MOVE_COST_DIAGONAL
            } else {
                MOVE_COST_STRAIGHT
            };
            let penalty = move_penalty(nx, ny, is_diagonal);
            let tentative_g = recorded_g + move_cost + penalty;

            if tentative_g < context.g_scores[n_idx] {
                if context.g_scores[n_idx] == i32::MAX {
                    context.visited.push(n_idx);
                }
                context.came_from[n_idx] = Some(current.idx);
                context.g_scores[n_idx] = tentative_g;
                context.open_set.push(PathNode {
                    idx: n_idx,
                    f_cost: tentative_g + heuristic(n_idx),
                });
            }
        }
    }

    None
}

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

    let target_grid_set: HashSet<(i32, i32)> = target_grids.iter().copied().collect();

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
                return Some(vec![start, (nx, ny)]);
            }
        }
    }

    let sum_x: f32 = target_grids.iter().map(|g| g.0 as f32).sum();
    let sum_y: f32 = target_grids.iter().map(|g| g.1 as f32).sum();
    let center_x = (sum_x / target_grids.len() as f32).round() as i32;
    let center_y = (sum_y / target_grids.len() as f32).round() as i32;
    let goal_grid = (center_x, center_y);
    let goal_idx = world_map.pos_to_idx(goal_grid.0, goal_grid.1)?;

    let start_idx = world_map.pos_to_idx(start.0, start.1)?;
    let heuristic = |idx| path_cost_heuristic(world_map, idx, goal_idx);
    let mut path = find_path_with_policy(
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
    )?;

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
        let path = path.unwrap();

        let last = path.last().unwrap();
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
