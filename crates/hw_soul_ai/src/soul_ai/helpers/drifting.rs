//! 漂流（Drifting）AI の純粋計算ヘルパー
//!
//! `Commands` / root-only Resource に依存しない純粋関数群。
//! ECS 反映は root 側の drifting system が担う。

use bevy::prelude::Vec2;
use hw_core::constants::{
    DRIFT_LATERAL_OFFSET_MAX, DRIFT_MOVE_TILES_MAX, DRIFT_MOVE_TILES_MIN,
    MAP_HEIGHT, MAP_WIDTH, SOUL_DESPAWN_EDGE_MARGIN_TILES,
};
use hw_core::soul::DriftEdge;
use hw_world::map::WorldMap;
use hw_world::{RIVER_Y_MAX, RIVER_Y_MIN};
use rand::Rng;

/// 現在のグリッド座標から、最も近いマップ端方向を選ぶ。
///
/// 川を挟んだ方向はスキップする（川の内側にいる場合のみ）。
pub fn choose_drift_edge(grid: (i32, i32)) -> DriftEdge {
    let (x, y) = grid;

    let mut candidates = vec![
        (DriftEdge::North, y),
        (DriftEdge::South, (MAP_HEIGHT - 1 - y).max(0)),
        (DriftEdge::West, x),
        (DriftEdge::East, (MAP_WIDTH - 1 - x).max(0)),
    ];

    if y < RIVER_Y_MIN {
        candidates.retain(|(edge, _)| !matches!(edge, DriftEdge::South));
    } else if y > RIVER_Y_MAX {
        candidates.retain(|(edge, _)| !matches!(edge, DriftEdge::North));
    }

    candidates
        .into_iter()
        .min_by_key(|(_, dist)| *dist)
        .map(|(edge, _)| edge)
        .unwrap_or(DriftEdge::South)
}

/// ソウルがマップ端の despawn 境界に到達しているかを判定する。
pub fn is_near_map_edge(grid: (i32, i32)) -> bool {
    grid.0 <= SOUL_DESPAWN_EDGE_MARGIN_TILES
        || grid.0 >= MAP_WIDTH - 1 - SOUL_DESPAWN_EDGE_MARGIN_TILES
        || grid.1 <= SOUL_DESPAWN_EDGE_MARGIN_TILES
        || grid.1 >= MAP_HEIGHT - 1 - SOUL_DESPAWN_EDGE_MARGIN_TILES
}

/// 現在グリッド周辺のランダムな歩行可能点を返す（Wandering フェーズ用）。
pub fn random_wander_target(grid: (i32, i32), world_map: &WorldMap, rng: &mut impl Rng) -> Vec2 {
    for _ in 0..24 {
        let dx = rng.gen_range(-4..=4);
        let dy = rng.gen_range(-4..=4);
        let target = (grid.0 + dx, grid.1 + dy);
        if world_map.is_walkable(target.0, target.1) {
            return WorldMap::grid_to_world(target.0, target.1);
        }
    }
    WorldMap::grid_to_world(grid.0, grid.1)
}

/// 指定した端方向に向けた漂流移動目的地を返す（Moving フェーズ用）。
pub fn drift_move_target(
    current_grid: (i32, i32),
    edge: DriftEdge,
    world_map: &WorldMap,
    rng: &mut impl Rng,
) -> Vec2 {
    let drift_tiles = rng.gen_range(DRIFT_MOVE_TILES_MIN..=DRIFT_MOVE_TILES_MAX);
    let lateral = rng.gen_range(-DRIFT_LATERAL_OFFSET_MAX..=DRIFT_LATERAL_OFFSET_MAX);

    let desired = match edge {
        DriftEdge::North => (current_grid.0 + lateral, current_grid.1 - drift_tiles),
        DriftEdge::South => (current_grid.0 + lateral, current_grid.1 + drift_tiles),
        DriftEdge::East => (current_grid.0 + drift_tiles, current_grid.1 + lateral),
        DriftEdge::West => (current_grid.0 - drift_tiles, current_grid.1 + lateral),
    };

    let clamped = (
        desired.0.clamp(0, MAP_WIDTH - 1),
        desired.1.clamp(0, MAP_HEIGHT - 1),
    );

    if world_map.is_walkable(clamped.0, clamped.1) {
        return WorldMap::grid_to_world(clamped.0, clamped.1);
    }

    let desired_world = WorldMap::grid_to_world(clamped.0, clamped.1);
    world_map
        .get_nearest_walkable_grid(desired_world)
        .map(|(gx, gy)| WorldMap::grid_to_world(gx, gy))
        .unwrap_or_else(|| WorldMap::grid_to_world(current_grid.0, current_grid.1))
}
