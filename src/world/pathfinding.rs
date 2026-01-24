// use bevy::prelude::*;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use crate::world::map::WorldMap;

// A*のためのノード
#[derive(Clone, Eq, PartialEq)]
pub struct PathNode {
    pub pos: (i32, i32),
    pub g_cost: i32,
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

// A*パスファインディング
pub fn find_path(world_map: &WorldMap, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    // 目的地（逆引きならソウル）が通行不能なら到達不能
    if !world_map.is_walkable(goal.0, goal.1) {
        return None;
    }

    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    let heuristic = |a: (i32, i32), b: (i32, i32)| -> i32 {
        ((a.0 - b.0).abs() + (a.1 - b.1).abs()) * 10
    };

    g_score.insert(start, 0);
    open_set.push(PathNode {
        pos: start,
        g_cost: 0,
        f_cost: heuristic(start, goal),
    });

    // 4方向に限定（斜め移動を廃止）
    let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    while let Some(current) = open_set.pop() {
        if current.pos == goal {
            // パスを再構築
            let mut path = vec![goal];
            let mut current_pos = goal;
            while let Some(&prev) = came_from.get(&current_pos) {
                path.push(prev);
                current_pos = prev;
            }
            path.reverse();
            return Some(path);
        }

        for (dx, dy) in &directions {
            let neighbor = (current.pos.0 + dx, current.pos.1 + dy);
            
            // 隣接マスが通行不能ならスキップ（目的地は許可済み）
            if !world_map.is_walkable(neighbor.0, neighbor.1) {
                continue;
            }

            // 直線移動のコスト（10）
            let tentative_g = g_score.get(&current.pos).unwrap_or(&i32::MAX) + 10;

            if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);
                open_set.push(PathNode {
                    pos: neighbor,
                    g_cost: tentative_g,
                    f_cost: tentative_g + heuristic(neighbor, goal),
                });
            }
        }
    }

    None
}

/// ターゲットの隣接マスへのパスを検索（ターゲット自体には入らない）
pub fn find_path_to_adjacent(world_map: &WorldMap, start: (i32, i32), target: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    // 逆引き検索を1回実行: ターゲット地点（岩など）から開始点（ソウル）に向かってパスを探す
    // ターゲット地点自体が通行不能でも、最初の展開（隣接マスへの移動）で通行可能マスに移行する
    let mut path = find_path(world_map, target, start)?;
    
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
