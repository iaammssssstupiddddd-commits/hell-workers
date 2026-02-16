//! 集会中の移動先選定（ランダムリング + overlap 回避）

use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;
use crate::systems::soul_ai::helpers::gathering_positions::{
    find_position_fallback_away,
    find_position_with_separation,
    random_position_around,
};
use crate::systems::spatial::SpatialGridOps;
use crate::world::map::WorldMap;

/// 到着直後・中心に近すぎる場合の移動先を探索
pub fn find_initial_gathering_position<G: SpatialGridOps>(
    center: Vec2,
    current_pos: Vec2,
    exclude_entity: Entity,
    soul_grid: &G,
    world_map: &WorldMap,
) -> Option<Vec2> {
    const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;
    let min_dist = TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN;
    let max_dist = TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;

    find_position_with_separation(
        center,
        exclude_entity,
        soul_grid,
        world_map,
        min_dist,
        max_dist,
        MIN_SEPARATION,
        20,
    )
    .or_else(|| {
        find_position_fallback_away(
            center,
            current_pos,
            exclude_entity,
            soul_grid,
            world_map,
        )
    })
}

/// 集会中の Wandering サブ行動: パス完了後の新目的地（現在位置から十分離れた位置）
pub fn find_gathering_wandering_target<G: SpatialGridOps>(
    center: Vec2,
    current_pos: Vec2,
    exclude_entity: Entity,
    soul_grid: &G,
    world_map: &WorldMap,
) -> Option<Vec2> {
    const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;
    const MIN_DIST_FROM_CURRENT: f32 = TILE_SIZE * 2.0;
    let min_dist = TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN;
    let max_dist = TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;

    let mut rng = rand::thread_rng();
    for _ in 0..10 {
        let new_target = random_position_around(center, min_dist, max_dist);
        let dist_from_current = (new_target - current_pos).length();
        if dist_from_current < MIN_DIST_FROM_CURRENT {
            continue;
        }
        let nearby = soul_grid.get_nearby_in_radius(new_target, MIN_SEPARATION);
        if nearby.iter().any(|&e| e != exclude_entity) {
            continue;
        }
        let (gx, gy) = WorldMap::world_to_grid(new_target);
        if world_map.is_walkable(gx, gy) {
            return Some(new_target);
        }
    }
    for _ in 0..5 {
        let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let fallback_target =
            center + Vec2::new(angle.cos(), angle.sin()) * TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;
        let nearby = soul_grid.get_nearby_in_radius(fallback_target, MIN_SEPARATION);
        if nearby.iter().any(|&e| e != exclude_entity) {
            continue;
        }
        let (gx, gy) = WorldMap::world_to_grid(fallback_target);
        if world_map.is_walkable(gx, gy) {
            return Some(fallback_target);
        }
    }
    None
}

/// Sleeping/Standing/Dancing: 中心に近すぎる場合の退避先
pub fn find_gathering_still_retreat_target<G: SpatialGridOps>(
    center: Vec2,
    current_pos: Vec2,
    exclude_entity: Entity,
    soul_grid: &G,
    world_map: &WorldMap,
) -> Option<Vec2> {
    const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;
    let away = if (current_pos - center).length() > 0.1 {
        (current_pos - center).normalize_or_zero()
    } else {
        let mut rng = rand::thread_rng();
        let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        Vec2::new(angle.cos(), angle.sin())
    };
    let target =
        center + away * TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN;

    let nearby = soul_grid.get_nearby_in_radius(target, MIN_SEPARATION);
    if nearby.iter().any(|&e| e != exclude_entity) {
        return None;
    }
    let (gx, gy) = WorldMap::world_to_grid(target);
    if world_map.is_walkable(gx, gy) {
        Some(target)
    } else {
        None
    }
}
