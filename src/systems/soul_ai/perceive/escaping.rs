//! 逃走ロジックのヘルパー
//!
//! Decide/Execute から共通利用する純粋判定関数とタイマーを定義する。

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::familiar::Familiar;
use crate::systems::soul_ai::gathering::GatheringSpot;
use crate::systems::spatial::{FamiliarSpatialGrid, SpatialGridOps};
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};

/// 逃走検出システム用のタイマーリソース
#[derive(Resource)]
pub struct EscapeDetectionTimer {
    pub timer: Timer,
}

impl Default for EscapeDetectionTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(ESCAPE_DETECTION_INTERVAL, TimerMode::Repeating),
        }
    }
}

/// 逃走行動システム用のタイマーリソース
#[derive(Resource)]
pub struct EscapeBehaviorTimer {
    pub timer: Timer,
    pub first_run_done: bool,
}

impl Default for EscapeBehaviorTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(ESCAPE_BEHAVIOR_INTERVAL, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

/// 逃走対象となる使い魔情報
#[derive(Debug, Clone, Copy)]
pub(crate) struct FamiliarThreat {
    pub entity: Entity,
    pub position: Vec2,
    pub distance: f32,
}

/// 最も近い使い魔を検出し、指定倍率内なら返す
pub(crate) fn detect_nearest_familiar_within_multiplier(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
    radius_multiplier: f32,
) -> Option<FamiliarThreat> {
    let search_radius = TILE_SIZE * 15.0;
    let nearby_familiars = familiar_grid.get_nearby_in_radius(soul_pos, search_radius);

    let mut nearest: Option<FamiliarThreat> = None;

    for fam_entity in nearby_familiars {
        if let Ok((transform, familiar)) = q_familiars.get(fam_entity) {
            let fam_pos = transform.translation.truncate();
            let distance = soul_pos.distance(fam_pos);
            let trigger_distance = familiar.command_radius * radius_multiplier;

            if distance < trigger_distance && nearest.map_or(true, |n| distance < n.distance) {
                nearest = Some(FamiliarThreat {
                    entity: fam_entity,
                    position: fam_pos,
                    distance,
                });
            }
        }
    }

    nearest
}

/// 最も近い使い魔を検出し、逃走が必要か判定
pub(crate) fn detect_nearest_familiar(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
) -> Option<FamiliarThreat> {
    detect_nearest_familiar_within_multiplier(
        soul_pos,
        familiar_grid,
        q_familiars,
        ESCAPE_TRIGGER_DISTANCE_MULTIPLIER,
    )
}

fn path_distance_world(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: Vec2,
    goal: Vec2,
) -> Option<f32> {
    let start_grid = WorldMap::world_to_grid(start);
    let goal_grid = WorldMap::world_to_grid(goal);
    let path = pathfinding::find_path(world_map, context, start_grid, goal_grid)?;
    if path.len() < 2 {
        return Some(0.0);
    }

    let mut total = 0.0;
    let mut prev = WorldMap::grid_to_world(path[0].0, path[0].1);
    for &(x, y) in path.iter().skip(1) {
        let pos = WorldMap::grid_to_world(x, y);
        total += prev.distance(pos);
        prev = pos;
    }
    Some(total)
}

/// 最も近い使い魔を検出し、安全距離内かつ経路距離が到達可能な場合のみ返す
pub(crate) fn detect_reachable_familiar_within_safe_distance(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
) -> Option<FamiliarThreat> {
    let search_radius = TILE_SIZE * 15.0;
    let nearby_familiars = familiar_grid.get_nearby_in_radius(soul_pos, search_radius);

    let mut best: Option<(FamiliarThreat, f32)> = None;

    for fam_entity in nearby_familiars {
        if let Ok((transform, familiar)) = q_familiars.get(fam_entity) {
            let fam_pos = transform.translation.truncate();
            let euclid = soul_pos.distance(fam_pos);
            let safe_distance = familiar.command_radius * ESCAPE_SAFE_DISTANCE_MULTIPLIER;

            if euclid > safe_distance {
                continue;
            }

            let Some(path_dist) = path_distance_world(world_map, pf_context, soul_pos, fam_pos)
            else {
                continue;
            };

            if path_dist > safe_distance {
                continue;
            }

            let threat = FamiliarThreat {
                entity: fam_entity,
                position: fam_pos,
                distance: euclid,
            };

            if best.map_or(true, |(_, best_dist)| path_dist < best_dist) {
                best = Some((threat, path_dist));
            }
        }
    }

    best.map(|(threat, _)| threat)
}

/// 警戒圏内に使い魔がいるかを判定
pub(crate) fn is_escape_threat_close(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
) -> bool {
    detect_nearest_familiar(soul_pos, familiar_grid, q_familiars).is_some()
}

/// 逃走方向を計算
pub(crate) fn calculate_escape_destination(
    soul_pos: Vec2,
    threat: &FamiliarThreat,
    safe_spot: Option<Vec2>,
    world_map: &WorldMap,
) -> Vec2 {
    let away_direction = (soul_pos - threat.position).normalize_or_zero();

    let desired = if let Some(spot_pos) = safe_spot {
        let to_spot = (spot_pos - soul_pos).normalize_or_zero();
        let combined = away_direction * 0.7 + to_spot * 0.3;
        soul_pos + combined.normalize_or_zero() * TILE_SIZE * 8.0
    } else {
        soul_pos + away_direction * TILE_SIZE * 10.0
    };

    if let Some((gx, gy)) = world_map.get_nearest_walkable_grid(desired) {
        WorldMap::grid_to_world(gx, gy)
    } else {
        soul_pos
    }
}

fn nearest_familiar_info(
    pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
) -> Option<(f32, f32)> {
    let search_radius = TILE_SIZE * 15.0;
    let nearby = familiar_grid.get_nearby_in_radius(pos, search_radius);
    let mut nearest: Option<(f32, f32)> = None;

    for fam_entity in nearby {
        if let Ok((transform, familiar)) = q_familiars.get(fam_entity) {
            let dist = pos.distance(transform.translation.truncate());
            if nearest.map_or(true, |(best_dist, _)| dist < best_dist) {
                nearest = Some((dist, familiar.command_radius));
            }
        }
    }

    nearest
}

/// 安全な集会スポットを探す
pub(crate) fn find_safe_gathering_spot(
    soul_pos: Vec2,
    gathering_spots: &Query<(Entity, &GatheringSpot)>,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
) -> Option<Vec2> {
    let mut best_spot: Option<(Vec2, f32)> = None;

    for (_, spot) in gathering_spots.iter() {
        let spot_pos = spot.center;
        let dist_to_soul = soul_pos.distance(spot_pos);
        let nearest = nearest_familiar_info(spot_pos, familiar_grid, q_familiars);
        let (dist_to_familiar, safe_distance) = match nearest {
            None => (TILE_SIZE * 1000.0, 0.0),
            Some((dist, command_radius)) => {
                (dist, command_radius * ESCAPE_SAFE_DISTANCE_MULTIPLIER)
            }
        };

        if dist_to_soul > ESCAPE_GATHERING_JOIN_RADIUS {
            continue;
        }

        if dist_to_familiar > safe_distance {
            let score = (1000.0 / (dist_to_soul + 1.0)) + (dist_to_familiar / TILE_SIZE);
            if best_spot.map_or(true, |(_, best_score)| score > best_score) {
                best_spot = Some((spot_pos, score));
            }
        }
    }

    best_spot.map(|(pos, _)| pos)
}
