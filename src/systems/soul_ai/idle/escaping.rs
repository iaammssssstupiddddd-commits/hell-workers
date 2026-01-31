//! 逃走システム - 使い魔からの逃走行動を管理
//!
//! 使役されていない魂が使い魔の影響圏から離れようとする行動を実装

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::{Familiar, UnderCommand};
use crate::systems::soul_ai::gathering::{GatheringSpot, ParticipatingIn};
use crate::systems::spatial::{FamiliarSpatialGrid, SpatialGridOps};
use crate::world::map::WorldMap;

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

/// 逃走対象となる使い魔情報
#[derive(Debug, Clone, Copy)]
struct FamiliarThreat {
    entity: Entity,
    position: Vec2,
    command_radius: f32,
    distance: f32,
}

/// 最も近い使い魔を検出し、指定倍率内なら返す
fn detect_nearest_familiar_within_multiplier(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
    radius_multiplier: f32,
) -> Option<FamiliarThreat> {
    let search_radius = TILE_SIZE * 15.0; // 広めに検索
    let nearby_familiars = familiar_grid.get_nearby_in_radius(soul_pos, search_radius);

    let mut nearest: Option<FamiliarThreat> = None;

    for fam_entity in nearby_familiars {
        if let Ok((transform, familiar)) = q_familiars.get(fam_entity) {
            let fam_pos = transform.translation.truncate();
            let distance = soul_pos.distance(fam_pos);
            let trigger_distance = familiar.command_radius * radius_multiplier;

            // 警戒圏内にいる場合
            if distance < trigger_distance {
                if nearest.map_or(true, |n| distance < n.distance) {
                    nearest = Some(FamiliarThreat {
                        entity: fam_entity,
                        position: fam_pos,
                        command_radius: familiar.command_radius,
                        distance,
                    });
                }
            }
        }
    }

    nearest
}

/// 最も近い使い魔を検出し、逃走が必要か判定
fn detect_nearest_familiar(
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

/// 警戒圏内に使い魔がいるかを判定
pub(crate) fn is_escape_threat_close(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
) -> bool {
    detect_nearest_familiar(soul_pos, familiar_grid, q_familiars).is_some()
}

/// 使い魔の影響範囲内にいるかを判定（command_radius）
pub(crate) fn is_familiar_in_influence_range(
    soul_pos: Vec2,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars: &Query<(&Transform, &Familiar)>,
) -> bool {
    detect_nearest_familiar_within_multiplier(soul_pos, familiar_grid, q_familiars, 1.0).is_some()
}

/// 逃走検出システム
/// 定期的に各Soulが使い魔から逃走すべきか判定
pub fn escaping_detection_system(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<EscapeDetectionTimer>,
    familiar_grid: Res<FamiliarSpatialGrid>,
    q_familiars: Query<(&Transform, &Familiar)>,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &DamnedSoul,
        Option<&UnderCommand>,
        Option<&ParticipatingIn>,
        &mut IdleState,
    )>,
) {
    if !timer.timer.tick(time.delta()).just_finished() {
        return;
    }

    for (entity, transform, soul, under_command, participating_in, mut idle_state) in
        q_souls.iter_mut()
    {
        // 既に逃走中ならスキップ
        if idle_state.behavior == IdleBehavior::Escaping {
            continue;
        }

        // 条件チェック: 使役中は逃走しない
        if under_command.is_some() {
            continue;
        }

        // 疲労による強制集会中は逃走しない
        if idle_state.behavior == IdleBehavior::ExhaustedGathering {
            continue;
        }

        // 条件チェック: ストレスが閾値を超えている
        if soul.stress <= ESCAPE_STRESS_THRESHOLD {
            continue;
        }

        let soul_pos = transform.translation.truncate();

        // 最も近い使い魔を検出
        if let Some(threat) = detect_nearest_familiar(soul_pos, &familiar_grid, &q_familiars) {
            // 逃走状態に遷移
            info!(
                "ESCAPE: Soul {:?} started escaping from Familiar {:?} (distance: {:.1}, stress: {:.2})",
                entity, threat.entity, threat.distance, soul.stress
            );
            // 集会中なら離脱
            if let Some(p) = participating_in {
                commands.entity(entity).remove::<ParticipatingIn>();
                commands.trigger(crate::events::OnGatheringLeft {
                    entity,
                    spot_entity: p.0,
                });
            }
            idle_state.behavior = IdleBehavior::Escaping;
            idle_state.idle_timer = 0.0;
            idle_state.behavior_duration = 5.0; // 初期行動時間
        }
    }
}

