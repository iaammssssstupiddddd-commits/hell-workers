//! 逃走ロジックのヘルパー
//!
//! Decide/Execute から共通利用する純粋判定関数とタイマーを定義する。

use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::familiar::Familiar;
use hw_spatial::FamiliarSpatialGrid;
use hw_world::SpatialGridOps;
use hw_world::coords::{grid_to_world, world_to_grid};
use hw_world::{
    PathGoalPolicy, PathSearchCaller, PathSearchResult, PathfindingContext,
    RuntimePathSearchBudget, WorldMap, find_path_with_budget,
};
use std::collections::HashMap;

use crate::soul_ai::helpers::gathering::GatheringSpot;

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
pub struct FamiliarThreat {
    pub entity: Entity,
    pub position: Vec2,
    pub distance: f32,
}

/// Budget を跨ぐ escape candidate search の再開位置。
///
/// A deferred candidate remains at `next_candidate`; candidates already
/// evaluated (and the best route distance) are not re-run on the next escape
/// behavior tick.
#[derive(Default)]
pub struct EscapePathSearchProgress {
    entries: HashMap<Entity, EscapeCandidateContinuation>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct EscapeSearchFingerprint {
    start_grid: (i32, i32),
    obstacle_version: u64,
}

struct EscapeCandidateContinuation {
    fingerprint: EscapeSearchFingerprint,
    candidates: Vec<Entity>,
    next_candidate: usize,
    best: Option<(Entity, f32)>,
}

impl EscapePathSearchProgress {
    fn reset_for(
        &mut self,
        entity: Entity,
        fingerprint: EscapeSearchFingerprint,
        candidates: Vec<Entity>,
    ) {
        self.entries.insert(
            entity,
            EscapeCandidateContinuation {
                fingerprint,
                candidates,
                next_candidate: 0,
                best: None,
            },
        );
    }

    fn needs_reset(&self, entity: Entity, fingerprint: EscapeSearchFingerprint) -> bool {
        self.entries
            .get(&entity)
            .is_none_or(|continuation| continuation.fingerprint != fingerprint)
    }

