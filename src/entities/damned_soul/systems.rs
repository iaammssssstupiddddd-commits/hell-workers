use super::*;
use crate::assets::GameAssets;
use crate::constants::*;
use crate::events::{
    OnExhausted, OnSoulRecruited, OnStressBreakdown, OnTaskAssigned, OnTaskCompleted,
};
use crate::relationships::Holding;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::work::unassign_task;
use crate::world::map::WorldMap;
use crate::world::pathfinding::find_path;
use rand::Rng;

/// 人間をスポーンする
pub fn spawn_damned_souls(mut spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    let mut rng = rand::thread_rng();
    for _ in 0..10 {
        let x = rng.gen_range(-100.0..100.0);
        let y = rng.gen_range(-100.0..100.0);
        spawn_events.write(DamnedSoulSpawnEvent {
            position: Vec2::new(x, y),
        });
    }
}

/// スポーンイベントを処理するシステム
pub fn soul_spawning_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<DamnedSoulSpawnEvent>,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    for event in spawn_events.read() {
        spawn_damned_soul_at(&mut commands, &game_assets, &world_map, event.position);
    }
}

/// 指定座標にソウルをスポーンする（内部用ヘルパー）
pub fn spawn_damned_soul_at(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    world_map: &Res<WorldMap>,
    pos: Vec2,
) {
    let spawn_grid = WorldMap::world_to_grid(pos);
    let mut actual_grid = spawn_grid;
    'search: for dx in -5..=5 {
        for dy in -5..=5 {
            let test = (spawn_grid.0 + dx, spawn_grid.1 + dy);
            if world_map.is_walkable(test.0, test.1) {
                actual_grid = test;
                break 'search;
            }
        }
    }
    let actual_pos = WorldMap::grid_to_world(actual_grid.0, actual_grid.1);

    let identity = SoulIdentity::random();
    let soul_name = identity.name.clone();
    let gender = identity.gender;

    let sprite_color = match gender {
        Gender::Male => Color::srgb(0.9, 0.9, 1.0), // わずかに青み
        Gender::Female => Color::srgb(1.0, 0.9, 0.9), // わずかに赤み
    };

    commands
        .spawn((
            DamnedSoul::default(),
            Name::new(format!("Soul: {}", soul_name)),
            identity,
            IdleState::default(),
            AssignedTask::default(),
            Sprite {
                image: game_assets.colonist.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
                color: sprite_color,
                ..default()
            },
            Transform::from_xyz(actual_pos.x, actual_pos.y, Z_CHARACTER),
            Destination(actual_pos),
            Path::default(),
            AnimationState::default(),
            crate::systems::visual::speech::components::SoulEmotionState::default(),
            crate::systems::visual::speech::conversation::components::ConversationInitiator {
                timer: Timer::from_seconds(CONVERSATION_CHECK_INTERVAL, TimerMode::Repeating),
            },
        ))
        .observe(on_task_assigned)
        .observe(on_task_completed)
        .observe(on_soul_recruited)
        .observe(on_stress_breakdown)
        .observe(on_exhausted)
        .observe(crate::systems::visual::speech::observers::on_released_from_service)
        .observe(crate::systems::visual::speech::observers::on_gathering_joined)
        .observe(crate::systems::visual::speech::observers::on_task_abandoned);

    info!("SPAWN: {} ({:?}) at {:?}", soul_name, gender, actual_pos);
}

/// 経路探索システム
pub fn pathfinding_system(
    world_map: Res<WorldMap>,
    mut query: Query<
        (Entity, &Transform, &Destination, &mut Path),
        (Changed<Destination>, With<DamnedSoul>),
    >,
) {
    for (entity, transform, destination, mut path) in query.iter_mut() {
        let current_pos = transform.translation.truncate();
        let start_grid = WorldMap::world_to_grid(current_pos);
        let goal_grid = WorldMap::world_to_grid(destination.0);

        if let Some(last) = path.waypoints.last() {
            if last.distance_squared(destination.0) < 1.0 {
                continue;
            }
        }

        if start_grid == goal_grid {
            path.waypoints = vec![destination.0];
            path.current_index = 0;
            continue;
        }

        if let Some(grid_path) = find_path(&*world_map, start_grid, goal_grid) {
            path.waypoints = grid_path
                .iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
            path.current_index = 0;
            debug!("PATH: Soul {:?} found new path", entity);
        } else {
            debug!("PATH: Soul {:?} failed to find path", entity);
            path.waypoints.clear();
        }
    }
}

