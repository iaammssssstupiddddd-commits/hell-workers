use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, Destination, GatheringBehavior, IdleBehavior, IdleState, Path,
};
use crate::systems::soul_ai::gathering::{GatheringSpot, ParticipatingIn};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use rand::Rng;

// ===== 集会関連の定数 =====
/// 集会エリアに「到着した」とみなす半径
const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE;
/// 重なり回避の最小間隔
const GATHERING_MIN_SEPARATION: f32 = TILE_SIZE * 1.2;

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
) {
    let dt = time.delta_secs();

    for (
        entity,
        transform,
        mut idle,
        mut dest,
        soul,
        mut path,
        task,
        participating_in,
        under_command_opt,
    ) in query.iter_mut()
    {
        // 参加中の集会スポットの座標とEntityを取得、または最寄りのスポットを探す
        let (gathering_center, target_spot_entity): (Option<Vec2>, Option<Entity>) =
            if let Some(p) = participating_in {
                let center = q_spots.get(p.0).ok().map(|(_, s)| s.center);
                (center, Some(p.0))
            } else {
                // 最寄りのスポットを探す
                let pos = transform.translation.truncate();
                let nearest = q_spots
                    .iter()
                    .filter(|(_, s)| s.participants < s.max_capacity)
                    .min_by(|(_, a), (_, b)| {
                        a.center
                            .distance_squared(pos)
                            .partial_cmp(&b.center.distance_squared(pos))
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
            idle.total_idle_time = 0.0;
            continue;
        }

        if !matches!(*task, AssignedTask::None) {
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

/// 怠惰行動のビジュアルフィードバック
pub fn idle_visual_system(
    q_spots: Query<&GatheringSpot>,
    mut query: Query<(
        &mut Transform,
        &mut Sprite,
        &IdleState,
        &DamnedSoul,
        &AssignedTask,
        Option<&ParticipatingIn>,
    )>,
) {
    for (mut transform, mut sprite, idle, soul, task, participating_in) in query.iter_mut() {
        if !matches!(task, AssignedTask::None) {
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
            sprite.color = Color::WHITE;
            continue;
        }

        match idle.behavior {
            IdleBehavior::Sleeping => {
                transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                sprite.color = Color::srgba(0.6, 0.6, 0.7, 1.0);
            }
            IdleBehavior::Sitting => {
                transform.rotation = Quat::IDENTITY;
                transform.scale.y = 0.8;
                sprite.color = Color::srgba(0.8, 0.8, 0.8, 1.0);
            }
            IdleBehavior::Wandering => {
                transform.rotation = Quat::IDENTITY;
                sprite.color = Color::WHITE;
            }
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                let gathering_center = if let Some(p) = participating_in {
                    q_spots.get(p.0).ok().map(|s| s.center)
                } else {
                    let pos = transform.translation.truncate();
                    q_spots
                        .iter()
                        .min_by(|a, b| {
                            a.center
                                .distance_squared(pos)
                                .partial_cmp(&b.center.distance_squared(pos))
                                .unwrap()
                        })
                        .map(|s| s.center)
                };

                if let Some(center) = gathering_center {
                    let current_pos = transform.translation.truncate();
                    let dist_from_center = (center - current_pos).length();
                    let has_arrived = dist_from_center <= GATHERING_ARRIVAL_RADIUS;

                    if !has_arrived {
                        transform.rotation = Quat::IDENTITY;
                        transform.scale = Vec3::ONE;

                        if idle.behavior == IdleBehavior::ExhaustedGathering {
                            sprite.color = Color::srgba(0.7, 0.6, 0.8, 0.9);
                        } else {
                            sprite.color = Color::srgba(0.85, 0.75, 1.0, 0.85);
                        }
                    } else {
                        sprite.color = Color::srgba(0.8, 0.7, 1.0, 0.7);

                        match idle.gathering_behavior {
                            GatheringBehavior::Wandering => {
                                transform.rotation = Quat::IDENTITY;
                                let pulse_speed = 0.5;
                                let scale_offset =
                                    (idle.total_idle_time * pulse_speed).sin() * 0.03 + 1.0;
                                transform.scale = Vec3::splat(scale_offset);
                            }
                            GatheringBehavior::Sleeping => {
                                transform.rotation =
                                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                                sprite.color = Color::srgba(0.6, 0.5, 0.8, 0.6);
                                let breath = (idle.total_idle_time * 0.3).sin() * 0.02 + 0.95;
                                transform.scale = Vec3::splat(breath);
                            }
                            GatheringBehavior::Standing => {
                                transform.rotation = Quat::IDENTITY;
                                let breath = (idle.total_idle_time * 0.2).sin() * 0.01 + 1.0;
                                transform.scale = Vec3::splat(breath);
                            }
                            GatheringBehavior::Dancing => {
                                let sway_angle = (idle.total_idle_time * 3.0).sin() * 0.15;
                                transform.rotation = Quat::from_rotation_z(sway_angle);
                                let bounce = (idle.total_idle_time * 4.0).sin() * 0.05 + 1.0;
                                transform.scale = Vec3::new(1.0, bounce, 1.0);
                                sprite.color = Color::srgba(1.0, 0.8, 1.0, 0.8);
                            }
                        }
                    }
                } else {
                    transform.rotation = Quat::IDENTITY;
                    sprite.color = Color::WHITE;
                }
            }
        }

        if soul.motivation > 0.5 {
            sprite.color = Color::srgb(1.0, 1.0, 0.8);
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
        }
    }
}

pub fn gathering_separation_system(
    world_map: Res<WorldMap>,
    q_spots: Query<&GatheringSpot>,
    mut query: Query<(
        Entity,
        &Transform,
        &mut Destination,
        &mut IdleState,
        &Path,
        &AssignedTask,
        &ParticipatingIn,
    )>,
) {
    let mut gathering_positions: Vec<(Entity, Vec2)> = Vec::new();
    for (entity, transform, _, _, _, _, _) in query.iter() {
        gathering_positions.push((entity, transform.translation.truncate()));
    }

    for (entity, transform, mut dest, mut idle, soul_path, soul_task, participating_in) in
        query.iter_mut()
    {
        if !idle.needs_separation {
            continue;
        }

        // タスク実行中は重なり回避しない
        if !matches!(soul_task, AssignedTask::None) {
            idle.needs_separation = false;
            continue;
        }

        // 集会中のうろうろ状態（ターゲットに向かって歩いている最中）は回避イベントを発生させない
        if idle.gathering_behavior == GatheringBehavior::Wandering {
            idle.needs_separation = false;
            continue;
        }

        if let Ok(spot) = q_spots.get(participating_in.0) {
            let center = spot.center;
            let current_pos = transform.translation.truncate();

            // 目的地にまだ到達していない、またはパス移動中の場合は回避をスキップ
            if !soul_path.waypoints.is_empty()
                && soul_path.current_index < soul_path.waypoints.len()
            {
                continue;
            }

            let mut is_overlapping =
                (center - current_pos).length() < TILE_SIZE * GATHERING_KEEP_DISTANCE_MIN;

            if !is_overlapping {
                for (other_entity, other_pos) in &gathering_positions {
                    if *other_entity == entity {
                        continue;
                    }
                    let dist = (current_pos - *other_pos).length();
                    if dist < GATHERING_MIN_SEPARATION {
                        is_overlapping = true;
                        break;
                    }
                }
            }

            if is_overlapping {
                let mut rng = rand::thread_rng();
                for _ in 0..10 {
                    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                    let dist: f32 = rng.gen_range(
                        TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN
                            ..TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX,
                    );
                    let offset = Vec2::new(angle.cos() * dist, angle.sin() * dist);
                    let new_pos = center + offset;

                    let mut valid = true;
                    for (other_entity, other_pos) in &gathering_positions {
                        if *other_entity == entity {
                            continue;
                        }
                        if (new_pos - *other_pos).length() < GATHERING_MIN_SEPARATION {
                            valid = false;
                            break;
                        }
                    }

                    if valid {
                        let target_grid = WorldMap::world_to_grid(new_pos);
                        if world_map.is_walkable(target_grid.0, target_grid.1) {
                            dest.0 = new_pos;
                            break;
                        }
                    }
                }
            }
        }

        idle.needs_separation = false;
    }
}
