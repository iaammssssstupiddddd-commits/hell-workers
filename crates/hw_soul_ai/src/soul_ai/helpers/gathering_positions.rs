//! 集会周辺のランダム位置生成と overlap 回避付き移動先探索

use bevy::prelude::*;
use rand::Rng;

use hw_core::constants::{GATHERING_KEEP_DISTANCE_TARGET_MAX, TILE_SIZE};
use hw_world::{PathWorld, SpatialGridOps, world_to_grid};

/// 中心周辺のランダムなリング上の位置を生成
pub fn random_position_around(center: Vec2, min_dist: f32, max_dist: f32) -> Vec2 {
    let mut rng = rand::thread_rng();
    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    let dist: f32 = rng.gen_range(min_dist..max_dist);
    center + Vec2::new(angle.cos() * dist, angle.sin() * dist)
}

/// `find_position_with_separation` の探索パラメータ。
pub struct SeparationParams {
    pub min_dist: f32,
    pub max_dist: f32,
    pub min_separation: f32,
    pub max_attempts: u32,
}

/// overlap 回避付きの移動先を探索。歩行可能かつ他 Soul と重ならない位置を返す。
pub fn find_position_with_separation<G: SpatialGridOps, W: PathWorld>(
    center: Vec2,
    exclude_entity: Entity,
    soul_grid: &G,
    world_map: &W,
    scratch: &mut Vec<Entity>,
    params: SeparationParams,
) -> Option<Vec2> {
    let mut rng = rand::thread_rng();
    for _ in 0..params.max_attempts {
        let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let dist: f32 = rng.gen_range(params.min_dist..params.max_dist);
        let new_pos = center + Vec2::new(angle.cos() * dist, angle.sin() * dist);

        soul_grid.get_nearby_in_radius_into(new_pos, params.min_separation, scratch);
        let occupied = scratch.iter().any(|&e| e != exclude_entity);
        if occupied {
            continue;
        }

        let (gx, gy) = world_to_grid(new_pos);
        if world_map.is_walkable(gx, gy) {
            return Some(new_pos);
        }
    }
    None
}

/// ランダム探索で見つからない場合のフォールバック: 中心の反対方向へ移動
pub fn find_position_fallback_away<G: SpatialGridOps, W: PathWorld>(
    center: Vec2,
    current_pos: Vec2,
    exclude_entity: Entity,
    soul_grid: &G,
    world_map: &W,
    scratch: &mut Vec<Entity>,
) -> Option<Vec2> {
    let away = if (current_pos - center).length() > 0.1 {
        (current_pos - center).normalize()
    } else {
        let mut rng = rand::thread_rng();
        let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        Vec2::new(angle.cos(), angle.sin())
    };
    let new_pos = center + away * TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;

    soul_grid.get_nearby_in_radius_into(new_pos, TILE_SIZE * 1.2, scratch);
    let occupied = scratch.iter().any(|&e| e != exclude_entity);
    if occupied {
        return None;
    }

    let (gx, gy) = world_to_grid(new_pos);
    if world_map.is_walkable(gx, gy) {
        Some(new_pos)
    } else {
        None
    }
}
