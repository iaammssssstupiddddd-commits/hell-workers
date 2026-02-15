use bevy::prelude::*;

use rand::Rng;

use crate::constants::*;
use crate::entities::damned_soul::{GatheringBehavior, IdleBehavior};
use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::gathering::{
    GATHERING_LEAVE_RADIUS, GatheringSpot,
};
use crate::systems::soul_ai::helpers::query_types::IdleDecisionSoulQuery;
use crate::systems::spatial::{GatheringSpotSpatialGrid, SpatialGridOps};
use crate::world::map::WorldMap;

// ===== 集会関連の定数 =====
/// 集会エリアに「到着した」とみなす半径
pub(crate) const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE;

// ===== ヘルパー関数 =====
/// ランダムな集会中のサブ行動を選択
fn random_gathering_behavior() -> GatheringBehavior {
    let mut rng = rand::thread_rng();
    match rng.gen_range(0..4) {
        0 => GatheringBehavior::Wandering,
        1 => GatheringBehavior::Sleeping,
        2 => GatheringBehavior::Standing,
        _ => GatheringBehavior::Dancing,
    }
}

/// ランダムな集会行動の持続時間を取得
fn random_gathering_duration() -> f32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(GATHERING_BEHAVIOR_DURATION_MIN..GATHERING_BEHAVIOR_DURATION_MAX)
}

/// 集会エリア周辺のランダムな位置を取得
fn random_position_around(center: Vec2, min_dist: f32, max_dist: f32) -> Vec2 {
    let mut rng = rand::thread_rng();
    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    let dist: f32 = rng.gen_range(min_dist..max_dist);
    center + Vec2::new(angle.cos() * dist, angle.sin() * dist)
}

