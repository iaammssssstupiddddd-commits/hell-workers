use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, Destination, GatheringBehavior, IdleBehavior, IdleState, Path,
};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::world::map::{GatheringArea, WorldMap};
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
    world_map: Res<WorldMap>,
    gathering_area: Res<GatheringArea>,
    mut query: Query<(
        &Transform,
        &mut IdleState,
        &mut Destination,
        &DamnedSoul,
        &mut Path,
        &mut AssignedTask,
        Option<&crate::entities::familiar::UnderCommand>,
    )>,
) {
    let dt = time.delta_secs();

    for (transform, mut idle, mut dest, soul, mut path, task, under_command_opt) in query.iter_mut()
    {
        // 疲労による強制集会（ExhaustedGathering）状態の場合は他の処理をスキップ
        if idle.behavior == IdleBehavior::ExhaustedGathering {
            let current_pos = transform.translation.truncate();
            let center = gathering_area.0;
            let dist_from_center = (center - current_pos).length();
            let has_arrived = dist_from_center <= GATHERING_ARRIVAL_RADIUS;

            if has_arrived {
                info!("IDLE: Soul transitioned from ExhaustedGathering to Gathering");
                idle.behavior = IdleBehavior::Gathering;
            } else {
                if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                    dest.0 = center;
                    path.waypoints.clear();
                }
                continue;
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
                let current_pos = transform.translation.truncate();
                let center = gathering_area.0;
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
                    if idle.behavior == IdleBehavior::ExhaustedGathering {
                        idle.behavior = IdleBehavior::Gathering;
                    }

                    match idle.gathering_behavior {
                        GatheringBehavior::Wandering => {
                            let path_complete = path.waypoints.is_empty()
                                || path.current_index >= path.waypoints.len();
                            if path_complete && idle.idle_timer >= idle.behavior_duration * 0.8 {
                                let new_target = random_position_around(
                                    center,
                                    TILE_SIZE * 0.5,
                                    TILE_SIZE * 1.5,
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
            }
            IdleBehavior::Sitting | IdleBehavior::Sleeping => {}
        }
    }
}

/// 怠惰行動のビジュアルフィードバック
pub fn idle_visual_system(
    gathering_area: Res<GatheringArea>,
    mut query: Query<(
        &mut Transform,
        &mut Sprite,
        &IdleState,
        &DamnedSoul,
        &AssignedTask,
    )>,
) {
    for (mut transform, mut sprite, idle, soul, task) in query.iter_mut() {
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
                let current_pos = transform.translation.truncate();
                let center = gathering_area.0;
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
                            transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
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
            }
        }

        if soul.motivation > 0.5 {
            sprite.color = Color::srgb(1.0, 1.0, 0.8);
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
        }
    }
}

/// 集会エリアでの魂の重なり回避システム
pub fn gathering_separation_system(
    gathering_area: Res<GatheringArea>,
    world_map: Res<WorldMap>,
    mut query: Query<(
        Entity,
        &Transform,
        &mut Destination,
        &mut IdleState,
        &Path,
        &AssignedTask,
    )>,
) {
    let gathering_positions: Vec<(Entity, Vec2)> = query
        .iter()
        .filter(|(_, _, _, idle, _, task)| {
            matches!(task, AssignedTask::None)
                && (idle.behavior == IdleBehavior::Gathering
                    || idle.behavior == IdleBehavior::ExhaustedGathering)
                && idle.gathering_behavior != GatheringBehavior::Wandering
        })
        .map(|(entity, transform, _, _, _, _)| (entity, transform.translation.truncate()))
        .collect();

    for (entity, transform, mut dest, mut idle, path, task) in query.iter_mut() {
        if !idle.needs_separation {
            continue;
        }

        if !matches!(task, AssignedTask::None) {
            idle.needs_separation = false;
            continue;
        }
        if idle.behavior != IdleBehavior::Gathering
            && idle.behavior != IdleBehavior::ExhaustedGathering
        {
            idle.needs_separation = false;
            continue;
        }
        if idle.gathering_behavior == GatheringBehavior::Wandering {
            idle.needs_separation = false;
            continue;
        }

        let current_pos = transform.translation.truncate();
        let center = gathering_area.0;
        let dist_from_center = (center - current_pos).length();

        if dist_from_center > GATHERING_ARRIVAL_RADIUS {
            continue;
        }

        if !path.waypoints.is_empty() && path.current_index < path.waypoints.len() {
            continue;
        }

        let mut is_overlapping = false;
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

        if is_overlapping {
            let mut rng = rand::thread_rng();
            for _ in 0..10 {
                let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                let dist: f32 = rng.gen_range(TILE_SIZE..TILE_SIZE * 2.5);
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

        idle.needs_separation = false;
    }
}