/// 移動システム
pub fn soul_movement(
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut Transform,
        &mut Path,
        &mut AnimationState,
        &DamnedSoul,
        &IdleState,
        Option<&StressBreakdown>,
    )>,
) {
    for (_entity, mut transform, mut path, mut anim, soul, idle, breakdown_opt) in query.iter_mut()
    {
        if let Some(breakdown) = breakdown_opt {
            if breakdown.is_frozen {
                anim.is_moving = false;
                continue;
            }
        }

        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let to_target = target - current_pos;
            let distance = to_target.length();

            if distance > 2.0 {
                let base_speed = SOUL_SPEED_BASE;
                let motivation_bonus = soul.motivation * SOUL_SPEED_MOTIVATION_BONUS;
                let laziness_penalty = soul.laziness * SOUL_SPEED_LAZINESS_PENALTY;
                let mut speed =
                    (base_speed + motivation_bonus - laziness_penalty).max(SOUL_SPEED_MIN);

                if idle.behavior == IdleBehavior::ExhaustedGathering {
                    speed *= SOUL_SPEED_EXHAUSTED_MULTIPLIER;
                }

                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;
                transform.translation += velocity.extend(0.0);

                anim.is_moving = true;
                if direction.x.abs() > 0.1 {
                    anim.facing_right = direction.x > 0.0;
                }
            } else {
                path.current_index += 1;
            }
        } else {
            anim.is_moving = false;
        }
    }
}

/// アニメーションシステム
pub fn animation_system(
    time: Res<Time>,
    mut query: Query<(
        &mut Transform,
        &mut Sprite,
        &mut AnimationState,
        &DamnedSoul,
    )>,
) {
    for (mut transform, mut sprite, mut anim, soul) in query.iter_mut() {
        sprite.flip_x = !anim.facing_right;

        if anim.is_moving {
            anim.bob_timer += time.delta_secs() * ANIM_BOB_SPEED;
            let bob = (anim.bob_timer.sin() * ANIM_BOB_AMPLITUDE) + 1.0;
            transform.scale = Vec3::new(1.0, bob, 1.0);
        } else {
            let breath_speed = ANIM_BREATH_SPEED_BASE - soul.laziness;
            anim.bob_timer += time.delta_secs() * breath_speed;
            let breath = (anim.bob_timer.sin() * ANIM_BREATH_AMPLITUDE) + 1.0;
            transform.scale = Vec3::splat(breath);
        }
    }
}

// ============================================================
// Observer ハンドラ
// ============================================================

fn on_task_assigned(on: On<OnTaskAssigned>, _q_souls: Query<&mut DamnedSoul>) {
    let soul_entity = on.entity;
    let event = on.event();
    info!(
        "OBSERVER: Soul {:?} assigned to task {:?} ({:?})",
        soul_entity, event.task_entity, event.work_type
    );
}

fn on_task_completed(on: On<OnTaskCompleted>, _q_souls: Query<&mut DamnedSoul>) {
    let soul_entity = on.entity;
    let event = on.event();
    info!(
        "OBSERVER: Soul {:?} completed task {:?} ({:?})",
        soul_entity, event.task_entity, event.work_type
    );
}

fn on_soul_recruited(on: On<OnSoulRecruited>, _q_souls: Query<&mut DamnedSoul>) {
    let soul_entity = on.entity;
    let event = on.event();
    info!(
        "OBSERVER: Soul {:?} recruited by Familiar {:?}",
        soul_entity, event.familiar_entity
    );
}

