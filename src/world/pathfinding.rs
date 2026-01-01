use bevy::prelude::*;
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

    let directions = [
        (0, 1), (0, -1), (1, 0), (-1, 0),
        (1, 1), (1, -1), (-1, 1), (-1, -1),
    ];

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
            
            if !world_map.is_walkable(neighbor.0, neighbor.1) {
                continue;
            }

            // 斜め移動のコスト（14）と直線移動のコスト（10）
            let move_cost = if *dx != 0 && *dy != 0 { 14 } else { 10 };
            let tentative_g = g_score.get(&current.pos).unwrap_or(&i32::MAX) + move_cost;

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