/// アイドル行動の決定システム (Decide Phase)
///
/// 怠惰行動のAIロジック。やる気が低い魂は怠惰な行動をする。
/// タスクがある魂は怠惰行動をしない。
///
/// このシステムはIdleState, Destination, Pathの更新と、
/// IdleBehaviorRequestの発行を行う。実際のエンティティ操作は
/// idle_behavior_apply_systemで行われる。
pub fn idle_behavior_decision_system(
    time: Res<Time>,
    mut request_writer: MessageWriter<IdleBehaviorRequest>,
    world_map: Res<WorldMap>,
    q_spots: Query<(Entity, &GatheringSpot, &crate::relationships::GatheringParticipants)>,
    mut query: IdleDecisionSoulQuery,
    spot_grid: Res<GatheringSpotSpatialGrid>,
    soul_grid: Res<crate::systems::spatial::SpatialGrid>,
) {
    let dt = time.delta_secs();

    for (entity, transform, mut idle, mut dest, soul, mut path, task, participating_in) in
        query.iter_mut()
    {
        // 参加中の集会スポットの座標とEntityを取得、または最寄りのスポットを探す
        let (gathering_center, target_spot_entity): (Option<Vec2>, Option<Entity>) =
            if let Some(p) = participating_in {
                let center = q_spots.get(p.0).ok().map(|(_, s, _)| s.center);
                (center, Some(p.0))
            } else {
                // 最寄りのスポットを空間グリッドで効率的に探す
                let pos = transform.translation.truncate();
                let spot_entities =
                    spot_grid.get_nearby_in_radius(pos, GATHERING_LEAVE_RADIUS * 2.0);

                let nearest = spot_entities
                    .iter()
                    .filter_map(|&e| q_spots.get(e).ok())
                    .filter(|item| item.2.len() < item.1.max_capacity)
                    .min_by(|a, b| {
                        a.1.center
                            .distance_squared(pos)
                            .partial_cmp(&b.1.center.distance_squared(pos))
                            .unwrap()
                    });
                match nearest {
                    Some((e, s, _)) => (Some(s.center), Some(e)),
                    None => (None, None),
                }
            };

        // 疲労による強制集会（ExhaustedGathering）状態の場合は他の処理をスキップ
        if idle.behavior == IdleBehavior::ExhaustedGathering {
            if let Some(center) = gathering_center {
                let current_pos = transform.translation.truncate();
                let dist_from_center = (center - current_pos).length();
                let has_arrived = dist_from_center <= GATHERING_ARRIVAL_RADIUS;

                if has_arrived {
                    debug!("IDLE: Soul transitioned from ExhaustedGathering to Gathering");
                    idle.behavior = IdleBehavior::Gathering;
                    idle.needs_separation = true; // 到着時に重なり回避を発動
                    // ParticipatingIn を追加（Executeフェーズで処理）
                    if participating_in.is_none() {
                        if let Some(spot_entity) = target_spot_entity {
                            request_writer.write(IdleBehaviorRequest {
                                entity,
                                operation: IdleBehaviorOperation::ArriveAtGathering { spot_entity },
                            });
                        }
                    }
                } else {
                    if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                        dest.0 = center;
                        path.waypoints.clear();
                    }
                    continue;
                }
            } else {
                // 向かうべき集会所がない場合はうろうろに戻る
                idle.behavior = IdleBehavior::Wandering;
            }
        }

        if !matches!(&*task, AssignedTask::None) {
            // タスク割り当て時は集会を解除
            if let Some(p) = participating_in {
                request_writer.write(IdleBehaviorRequest {
                    entity,
                    operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
                });
            }
            if idle.behavior != IdleBehavior::Wandering {
                idle.behavior = IdleBehavior::Wandering;
                idle.idle_timer = 0.0;
                idle.behavior_duration = 3.0;
                idle.needs_separation = false;
            }
            idle.total_idle_time = 0.0;
            continue;
        }

        // 逃走中（Escaping）は escaping_decision_system に任せる
        if idle.behavior == IdleBehavior::Escaping {
            continue;
        }

        idle.total_idle_time += dt;

        if soul.motivation > MOTIVATION_THRESHOLD && soul.fatigue < FATIGUE_IDLE_THRESHOLD {
            continue;
        }

        idle.idle_timer += dt;

        if idle.idle_timer >= idle.behavior_duration {
            idle.idle_timer = 0.0;

            if soul.fatigue > FATIGUE_GATHERING_THRESHOLD
                || idle.total_idle_time > IDLE_TIME_TO_GATHERING
            {
                if idle.behavior != IdleBehavior::Gathering
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                {
                    idle.gathering_behavior = random_gathering_behavior();
                    idle.gathering_behavior_timer = 0.0;
                    idle.gathering_behavior_duration = random_gathering_duration();
                    idle.needs_separation = true;
                }

                if soul.fatigue > FATIGUE_GATHERING_THRESHOLD {
                    idle.behavior = IdleBehavior::ExhaustedGathering;
                } else {
                    idle.behavior = IdleBehavior::Gathering;
                }
            } else {
                let mut rng = rand::thread_rng();
                let roll: f32 = rng.gen_range(0.0..1.0);

                idle.behavior = if soul.laziness > LAZINESS_THRESHOLD_HIGH {
                    if roll < 0.6 {
                        IdleBehavior::Sleeping
                    } else if roll < 0.9 {
                        IdleBehavior::Sitting
                    } else {
                        IdleBehavior::Wandering
                    }
                } else if soul.laziness > LAZINESS_THRESHOLD_MID {
                    if roll < 0.3 {
                        IdleBehavior::Sleeping
                    } else if roll < 0.6 {
                        IdleBehavior::Sitting
                    } else {
                        IdleBehavior::Wandering
                    }
                } else {
                    if roll < 0.7 {
                        IdleBehavior::Wandering
                    } else {
                        IdleBehavior::Sitting
                    }
                };
            }

            let mut rng = rand::thread_rng();
            idle.behavior_duration = match idle.behavior {
                IdleBehavior::Sleeping => {
                    rng.gen_range(IDLE_DURATION_SLEEP_MIN..IDLE_DURATION_SLEEP_MAX)
                }
                IdleBehavior::Sitting => {
                    rng.gen_range(IDLE_DURATION_SIT_MIN..IDLE_DURATION_SIT_MAX)
                }
                IdleBehavior::Wandering => {
                    rng.gen_range(IDLE_DURATION_WANDER_MIN..IDLE_DURATION_WANDER_MAX)
                }
                IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                    rng.gen_range(IDLE_DURATION_WANDER_MIN..IDLE_DURATION_WANDER_MAX)
                }
                IdleBehavior::Escaping => {
                    // 逃走中は短い間隔で再評価
                    2.0
                }
            };
        }

        match idle.behavior {
            IdleBehavior::Wandering => {
                if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                    let current_pos = transform.translation.truncate();
                    let current_grid = WorldMap::world_to_grid(current_pos);

                    let mut rng = rand::thread_rng();
                    for _ in 0..10 {
                        let dx: i32 = rng.gen_range(-5..=5);
                        let dy: i32 = rng.gen_range(-5..=5);
                        let new_grid = (current_grid.0 + dx, current_grid.1 + dy);

                        if world_map.is_walkable(new_grid.0, new_grid.1) {
                            let new_pos = WorldMap::grid_to_world(new_grid.0, new_grid.1);
                            dest.0 = new_pos;
                            break;
                        }
                    }
                }
            }
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                if let Some(center) = gathering_center {
                    let current_pos = transform.translation.truncate();
                    let dist_from_center = (center - current_pos).length();

                    idle.gathering_behavior_timer += dt;
                    if idle.gathering_behavior_timer >= idle.gathering_behavior_duration {
                        idle.gathering_behavior_timer = 0.0;
                        idle.gathering_behavior = random_gathering_behavior();
                        idle.gathering_behavior_duration = random_gathering_duration();
                        idle.needs_separation = true;
                    }

                    if dist_from_center > GATHERING_ARRIVAL_RADIUS {
                        if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                            dest.0 = center;
                        }
                    } else {
                        // 到着時にParticipatingInを追加（Executeフェーズで処理）
                        let just_arrived = participating_in.is_none();
                        if just_arrived {
                            if let Some(spot_entity) = target_spot_entity {
                                request_writer.write(IdleBehaviorRequest {
                                    entity,
                                    operation: IdleBehaviorOperation::JoinGathering { spot_entity },
                                });
                            }
                        }

                        if idle.behavior == IdleBehavior::ExhaustedGathering {
                            idle.behavior = IdleBehavior::Gathering;
                        }

                        // 到着直後、または中心に近すぎる場合は離れた位置を設定
                        // ただし、既に移動中（waypointsがある）の場合はスキップ（separation_systemによる移動を妨げない）
                        let is_moving = !path.waypoints.is_empty() && path.current_index < path.waypoints.len();

                        if !is_moving && (just_arrived || dist_from_center < TILE_SIZE * GATHERING_KEEP_DISTANCE_MIN) {
                            let mut found = false;
                            const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;

                            for _ in 0..20 {
                                let new_target = random_position_around(
                                    center,
                                    TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN,
                                    TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX,
                                );

                                // 他のSoulと重ならないかチェック
                                let nearby_souls = soul_grid.get_nearby_in_radius(new_target, MIN_SEPARATION);
                                let position_occupied = nearby_souls.iter().any(|&other| other != entity);

                                if !position_occupied {
                                    let target_grid = WorldMap::world_to_grid(new_target);
                                    if world_map.is_walkable(target_grid.0, target_grid.1) {
                                        dest.0 = new_target;
                                        path.waypoints.clear();
                                        path.current_index = 0;
                                        found = true;
                                        break;
                                    }
                                }
                            }
                            if !found {
                                // 見つからない場合は中心の反対方向に移動
                                // ただしoverlapチェックを行う
                                let away = (current_pos - center).normalize_or_zero();
                                let fallback_target = center + away * TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;

                                // フォールバック位置でもoverlapをチェック
                                let nearby_souls = soul_grid.get_nearby_in_radius(fallback_target, MIN_SEPARATION);
                                let position_occupied = nearby_souls.iter().any(|&other| other != entity);

                                if !position_occupied {
                                    let target_grid = WorldMap::world_to_grid(fallback_target);
                                    if world_map.is_walkable(target_grid.0, target_grid.1) {
                                        dest.0 = fallback_target;
                                        path.waypoints.clear();
                                        path.current_index = 0;
                                    }
                                }
                                // overlap回避できない場合は目的地を変更しない（separation_systemに任せる）
                            }
                        }

                        match idle.gathering_behavior {
                            GatheringBehavior::Wandering => {
                                let path_complete = path.waypoints.is_empty()
                                    || path.current_index >= path.waypoints.len();
                                // パス完了時に新しい目的地を設定（うろつき）
                                if path_complete {
                                    let mut found = false;
                                    const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;

                                    for _ in 0..10 {
                                        let new_target = random_position_around(
                                            center,
                                            TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN,
                                            TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX,
                                        );

                                        // 現在位置から十分離れているか（うろつき感を出すため）
                                        let dist_from_current = (new_target - current_pos).length();
                                        if dist_from_current < TILE_SIZE * 2.0 {
                                            continue; // 近すぎる場合はスキップ
                                        }

                                        // 他のSoulと重ならないかチェック
                                        let nearby_souls = soul_grid.get_nearby_in_radius(new_target, MIN_SEPARATION);
                                        let position_occupied = nearby_souls.iter().any(|&other| other != entity);

                                        if !position_occupied {
                                            let target_grid = WorldMap::world_to_grid(new_target);
                                            if world_map.is_walkable(target_grid.0, target_grid.1) {
                                                dest.0 = new_target;
                                                path.waypoints.clear();
                                                path.current_index = 0;
                                                found = true;
                                                break;
                                            }
                                        }
                                    }
                                    if !found {
                                        // 見つからない場合はランダムな方向に移動
                                        // ただしoverlapチェックを行う
                                        let mut rng = rand::thread_rng();
                                        let mut fallback_found = false;

                                        // フォールバック用の追加試行
                                        for _ in 0..5 {
                                            let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                                            let dist = TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;
                                            let offset = Vec2::new(angle.cos() * dist, angle.sin() * dist);
                                            let fallback_target = center + offset;

                                            let nearby_souls = soul_grid.get_nearby_in_radius(fallback_target, MIN_SEPARATION);
                                            let position_occupied = nearby_souls.iter().any(|&other| other != entity);

                                            if !position_occupied {
                                                let target_grid = WorldMap::world_to_grid(fallback_target);
                                                if world_map.is_walkable(target_grid.0, target_grid.1) {
                                                    dest.0 = fallback_target;
                                                    path.waypoints.clear();
                                                    path.current_index = 0;
                                                    fallback_found = true;
                                                    break;
                                                }
                                            }
                                        }
                                        // 5回試してもダメなら目的地を変更しない（separation_systemに任せる）
                                        if !fallback_found {
                                            warn!("IDLE: Soul {:?} Wandering fallback failed after 5 attempts, keeping current destination", entity);
                                        }
                                    }
                                }
                            }
                            GatheringBehavior::Sleeping | GatheringBehavior::Standing | GatheringBehavior::Dancing => {
                                // これらの状態では移動しないが、中心に近すぎる場合は離れる
                                if dist_from_center < TILE_SIZE * GATHERING_KEEP_DISTANCE_MIN {
                                    let away = (current_pos - center).normalize_or_zero();
                                    let target = center + away * TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN;

                                    // 中心に近すぎる時の移動先もoverlapチェック
                                    const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;
                                    let nearby_souls = soul_grid.get_nearby_in_radius(target, MIN_SEPARATION);
                                    let position_occupied = nearby_souls.iter().any(|&other| other != entity);

                                    if !position_occupied {
                                        let target_grid = WorldMap::world_to_grid(target);
                                        if world_map.is_walkable(target_grid.0, target_grid.1) {
                                            dest.0 = target;
                                            path.waypoints.clear();
                                            path.current_index = 0;
                                        }
                                    }
                                    // overlap回避できない場合は目的地を変更しない（separation_systemに任せる）
                                } else {
                                    // 重なり回避移動中かチェック（separation_systemによる移動を妨げない）
                                    const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;
                                    let nearby_souls = soul_grid.get_nearby_in_radius(current_pos, MIN_SEPARATION);
                                    let has_overlap = nearby_souls.iter().any(|&other| other != entity);
                                    let dist_to_dest = (dest.0 - current_pos).length();

                                    // 重なりがある、または目的地まで遠い場合は、移動を継続（waypointsをクリアしない）
                                    if !has_overlap && dist_to_dest < TILE_SIZE * 0.5 {
                                        // 重なりなし & 目的地に近い → 移動停止
                                        path.waypoints.clear();
                                        path.current_index = 0;
                                    }
                                    // それ以外の場合は、waypoints はそのまま（separation による移動を継続）
                                }
                            }
                        }
                    }
                } else {
                    // 中心が見つからない場合は Wandering へ
                    idle.behavior = IdleBehavior::Wandering;
                }
            }
            IdleBehavior::Sitting | IdleBehavior::Sleeping => {}
            IdleBehavior::Escaping => {
                // 逃走中は escaping_decision_system で処理されるため、
                // ここでは何もしない（continueされるはず）
            }
        }
    }
}