fn on_stress_breakdown(
    on: On<OnStressBreakdown>,
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut DamnedSoul,
        &mut AssignedTask,
        &mut Path,
        Option<&Holding>,
        Option<&crate::entities::familiar::UnderCommand>,
    )>,
    q_designations: Query<(
        Entity,
        &Transform,
        &crate::systems::jobs::Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&crate::systems::jobs::TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    mut haul_cache: ResMut<crate::systems::familiar_ai::haul_cache::HaulReservationCache>,
    mut ev_created: MessageWriter<crate::systems::jobs::DesignationCreatedEvent>,
) {
    let soul_entity = on.entity;
    if let Ok((entity, transform, mut _soul, mut task, mut path, holding_opt, under_command)) =
        q_souls.get_mut(soul_entity)
    {
        info!("OBSERVER: Soul {:?} had a stress breakdown!", entity);

        commands
            .entity(entity)
            .insert(StressBreakdown { is_frozen: true });

        if !matches!(*task, AssignedTask::None) {
            unassign_task(
                &mut commands,
                entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                holding_opt,
                &q_designations,
                &mut *haul_cache,
                Some(&mut ev_created),
                true,
            );
        }

        if under_command.is_some() {
            commands
                .entity(entity)
                .remove::<crate::entities::familiar::UnderCommand>();
        }
    }
}

fn on_exhausted(
    on: On<OnExhausted>,
    mut commands: Commands,
    q_spots: Query<&crate::systems::soul_ai::gathering::GatheringSpot>,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut IdleState,
        &mut AssignedTask,
        &mut Path,
        &mut Destination,
        Option<&Holding>,
        Option<&crate::entities::familiar::UnderCommand>,
    )>,
    q_designations: Query<(
        Entity,
        &Transform,
        &crate::systems::jobs::Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&crate::systems::jobs::TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    mut haul_cache: ResMut<crate::systems::familiar_ai::haul_cache::HaulReservationCache>,
    mut ev_created: MessageWriter<crate::systems::jobs::DesignationCreatedEvent>,
) {
    let soul_entity = on.entity;
    if let Ok((
        entity,
        transform,
        mut idle,
        mut task,
        mut path,
        mut dest,
        holding_opt,
        under_command_opt,
    )) = q_souls.get_mut(soul_entity)
    {
        info!(
            "OBSERVER: Soul {:?} is exhausted, heading to gathering area",
            entity
        );

        if under_command_opt.is_some() {
            commands
                .entity(entity)
                .remove::<crate::entities::familiar::UnderCommand>();
        }

        if !matches!(*task, AssignedTask::None) {
            unassign_task(
                &mut commands,
                entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                holding_opt,
                &q_designations,
                &mut *haul_cache,
                Some(&mut ev_created),
                true,
            );
        }

        if idle.behavior != IdleBehavior::ExhaustedGathering {
            if idle.behavior != IdleBehavior::Gathering {
                let mut rng = rand::thread_rng();
                idle.gathering_behavior = match rng.gen_range(0..4) {
                    0 => GatheringBehavior::Wandering,
                    1 => GatheringBehavior::Sleeping,
                    2 => GatheringBehavior::Standing,
                    _ => GatheringBehavior::Dancing,
                };
                idle.gathering_behavior_timer = 0.0;
                idle.gathering_behavior_duration = rng.gen_range(60.0..90.0);
                idle.needs_separation = true;
            }
            idle.behavior = IdleBehavior::ExhaustedGathering;
            idle.idle_timer = 0.0;
            let mut rng = rand::thread_rng();
            idle.behavior_duration = rng.gen_range(2.0..4.0);
        }

        // 最寄りの集会所を探す
        let current_pos = transform.translation.truncate();
        let gathering_center = q_spots
            .iter()
            .min_by(|a, b| {
                a.center
                    .distance_squared(current_pos)
                    .partial_cmp(&b.center.distance_squared(current_pos))
                    .unwrap()
            })
            .map(|s| s.center);

        if let Some(center) = gathering_center {
            let dist_from_center = (center - current_pos).length();

            if dist_from_center > TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE {
                dest.0 = center;
                path.waypoints.clear();
                path.current_index = 0;
            }
        }
    }
}
