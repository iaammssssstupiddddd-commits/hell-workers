use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, Destination, GatheringBehavior, IdleBehavior, IdleState, Path,
};
use crate::entities::familiar::UnderCommand;
use crate::systems::soul_ai::gathering::{GATHERING_LEAVE_RADIUS, GatheringSpot, ParticipatingIn};
use crate::systems::soul_ai::task_execution::AssignedTask;
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

/// 怠惰行動のAIシステム
/// やる気が低い人間は怠惰な行動をする
/// タスクがある人間は怠惰行動をしない
pub fn idle_behavior_system(
    time: Res<Time>,
    mut commands: Commands,
    world_map: Res<WorldMap>,
    q_spots: Query<(Entity, &GatheringSpot)>,
    mut query: Query<(
        Entity,
        &Transform,
        &mut IdleState,
        &mut Destination,
        &DamnedSoul,
        &mut Path,
        &mut AssignedTask,
        Option<&ParticipatingIn>,
        Option<&crate::entities::familiar::UnderCommand>,
    )>,
    spot_grid: Res<GatheringSpotSpatialGrid>,
    _q_targets: Query<(
        &Transform,
        Option<&crate::systems::jobs::Tree>,
        Option<&crate::systems::jobs::Rock>,
        Option<&crate::systems::logistics::ResourceItem>,
        Option<&crate::systems::jobs::Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    _q_designations: Query<(
        Entity,
        &Transform,
        &crate::systems::jobs::Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&crate::systems::jobs::TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
) {
    let dt = time.delta_secs();

    for (entity, transform, idle, dest, soul, path, task, participating_in, under_command_opt) in
        query.iter_mut()
    {
        #[allow(clippy::type_complexity)]
        let (
            entity,
            transform,
            mut idle,
            mut dest,
            soul,
            mut path,
            task,
            participating_in,
            under_command_opt,
        ): (
            Entity,
            &Transform,
            Mut<IdleState>,
            Mut<Destination>,
            &DamnedSoul,
            Mut<Path>,
            Mut<AssignedTask>,
            Option<&ParticipatingIn>,
            Option<&UnderCommand>,
        ) = (
            entity,
            transform,
            idle,
            dest,
            soul,
            path,
            task,
            participating_in,
            under_command_opt,
        );
        // 参加中の集会スポットの座標とEntityを取得、または最寄りのスポットを探す
        let (gathering_center, target_spot_entity): (Option<Vec2>, Option<Entity>) =
            if let Some(p) = participating_in {
                let center = q_spots.get(p.0).ok().map(|(_, s)| s.center);
                (center, Some(p.0))
            } else {
                // 最寄りのスポットを空間グリッドで効率的に探す
                let pos = transform.translation.truncate();
                // 離脱半径の2倍程度の範囲で探す (あまり遠くの集会所には行かない)
                let spot_entities =
                    spot_grid.get_nearby_in_radius(pos, GATHERING_LEAVE_RADIUS * 2.0);

                let nearest = spot_entities
                    .iter()
                    .filter_map(|&e| q_spots.get(e).ok())
                    .filter(|item| item.1.participants < item.1.max_capacity)
                    .min_by(|a, b| {
                        a.1.center
                            .distance_squared(pos)
                            .partial_cmp(&b.1.center.distance_squared(pos))
                            .unwrap()
                    });
                match nearest {
                    Some((e, s)) => (Some(s.center), Some(e)),
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
                    info!("IDLE: Soul transitioned from ExhaustedGathering to Gathering");
                    idle.behavior = IdleBehavior::Gathering;
                    // ParticipatingIn を追加
                    if participating_in.is_none() {
                        if let Some(spot_entity) = target_spot_entity {
                            commands.entity(entity).insert(ParticipatingIn(spot_entity));
                            commands.trigger(crate::events::OnGatheringParticipated {
                                entity,
                                spot_entity,
                            });
                        }
                    }
                    commands.trigger(crate::events::OnGatheringJoined { entity });
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

        if under_command_opt.is_some() {
            // 使役されたら集会から抜ける
            if let Some(p) = participating_in {
                commands.entity(entity).remove::<ParticipatingIn>();
                commands.trigger(crate::events::OnGatheringLeft {
                    entity,
                    spot_entity: p.0,
                });
            }
            idle.total_idle_time = 0.0;
            continue;
        }

        if !matches!(&*task, AssignedTask::None) {
            // タスクが割り当てられたら集会から抜ける
            if let Some(p) = participating_in {
                commands.entity(entity).remove::<ParticipatingIn>();
                commands.trigger(crate::events::OnGatheringLeft {
                    entity,
                    spot_entity: p.0,
                });
            }
            idle.total_idle_time = 0.0;
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
                        // 到着時にParticipatingInを追加
                        if participating_in.is_none() {
                            if let Some(spot_entity) = target_spot_entity {
                                commands.entity(entity).insert(ParticipatingIn(spot_entity));
                                commands.trigger(crate::events::OnGatheringParticipated {
                                    entity,
                                    spot_entity,
                                });
                            }
                        }

                        if idle.behavior == IdleBehavior::ExhaustedGathering {
                            idle.behavior = IdleBehavior::Gathering;
                        }

                        match idle.gathering_behavior {
                            GatheringBehavior::Wandering => {
                                let path_complete = path.waypoints.is_empty()
                                    || path.current_index >= path.waypoints.len();
                                if path_complete && idle.idle_timer >= idle.behavior_duration * 0.8
                                {
                                    let new_target = random_position_around(
                                        center,
                                        TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN,
                                        TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX,
                                    );
                                    let target_grid = WorldMap::world_to_grid(new_target);
                                    if world_map.is_walkable(target_grid.0, target_grid.1) {
                                        dest.0 = new_target;
                                    } else {
                                        dest.0 = center;
                                    }
                                    idle.idle_timer = 0.0;
                                    let mut rng = rand::thread_rng();
                                    idle.behavior_duration = rng.gen_range(2.0..3.0);
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    // 中心が見つからない場合は Wandering へ
                    idle.behavior = IdleBehavior::Wandering;
                }
            }
            IdleBehavior::Sitting | IdleBehavior::Sleeping => {}
        }
    }
}