/// 逃走方向を計算
/// 使い魔から離れる方向 + 安全な集会スポットがある場合はそちらへ
fn calculate_escape_destination(
    soul_pos: Vec2,
    threat: &FamiliarThreat,
    gathering_spots: &Query<(Entity, &GatheringSpot)>,
    world_map: &WorldMap,
) -> Vec2 {
    // 1. 使い魔から離れる基本方向
    let away_direction = (soul_pos - threat.position).normalize_or_zero();

    // 2. 安全な集会スポットを探す
    let safe_spot = find_safe_gathering_spot(soul_pos, threat, gathering_spots);

    let desired = if let Some(spot_pos) = safe_spot {
        // 集会スポットが安全圏内ならそちらへ向かう
        let to_spot = (spot_pos - soul_pos).normalize_or_zero();
        // 合成: 使い魔から離れる70% + 集会スポットへ30%
        let combined = away_direction * 0.7 + to_spot * 0.3;
        soul_pos + combined.normalize_or_zero() * TILE_SIZE * 8.0
    } else {
        // 安全な集会スポットがない場合、単純に遠くへ
        soul_pos + away_direction * TILE_SIZE * 10.0
    };

    // 目的地を通行可能なグリッドにスナップ
    if let Some((gx, gy)) = world_map.get_nearest_walkable_grid(desired) {
        WorldMap::grid_to_world(gx, gy)
    } else {
        soul_pos
    }
}

/// 安全な集会スポットを探す
fn find_safe_gathering_spot(
    soul_pos: Vec2,
    threat: &FamiliarThreat,
    gathering_spots: &Query<(Entity, &GatheringSpot)>,
) -> Option<Vec2> {
    let mut best_spot: Option<(Vec2, f32)> = None; // (position, score)

    for (_, spot) in gathering_spots.iter() {
        let spot_pos = spot.center;
        let dist_to_soul = soul_pos.distance(spot_pos);
        let dist_to_familiar = spot_pos.distance(threat.position);
        let safe_distance = threat.command_radius * ESCAPE_SAFE_DISTANCE_MULTIPLIER;

        // Escaping状態のSoulは一定距離以内の集会スポットのみ対象
        if dist_to_soul > ESCAPE_GATHERING_JOIN_RADIUS {
            continue;
        }

        // 安全圏内にある集会スポットのみ対象
        if dist_to_familiar > safe_distance {
            // スコア: 近いほど高スコア、使い魔から遠いほど高スコア
            let score = (1000.0 / (dist_to_soul + 1.0)) + (dist_to_familiar / TILE_SIZE);

            if best_spot.map_or(true, |(_, best_score)| score > best_score) {
                best_spot = Some((spot_pos, score));
            }
        }
    }

    best_spot.map(|(pos, _)| pos)
}

/// 逃走行動システム
/// Escaping状態のSoulの移動を制御
pub fn escaping_behavior_system(
    _time: Res<Time>,
    world_map: Res<WorldMap>,
    familiar_grid: Res<FamiliarSpatialGrid>,
    q_familiars: Query<(&Transform, &Familiar)>,
    q_gathering_spots: Query<(Entity, &GatheringSpot)>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut IdleState,
            &mut Destination,
            &mut Path,
            &DamnedSoul,
            Option<&UnderCommand>,
        ),
    >,
) {
    for (entity, transform, mut idle_state, mut destination, mut path, _soul, under_command) in
        q_souls.iter_mut()
    {
        // Escaping状態のみ処理
        if idle_state.behavior != IdleBehavior::Escaping {
            continue;
        }

        // 使役が入ったら逃走終了
        if under_command.is_some() {
            idle_state.behavior = IdleBehavior::Wandering;
            idle_state.behavior_duration = 3.0;
            path.waypoints.clear();
            path.current_index = 0;
            continue;
        }

        let soul_pos = transform.translation.truncate();

        // 最も近い脅威を再検出
        if let Some(threat) = detect_nearest_familiar(soul_pos, &familiar_grid, &q_familiars) {
            // 安全圏内に到達したかチェック
            let safe_distance = threat.command_radius * ESCAPE_SAFE_DISTANCE_MULTIPLIER;

            if threat.distance > safe_distance {
                // 安全圏に到達 - 通常のWanderingに戻る
                info!(
                    "ESCAPE: Soul {:?} reached safety (distance: {:.1}), returning to Wandering",
                    entity, threat.distance
                );
                idle_state.behavior = IdleBehavior::Wandering;
                idle_state.behavior_duration = 3.0;
                path.waypoints.clear();
                continue;
            }

            // 逃走先を計算
            let escape_dest = calculate_escape_destination(
                soul_pos,
                &threat,
                &q_gathering_spots,
                &world_map,
            );

            // 目的地を更新（既存のパスが古いか、目的地が変わった場合）
            let current_dest = destination.0;
            if path.waypoints.is_empty()
                || current_dest.distance(escape_dest) > TILE_SIZE * 2.0
            {
                destination.0 = escape_dest;
                // Pathはsoul_movementシステムで計算される
                path.waypoints.clear();
                path.current_index = 0;
            }
        } else {
            // 脅威がなくなった - 通常のWanderingに戻る
            idle_state.behavior = IdleBehavior::Wandering;
            idle_state.behavior_duration = 3.0;
            path.waypoints.clear();
        }
    }
}

/// 逃走中のSoulが集会スポットに参加できるようにするシステム
/// （将来的な拡張用: Escaping状態のSoulは通常より遠くから集会に参加可能）
#[allow(dead_code)]
pub fn escaping_gathering_join_system() {
    // 現在はescaping_behavior_systemで処理
    // 必要に応じてEscaping中のSoulの集会参加距離を拡張する際に実装
}