    pub fn clear_entity(&mut self, entity: Entity) {
        self.entries.remove(&entity);
    }
}

/// 最も近い使い魔を検出し、指定倍率内なら返す
pub fn detect_nearest_familiar_within_multiplier(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
    radius_multiplier: f32,
    scratch: &mut Vec<Entity>,
) -> Option<FamiliarThreat> {
    let search_radius = TILE_SIZE * 15.0;
    familiar_grid.get_nearby_in_radius_into(soul_pos, search_radius, scratch);

    let mut nearest: Option<FamiliarThreat> = None;

    for &fam_entity in scratch.iter() {
        if let Ok((transform, familiar)) = q_familiars.get(fam_entity) {
            let fam_pos = transform.translation.truncate();
            let distance = soul_pos.distance(fam_pos);
            let trigger_distance = familiar.command_radius * radius_multiplier;

            if distance < trigger_distance && nearest.is_none_or(|n| distance < n.distance) {
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
pub fn detect_nearest_familiar(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
    scratch: &mut Vec<Entity>,
) -> Option<FamiliarThreat> {
    detect_nearest_familiar_within_multiplier(
        soul_pos,
        familiar_grid,
        q_familiars,
        ESCAPE_TRIGGER_DISTANCE_MULTIPLIER,
        scratch,
    )
}

fn path_distance_world(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    budget: &mut RuntimePathSearchBudget,
    start: Vec2,
    goal: Vec2,
) -> PathSearchResult<f32> {
    let start_grid = world_to_grid(start);
    let goal_grid = world_to_grid(goal);
    let path = match find_path_with_budget(
        world_map,
        context,
        budget,
        PathSearchCaller::Escape,
        start_grid,
        goal_grid,
        PathGoalPolicy::RespectGoalWalkability,
    ) {
        PathSearchResult::Found(path) => path,
        PathSearchResult::Unreachable => return PathSearchResult::Unreachable,
        PathSearchResult::Deferred => return PathSearchResult::Deferred,
    };

    if path.len() < 2 {
        return PathSearchResult::Found(0.0);
    }

    let mut total = 0.0;
    let mut prev = grid_to_world(path[0].0, path[0].1);
    for &(x, y) in path.iter().skip(1) {
        let pos = grid_to_world(x, y);
        total += prev.distance(pos);
        prev = pos;
    }
    PathSearchResult::Found(total)
}

/// `detect_reachable_familiar_within_safe_distance` の探索入力。
pub(crate) struct EscapePathSearchInputs<'a, 'w, 's> {
    pub escaping_soul: Entity,
    pub progress: &'a mut EscapePathSearchProgress,
    pub soul_pos: Vec2,
    pub familiar_grid: &'a FamiliarSpatialGrid,
    pub q_familiars: &'a Query<'w, 's, (&'static Transform, &'static Familiar)>,
    pub world_map: &'a WorldMap,
    pub pf_context: &'a mut PathfindingContext,
    pub budget: &'a mut RuntimePathSearchBudget,
    pub scratch: &'a mut Vec<Entity>,
}

/// 最も近い使い魔を検出し、安全距離内かつ経路距離が到達可能な場合のみ返す
///
/// 候補探索中にbudgetが尽きた場合は、空間gridの候補順に依存した部分結果を採用せず、
/// `Deferred`をそのまま呼び出し元へ返す。
pub(crate) fn detect_reachable_familiar_within_safe_distance(
    inputs: EscapePathSearchInputs<'_, '_, '_>,
) -> PathSearchResult<FamiliarThreat> {
    let EscapePathSearchInputs {
        escaping_soul,
        progress,
        soul_pos,
        familiar_grid,
        q_familiars,
        world_map,
        pf_context,
        budget,
        scratch,
    } = inputs;
    let fingerprint = EscapeSearchFingerprint {
        start_grid: world_to_grid(soul_pos),
        obstacle_version: world_map.obstacle_version,
    };
    if progress.needs_reset(escaping_soul, fingerprint) {
        let search_radius = TILE_SIZE * 15.0;
        familiar_grid.get_nearby_in_radius_into(soul_pos, search_radius, scratch);
        let mut candidates = scratch.clone();
        candidates.sort_by_key(|entity| entity.to_bits());
        progress.reset_for(escaping_soul, fingerprint, candidates);
    }

    let Some(continuation) = progress.entries.get_mut(&escaping_soul) else {
        return PathSearchResult::Unreachable;
    };

    while continuation.next_candidate < continuation.candidates.len() {
        let fam_entity = continuation.candidates[continuation.next_candidate];
        if let Ok((transform, familiar)) = q_familiars.get(fam_entity) {
            let fam_pos = transform.translation.truncate();
            let euclid = soul_pos.distance(fam_pos);
            let safe_distance = familiar.command_radius * ESCAPE_SAFE_DISTANCE_MULTIPLIER;

            if euclid > safe_distance {
                continuation.next_candidate += 1;
                continue;
            }

            let skip_pathfinding_threshold =
                safe_distance * ESCAPE_PATHFINDING_SKIP_THRESHOLD_RATIO;
            if euclid < skip_pathfinding_threshold {
                if continuation
                    .best
                    .is_none_or(|(_, best_dist)| euclid < best_dist)
                {
                    continuation.best = Some((fam_entity, euclid));
                }
                continuation.next_candidate += 1;
                continue;
            }

            let path_dist =
                match path_distance_world(world_map, pf_context, budget, soul_pos, fam_pos) {
                    PathSearchResult::Found(path_dist) => path_dist,
                    PathSearchResult::Unreachable => {
                        continuation.next_candidate += 1;
                        continue;
                    }
                    PathSearchResult::Deferred => return PathSearchResult::Deferred,
                };

            if path_dist > safe_distance {
                continuation.next_candidate += 1;
                continue;
            }

            if continuation
                .best
                .is_none_or(|(_, best_dist)| path_dist < best_dist)
            {
                continuation.best = Some((fam_entity, path_dist));
            }
        }
        continuation.next_candidate += 1;
    }

    let best = continuation.best;
    progress.clear_entity(escaping_soul);
    let Some((best_entity, _)) = best else {
        return PathSearchResult::Unreachable;
    };
    let Ok((transform, _)) = q_familiars.get(best_entity) else {
        return PathSearchResult::Unreachable;
    };
    let position = transform.translation.truncate();
    PathSearchResult::Found(FamiliarThreat {
        entity: best_entity,
        position,
        distance: soul_pos.distance(position),
    })
}

/// 警戒圏内に使い魔がいるかを判定
pub fn is_escape_threat_close(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
    scratch: &mut Vec<Entity>,
) -> bool {
    detect_nearest_familiar(soul_pos, familiar_grid, q_familiars, scratch).is_some()
}

/// 逃走方向を計算
pub fn calculate_escape_destination(
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
        grid_to_world(gx, gy)
    } else {
        soul_pos
    }
}

fn nearest_familiar_info(
    pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
    scratch: &mut Vec<Entity>,
) -> Option<(f32, f32)> {
    let search_radius = TILE_SIZE * 15.0;
    familiar_grid.get_nearby_in_radius_into(pos, search_radius, scratch);
    let mut nearest: Option<(f32, f32)> = None;

    for &fam_entity in scratch.iter() {
        if let Ok((transform, familiar)) = q_familiars.get(fam_entity) {
            let dist = pos.distance(transform.translation.truncate());
            if nearest.is_none_or(|(best_dist, _)| dist < best_dist) {
                nearest = Some((dist, familiar.command_radius));
            }
        }
    }

    nearest
}

/// 安全な集会スポットを探す
pub fn find_safe_gathering_spot(
    soul_pos: Vec2,
    gathering_spots: &Query<(Entity, &GatheringSpot)>,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
    scratch: &mut Vec<Entity>,
) -> Option<Vec2> {
    let mut best_spot: Option<(Vec2, f32)> = None;

    for (_, spot) in gathering_spots.iter() {
        let spot_pos = spot.center;
        let dist_to_soul = soul_pos.distance(spot_pos);
        let nearest = nearest_familiar_info(spot_pos, familiar_grid, q_familiars, scratch);
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
            if best_spot.is_none_or(|(_, best_score)| score > best_score) {
                best_spot = Some((spot_pos, score));
            }
        }
    }

    best_spot.map(|(pos, _)| pos)
}
