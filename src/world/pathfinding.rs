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
    pub allow_goal_obstacle: bool,
    /// 訪問済みインデックスを追跡（reset時の最適化用）
    visited: Vec<usize>,
}

impl Default for PathfindingContext {
    fn default() -> Self {
        let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
        Self {
            g_scores: vec![i32::MAX; size],
            came_from: vec![None; size],
            open_set: BinaryHeap::with_capacity(size / 4),
            allow_goal_obstacle: false,
            visited: Vec::with_capacity(512),
        }
    }
}

impl PathfindingContext {
    fn reset(&mut self) {
        // 訪問済みセルのみリセット（O(n) → O(k) 最適化）
        for &idx in &self.visited {
            self.g_scores[idx] = i32::MAX;
            self.came_from[idx] = None;
        }
        self.visited.clear();
        self.open_set.clear();
        self.allow_goal_obstacle = false;
    }
}

// A*パスファインディング
pub fn find_path(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: (i32, i32),
    goal: (i32, i32),
) -> Option<Vec<(i32, i32)>> {
    let start_idx = world_map.pos_to_idx(start.0, start.1)?;
    let goal_idx = world_map.pos_to_idx(goal.0, goal.1)?;

    // 目的地（逆引きならソウル）が通行不能なら到達不能
    // ただし、goal_can_be_obstacle が true の場合はチェックをスキップ
    if !world_map.is_walkable(goal.0, goal.1) && !context.allow_goal_obstacle {
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

    context.visited.push(start_idx);
    context.g_scores[start_idx] = 0;
    context.open_set.push(PathNode {
        idx: start_idx,
        f_cost: heuristic(start_idx, goal_idx),
    });

    // 8方向に拡張
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
        // ヒープに残った古いエントリ（より悪いコスト）をスキップ
        let recorded_g = context.g_scores[current.idx];
        if recorded_g == i32::MAX {
            continue;
        }
        let expected_f = recorded_g.saturating_add(heuristic(current.idx, goal_idx));
        if current.f_cost > expected_f {
            continue;
        }

        if current.idx == goal_idx {
            // パスを再構築
            let mut path = vec![goal];
            let mut curr = goal_idx;
            while let Some(prev) = context.came_from[curr] {
                path.push(WorldMap::idx_to_pos(prev));
                curr = prev;
                if curr == start_idx {
                    break;
                }
            }
            path.reverse();
            // 経路の平滑化を適用せずにダイレクトなグリッドパスを返す
            // return Some(smooth_path(world_map, path));
            return Some(path);
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
                if !world_map.is_walkable(curr_pos.0 + dx, curr_pos.1)
                    || !world_map.is_walkable(curr_pos.0, curr_pos.1 + dy)
                {
                    continue;
                }
            }

            // 移動コスト: 直線は10、斜めは14
            let move_cost = if is_diagonal {
                MOVE_COST_DIAGONAL
            } else {
                MOVE_COST_STRAIGHT
            };
            let tentative_g = current_g + move_cost;

            if tentative_g < context.g_scores[n_idx] {
                // 初訪問時のみ記録（重複防止）
                if context.g_scores[n_idx] == i32::MAX {
                    context.visited.push(n_idx);
                }
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
    target: (i32, i32),
) -> Option<Vec<(i32, i32)>> {
    // 逆引き検索を1回実行: ターゲット地点（岩など）から開始点（ソウル）に向かってパスを探す
    // ターゲット地点自体が通行不能でも、最初の展開（隣接マスへの移動）で通行可能マスに移行する
    // 開始点が通行不能な場合（アイテムの上にいるなど）は、allow_goal_obstacleを設定
    let start_walkable = world_map.is_walkable(start.0, start.1);
    if !start_walkable {
        context.allow_goal_obstacle = true;
    }

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

/// ターゲット（複数の占有マス）へ向かい、その境界（隣接する歩行可能タイル）で停止するパスを探索
/// ターゲット自体が障害物（非Walkable）であっても到達可能とする
/// ターゲット（複数の占有マス）の中心へ向かうパスを計算し、
/// 占有領域（障害物）に接触する直前の地点（境界）で停止するパスを返す。
///
/// アルゴリズム:
/// 1. 占有領域を「高いコストで歩行可能」とみなして、開始地点から中心地点までのA*探索を行う。
///    これにより、障害物を避けるのではなく、最短距離で障害物の「中」へ向かうパスが得られる。
/// 2. 得られたパスを開始地点から順にスキャンする。
/// 3. パス上の点が占有領域（target_grids）に含まれる最初の地点を見つける。
/// 4. その地点の「一つ手前」を最終的なゴールとし、パスをそこで切り詰める。
///
/// これにより、2x2などの大きな建築物の「どの側面が最も近いか」を自動的に判別し、
/// かつ中心座標がタイル境界にある場合でも適切に隣接タイルへのパスを生成できる。
pub fn find_path_to_boundary(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: (i32, i32),
    target_grids: &[(i32, i32)],
) -> Option<Vec<(i32, i32)>> {
    if target_grids.is_empty() {
        return None;
    }

    // すでにターゲット内にいる場合は、外へ脱出するための最短マスを探す
    if target_grids.contains(&start) {
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
            if !target_grids.contains(&(nx, ny)) && world_map.is_walkable(nx, ny) {
                return Some(vec![start, (nx, ny)]);
            }
        }
    }

    // 重心を計算して一意なターゲット地点（目標）とする
    let sum_x: f32 = target_grids.iter().map(|g| g.0 as f32).sum();
    let sum_y: f32 = target_grids.iter().map(|g| g.1 as f32).sum();
    let center_x = (sum_x / target_grids.len() as f32).round() as i32;
    let center_y = (sum_y / target_grids.len() as f32).round() as i32;
    let goal_grid = (center_x, center_y);
    let goal_idx = world_map.pos_to_idx(goal_grid.0, goal_grid.1)?;

    context.reset();
    let start_idx = world_map.pos_to_idx(start.0, start.1)?;

    // ヒューリスティック
    let heuristic = |idx: usize, g_idx: usize| -> i32 {
        let p1 = WorldMap::idx_to_pos(idx);
        let p2 = WorldMap::idx_to_pos(g_idx);
        let dx = (p1.0 - p2.0).abs();
        let dy = (p1.1 - p2.1).abs();
        let min_d = dx.min(dy);
        let max_d = dx.max(dy);
        MOVE_COST_DIAGONAL * min_d + MOVE_COST_STRAIGHT * (max_d - min_d)
    };

    context.visited.push(start_idx);
    context.g_scores[start_idx] = 0;
    context.open_set.push(PathNode {
        idx: start_idx,
        f_cost: heuristic(start_idx, goal_idx),
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
        // ヒープに残った古いエントリ（より悪いコスト）をスキップ
        let recorded_g = context.g_scores[current.idx];
        if recorded_g == i32::MAX {
            continue;
        }
        let expected_f = recorded_g.saturating_add(heuristic(current.idx, goal_idx));
        if current.f_cost > expected_f {
            continue;
        }

        let curr_pos = WorldMap::idx_to_pos(current.idx);

        // ターゲット領域のいずれかのマスに到達したなら、その手前でパスを生成して終了
        if target_grids.contains(&curr_pos) {
            let mut path = vec![curr_pos];
            let mut c = current.idx;
            while let Some(prev) = context.came_from[c] {
                path.push(WorldMap::idx_to_pos(prev));
                c = prev;
            }
            path.reverse();

            // ターゲット内の最後のノードを削除（境界で停止）
            path.pop();

            if path.is_empty() {
                return Some(vec![start]);
            }
            return Some(path);
        }

        let current_g = context.g_scores[current.idx];

        for (dx, dy) in &directions {
            let nx = curr_pos.0 + dx;
            let ny = curr_pos.1 + dy;

            let n_idx = match world_map.pos_to_idx(nx, ny) {
                Some(idx) => idx,
                None => continue,
            };

            // 歩行可能チェック: ターゲット内のマスは「透明」な障害物として扱い、許可する
            let is_in_target = target_grids.contains(&(nx, ny));
            if !world_map.is_walkable(nx, ny) && !is_in_target {
                continue;
            }

            // 角抜けチェック
            if dx.abs() == 1 && dy.abs() == 1 {
                let s1 = world_map.is_walkable(curr_pos.0 + dx, curr_pos.1)
                    || target_grids.contains(&(curr_pos.0 + dx, curr_pos.1));
                let s2 = world_map.is_walkable(curr_pos.0, curr_pos.1 + dy)
                    || target_grids.contains(&(curr_pos.0, curr_pos.1 + dy));
                if !s1 || !s2 {
                    continue;
                }
            }

            let move_cost = if dx.abs() == 1 && dy.abs() == 1 {
                MOVE_COST_DIAGONAL
            } else {
                MOVE_COST_STRAIGHT
            };
            let tentative_g = current_g + move_cost;

            if tentative_g < context.g_scores[n_idx] {
                // 初訪問時のみ記録（重複防止）
                if context.g_scores[n_idx] == i32::MAX {
                    context.visited.push(n_idx);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::WorldMap;

    fn create_test_map(_width: usize, _height: usize) -> WorldMap {
        WorldMap::default()
    }

    #[test]
    fn test_path_to_boundary_1x1_open() {
        let map = create_test_map(10, 10);
        let mut ctx = PathfindingContext::default();
        let target = vec![(5, 5)];

        let path = find_path_to_boundary(&map, &mut ctx, (1, 1), &target);
        assert!(path.is_some(), "Path should be found");
        let path = path.unwrap();

        // 1マスのターゲットの場合、最後はターゲットの隣接マスで終わるべき
        let last = path.last().unwrap();
        let dx = (last.0 - 5).abs();
        let dy = (last.1 - 5).abs();
        // 中心(5,5)への隣接条件: dx<=1 && dy<=1
        assert!(
            dx <= 1 && dy <= 1,
            "Last {:?} should be adjacent to (5,5)",
            last
        );
        assert!(*last != (5, 5), "Last {:?} should not be target", last);
    }
}
