//! 集会周辺のランダム位置生成と overlap 回避付き移動先探索

use rand::Rng;

use crate::constants::{GATHERING_KEEP_DISTANCE_TARGET_MAX, TILE_SIZE};
use crate::systems::spatial::SpatialGridOps;
use crate::world::map::WorldMap;

/// 中心周辺のランダムなリング上の位置を生成
pub fn random_position_around(
    center: bevy::prelude::Vec2,
    min_dist: f32,
    max_dist: f32,
) -> bevy::prelude::Vec2 {
    let mut rng = rand::thread_rng();
    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    let dist: f32 = rng.gen_range(min_dist..max_dist);
    center + bevy::prelude::Vec2::new(angle.cos() * dist, angle.sin() * dist)
}

/// overlap 回避付きの移動先を探索。歩行可能かつ他 Soul と重ならない位置を返す。
pub fn find_position_with_separation<G: SpatialGridOps>(
    center: bevy::prelude::Vec2,
    exclude_entity: bevy::prelude::Entity,
    soul_grid: &G,
    world_map: &WorldMap,
    min_dist: f32,
    max_dist: f32,
    min_separation: f32,
    max_attempts: u32,
) -> Option<bevy::prelude::Vec2> {
    let mut rng = rand::thread_rng();
    for _ in 0..max_attempts {
        let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let dist: f32 = rng.gen_range(min_dist..max_dist);
        let new_pos = center + bevy::prelude::Vec2::new(angle.cos() * dist, angle.sin() * dist);

        let nearby = soul_grid.get_nearby_in_radius(new_pos, min_separation);
        let occupied = nearby.iter().any(|&e| e != exclude_entity);
        if occupied {
            continue;
        }

        let (gx, gy) = WorldMap::world_to_grid(new_pos);
        if world_map.is_walkable(gx, gy) {
            return Some(new_pos);
        }
    }
    None
}

/// ランダム探索で見つからない場合のフォールバック: 中心の反対方向へ移動
pub fn find_position_fallback_away<G: SpatialGridOps>(
    center: bevy::prelude::Vec2,
    current_pos: bevy::prelude::Vec2,
    exclude_entity: bevy::prelude::Entity,
    soul_grid: &G,
    world_map: &WorldMap,
) -> Option<bevy::prelude::Vec2> {
    let away = if (current_pos - center).length() > 0.1 {
        (current_pos - center).normalize()
    } else {
        let mut rng = rand::thread_rng();
        let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        bevy::prelude::Vec2::new(angle.cos(), angle.sin())
    };
    let new_pos = center + away * TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;

    let nearby = soul_grid.get_nearby_in_radius(new_pos, TILE_SIZE * 1.2);
    let occupied = nearby.iter().any(|&e| e != exclude_entity);
    if occupied {
        return None;
    }

    let (gx, gy) = WorldMap::world_to_grid(new_pos);
    if world_map.is_walkable(gx, gy) {
        Some(new_pos)
    } else {
        None
    }
}
