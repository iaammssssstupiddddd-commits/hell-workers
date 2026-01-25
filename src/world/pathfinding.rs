use crate::constants::{MAP_HEIGHT, MAP_WIDTH};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// 直線移動のコスト
pub const MOVE_COST_STRAIGHT: i32 = 10;
/// 斜め移動のコスト (10 * √2 ≈ 14.14)
pub const MOVE_COST_DIAGONAL: i32 = 14;

// A*のためのノード
#[derive(Clone, Eq, PartialEq)]
pub struct PathNode {
    pub idx: usize,
    pub f_cost: i32,
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_cost.cmp(&self.f_cost) // 最小ヒープにするため逆順
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 経路探索用の作業メモリを再利用するための構造体
pub struct PathfindingContext {
    pub g_scores: Vec<i32>,
    pub came_from: Vec<Option<usize>>,
    pub open_set: BinaryHeap<PathNode>,
}

impl Default for PathfindingContext {
    fn default() -> Self {
        let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
        Self {
            g_scores: vec![i32::MAX; size],
            came_from: vec![None; size],
            open_set: BinaryHeap::with_capacity(size / 4),
        }
    }
}

impl PathfindingContext {
    fn reset(&mut self) {
        self.g_scores.fill(i32::MAX);
        self.came_from.fill(None);
        self.open_set.clear();
    }
}

// A*パスファインディング
pub fn find_path(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: (i32, i32),
    goal: (i32, i32)
) -> Option<Vec<(i32, i32)>> {
    let start_idx = world_map.pos_to_idx(start.0, start.1)?;
    let goal_idx = world_map.pos_to_idx(goal.0, goal.1)?;

    // 目的地（逆引きならソウル）が通行不能なら到達不能
    if !world_map.is_walkable(goal.0, goal.1) {
        return None;
    }

    context.reset();

    let heuristic = |idx: usize, g_idx: usize| -> i32 {
        let p1 = WorldMap::idx_to_pos(idx);
        let p2 = WorldMap::idx_to_pos(g_idx);
        let dx = (p1.0 - p2.0).abs();
        let dy = (p1.1 - p2.1).abs();
        let min_d = dx.min(dy);
        let max_d = dx.max(dy);
        // Octile距離: 14 * min_d + 10 * (max_d - min_d)
        MOVE_COST_DIAGONAL * min_d + MOVE_COST_STRAIGHT * (max_d - min_d)
    };

    context.g_scores[start_idx] = 0;
    context.open_set.push(PathNode {
        idx: start_idx,
        f_cost: heuristic(start_idx, goal_idx),
    });

    // 8方向に拡張
    let directions = [
        (0, 1), (0, -1), (1, 0), (-1, 0),
        (1, 1), (1, -1), (-1, 1), (-1, -1)
    ];

    while let Some(current) = context.open_set.pop() {
        if current.idx == goal_idx {
            // パスを再構築
            let mut path = vec![goal];
            let mut curr = goal_idx;
            while let Some(prev) = context.came_from[curr] {
                path.push(WorldMap::idx_to_pos(prev));
                curr = prev;
                if curr == start_idx { break; }
            }
            path.reverse();
            // 経路の平滑化を適用
            return Some(smooth_path(world_map, path));
        }

        let curr_pos = WorldMap::idx_to_pos(current.idx);
        let current_g = context.g_scores[current.idx];

        for (dx, dy) in &directions {
            let nx = curr_pos.0 + dx;
            let ny = curr_pos.1 + dy;
            
            let n_idx = match world_map.pos_to_idx(nx, ny) {
                Some(idx) => idx,
                None => continue,
            };

            // 隣接マスが通行不能ならスキップ（目的地は許可済み）
            if !world_map.is_walkable(nx, ny) {
                continue;
            }

            // 斜め移動の場合の追加チェック
            let is_diagonal = dx.abs() == 1 && dy.abs() == 1;
            if is_diagonal {
                // 角抜け防止: 斜め移動の際、隣接する2マスが通行不能なら通れない
                // (x+dx, y) または (x, y+dy) のどちらかが通行不能な場合、通り抜けを制限する
                // ここでは「両方が通行可能」であることを条件とする
                if !world_map.is_walkable(curr_pos.0 + dx, curr_pos.1) || 
                   !world_map.is_walkable(curr_pos.0, curr_pos.1 + dy) {
                    continue;
                }
            }

            // 移動コスト: 直線は10、斜めは14
            let move_cost = if is_diagonal { MOVE_COST_DIAGONAL } else { MOVE_COST_STRAIGHT };
            let tentative_g = current_g + move_cost;

            if tentative_g < context.g_scores[n_idx] {
                context.came_from[n_idx] = Some(current.idx);
                context.g_scores[n_idx] = tentative_g;
                context.open_set.push(PathNode {
                    idx: n_idx,
                    f_cost: tentative_g + heuristic(n_idx, goal_idx),
                });
            }
        }
    }

    None
}

/// ターゲットの隣接マスへのパスを検索（ターゲット自体には入らない）
pub fn find_path_to_adjacent(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: (i32, i32),
    target: (i32, i32)
) -> Option<Vec<(i32, i32)>> {
    // 逆引き検索を1回実行: ターゲット地点（岩など）から開始点（ソウル）に向かってパスを探す
    // ターゲット地点自体が通行不能でも、最初の展開（隣接マスへの移動）で通行可能マスに移行する
    let mut path = find_path(world_map, context, target, start)?;
    
    // 得られたパスは [target, neighbor, ..., start]
    // これを逆転させて [start, ..., neighbor, target] にし、target を削除すれば隣接マス到着パスになる
    path.reverse();
    path.pop(); // ターゲット自体(岩の中心)を削除。これで隣接マスで止まる。
    
    if path.is_empty() {
        // すでにターゲットの隣にある場合、空の代わりに開始地点を返す（移動不要）
        Some(vec![start])
    } else {
        Some(path)
    }
}

/// 経路を平滑化する（直線で行ける場所を一直線に結ぶ）
pub fn smooth_path(world_map: &WorldMap, path: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    if path.len() <= 2 {
        return path;
    }

    let mut smoothed = vec![path[0]];
    let mut current = 0;

    while current < path.len() - 1 {
        let mut furthest_visible = current + 1;
        
        // 先のノードが今の位置から直線で見えるかチェック
        // パフォーマンスのため、後ろから順にチェックして最初に見つかった「見える点」を採用する
        for next in (current + 2..path.len()).rev() {
            if world_map.has_line_of_sight(path[current], path[next]) {
                furthest_visible = next;
                break;
            }
        }
        
        smoothed.push(path[furthest_visible]);
        current = furthest_visible;
    }

    smoothed
}
