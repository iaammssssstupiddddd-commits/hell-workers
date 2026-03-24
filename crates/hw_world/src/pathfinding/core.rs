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
    pub target_grid_set: HashSet<(i32, i32)>,
}

impl Default for PathfindingContext {
    fn default() -> Self {
        let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
        Self {
            g_scores: vec![i32::MAX; size],
            came_from: vec![None; size],
            open_set: BinaryHeap::with_capacity(size / 4),
            visited: Vec::with_capacity(512),
            target_grid_set: HashSet::with_capacity(64),
        }
    }
}

impl PathfindingContext {
    pub(super) fn reset(&mut self) {
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

pub(super) fn path_cost_heuristic(world_map: &impl PathWorld, idx: usize, goal_idx: usize) -> i32 {
    let p1 = world_map.idx_to_pos(idx);
    let p2 = world_map.idx_to_pos(goal_idx);
    let dx = (p1.0 - p2.0).abs();
    let dy = (p1.1 - p2.1).abs();
    let min_d = dx.min(dy);
    let max_d = dx.max(dy);
    MOVE_COST_DIAGONAL * min_d + MOVE_COST_STRAIGHT * (max_d - min_d)
}

pub(super) fn build_path_from_came_from(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn find_path_with_policy<FG, FM, FD, FE>(
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
