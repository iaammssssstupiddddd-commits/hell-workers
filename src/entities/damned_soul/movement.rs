//! ソウルの移動・パス追従・アニメーションシステム

use super::*;
use crate::assets::GameAssets;
use crate::constants::*;
use crate::events::{OnExhausted, OnGatheringParticipated};
use crate::relationships::PushingWheelbarrow;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::gathering::{GatheringObjectType, GatheringSpot};
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::systems::visual::speech::conversation::events::{
    ConversationCompleted, ConversationTone, ConversationToneTriggered,
};
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};

const EXPRESSION_PRIORITY_CONVERSATION_TONE: u8 = 20;
const EXPRESSION_PRIORITY_CONVERSATION_COMPLETED: u8 = 10;
const EXPRESSION_PRIORITY_GATHERING_OBJECT: u8 = 15;
const EXPRESSION_PRIORITY_EXHAUSTED: u8 = 30;

fn tone_to_expression_kind(tone: ConversationTone) -> Option<ConversationExpressionKind> {
    match tone {
        ConversationTone::Positive => Some(ConversationExpressionKind::Positive),
        ConversationTone::Negative => Some(ConversationExpressionKind::Negative),
        ConversationTone::Neutral => None,
    }
}

fn lock_seconds_for_tone_event(tone: ConversationTone) -> Option<f32> {
    match tone {
        ConversationTone::Positive => Some(SOUL_EVENT_LOCK_TONE_POSITIVE),
        ConversationTone::Negative => Some(SOUL_EVENT_LOCK_TONE_NEGATIVE),
        ConversationTone::Neutral => None,
    }
}

fn lock_seconds_for_completed_event(tone: ConversationTone) -> Option<f32> {
    match tone {
        ConversationTone::Positive => Some(SOUL_EVENT_LOCK_COMPLETED_POSITIVE),
        ConversationTone::Negative => Some(SOUL_EVENT_LOCK_COMPLETED_NEGATIVE),
        ConversationTone::Neutral => None,
    }
}

fn apply_expression_lock(
    commands: &mut Commands,
    entity: Entity,
    kind: ConversationExpressionKind,
    lock_secs: f32,
    priority: u8,
    q_expression: &mut Query<&mut ConversationExpression, With<DamnedSoul>>,
) {
    if let Ok(mut expression) = q_expression.get_mut(entity) {
        if priority > expression.priority {
            expression.kind = kind;
            expression.priority = priority;
            expression.remaining_secs = lock_secs;
        } else if priority == expression.priority {
            if expression.kind == kind {
                expression.remaining_secs = expression.remaining_secs.max(lock_secs);
            } else {
                expression.kind = kind;
                expression.remaining_secs = lock_secs;
            }
        }
        return;
    }

    commands.entity(entity).insert(ConversationExpression {
        kind,
        priority,
        remaining_secs: lock_secs,
    });
}

fn select_soul_image<'a>(
    game_assets: &'a GameAssets,
    idle: &IdleState,
    breakdown_opt: Option<&StressBreakdown>,
    expression_opt: Option<&ConversationExpression>,
) -> &'a Handle<Image> {
    if let Some(breakdown) = breakdown_opt {
        if breakdown.is_frozen {
            return &game_assets.soul_stress_breakdown;
        }
        return &game_assets.soul_stress;
    }

    if let Some(expression) = expression_opt {
        match expression.kind {
            ConversationExpressionKind::Positive => return &game_assets.soul_lough,
            ConversationExpressionKind::Negative => return &game_assets.soul_stress,
            ConversationExpressionKind::Exhausted => return &game_assets.soul_exhausted,
            ConversationExpressionKind::GatheringWine => return &game_assets.soul_wine,
            ConversationExpressionKind::GatheringTrump => return &game_assets.soul_trump,
        }
    }

    match idle.behavior {
        IdleBehavior::Sleeping => &game_assets.soul_sleep,
        IdleBehavior::ExhaustedGathering => &game_assets.soul_exhausted,
        IdleBehavior::Escaping => &game_assets.soul,
        IdleBehavior::Gathering => match idle.gathering_behavior {
            GatheringBehavior::Sleeping => &game_assets.soul_sleep,
            GatheringBehavior::Wandering
            | GatheringBehavior::Standing
            | GatheringBehavior::Dancing => &game_assets.soul,
        },
        IdleBehavior::Wandering | IdleBehavior::Sitting => &game_assets.soul,
    }
}

pub fn apply_conversation_expression_event_system(
    mut commands: Commands,
    q_souls: Query<(), With<DamnedSoul>>,
    q_spots: Query<&GatheringSpot>,
    mut q_expression: Query<&mut ConversationExpression, With<DamnedSoul>>,
    mut ev_exhausted_reader: MessageReader<OnExhausted>,
    mut ev_gathering_participated_reader: MessageReader<OnGatheringParticipated>,
    mut ev_tone_reader: MessageReader<ConversationToneTriggered>,
    mut ev_reader: MessageReader<ConversationCompleted>,
) {
    for event in ev_exhausted_reader.read() {
        if q_souls.get(event.entity).is_err() {
            continue;
        }
        apply_expression_lock(
            &mut commands,
            event.entity,
            ConversationExpressionKind::Exhausted,
            SOUL_EVENT_LOCK_EXHAUSTED,
            EXPRESSION_PRIORITY_EXHAUSTED,
            &mut q_expression,
        );
    }

    for event in ev_gathering_participated_reader.read() {
        if q_souls.get(event.entity).is_err() {
            continue;
        }
        let Ok(spot) = q_spots.get(event.spot_entity) else {
            continue;
        };
        let kind = match spot.object_type {
            GatheringObjectType::Barrel => Some(ConversationExpressionKind::GatheringWine),
            GatheringObjectType::CardTable => Some(ConversationExpressionKind::GatheringTrump),
            GatheringObjectType::Nothing | GatheringObjectType::Campfire => None,
        };
        let Some(kind) = kind else {
            continue;
        };

        apply_expression_lock(
            &mut commands,
            event.entity,
            kind,
            SOUL_EVENT_LOCK_GATHERING_OBJECT,
            EXPRESSION_PRIORITY_GATHERING_OBJECT,
            &mut q_expression,
        );
    }

    for event in ev_tone_reader.read() {
        if q_souls.get(event.speaker).is_err() {
            continue;
        }
        let Some(kind) = tone_to_expression_kind(event.tone) else {
            continue;
        };
        let Some(lock_secs) = lock_seconds_for_tone_event(event.tone) else {
            continue;
        };

        apply_expression_lock(
            &mut commands,
            event.speaker,
            kind,
            lock_secs,
            EXPRESSION_PRIORITY_CONVERSATION_TONE,
            &mut q_expression,
        );
    }

    for event in ev_reader.read() {
        let Some(kind) = tone_to_expression_kind(event.tone) else {
            continue;
        };
        let Some(lock_secs) = lock_seconds_for_completed_event(event.tone) else {
            continue;
        };

        for &entity in &event.participants {
            if q_souls.get(entity).is_err() {
                continue;
            }
            apply_expression_lock(
                &mut commands,
                entity,
                kind,
                lock_secs,
                EXPRESSION_PRIORITY_CONVERSATION_COMPLETED,
                &mut q_expression,
            );
        }
    }
}

pub fn update_conversation_expression_timer_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut ConversationExpression), With<DamnedSoul>>,
) {
    let dt = time.delta_secs();
    for (entity, mut expression) in query.iter_mut() {
        expression.remaining_secs -= dt;
        if expression.remaining_secs <= 0.0 {
            commands.entity(entity).remove::<ConversationExpression>();
        }
    }
}

/// 障害物に埋まったソウルを最寄りの歩行可能タイルへ逃がす。
/// 建築物の配置や障害物の追加で現在位置が通行不可になった場合に実行される。
pub fn soul_stuck_escape_system(
    world_map: Res<WorldMap>,
    mut query: Query<(&mut Transform, &mut Path), With<DamnedSoul>>,
) {
    for (mut transform, mut path) in query.iter_mut() {
        let current_pos = transform.translation.truncate();
        if world_map.is_walkable_world(current_pos) {
            continue;
        }
        if let Some((gx, gy)) = world_map.get_nearest_walkable_grid(current_pos) {
            let escape_pos = WorldMap::grid_to_world(gx, gy);
            transform.translation.x = escape_pos.x;
            transform.translation.y = escape_pos.y;
            path.waypoints.clear();
            path.current_index = 0;
            debug!(
                "SOUL_STUCK_ESCAPE: moved soul from {:?} to walkable {:?}",
                current_pos, escape_pos
            );
        }
    }
}

pub fn pathfinding_system(
    mut commands: Commands,
    world_map: Res<WorldMap>,
    mut pf_context: Local<PathfindingContext>,
    mut query: Query<
        (
            Entity,
            &Transform,
            &Destination,
            &mut Path,
            &mut AssignedTask,
            &IdleState,
            Option<&mut crate::systems::logistics::Inventory>,
        ),
        With<DamnedSoul>,
    >,
    // haul_cache removed
    mut queries: crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) {
    for (entity, transform, destination, mut path, mut task, idle, mut inventory_opt) in
        query.iter_mut()
    {
        let current_pos = transform.translation.truncate();
        let start_grid = WorldMap::world_to_grid(current_pos);
        let goal_grid = WorldMap::world_to_grid(destination.0);

        // すでに有効なパスがあり、目的地も変わっていないならスキップ
        //
        // ただし、移動側が衝突で waypoint をスキップして `current_index == waypoints.len()` になっている場合、
        // パスが「残っている」扱いで再計算されず、結果的に停止してしまうことがある。
        // そのため「まだパス追従中」の場合のみスキップする。
        //
        // また、パス上に新たな障害物が追加されていないかも確認する。
        if path.current_index < path.waypoints.len() && !path.waypoints.is_empty() {
            if let Some(last) = path.waypoints.last() {
                if last.distance_squared(destination.0) < 1.0 {
                    // パス上に障害物がないか確認（残りの経路部分のみ）
                    let path_blocked = path.waypoints[path.current_index..].iter().any(|wp| {
                        let grid = WorldMap::world_to_grid(*wp);
                        !world_map.is_walkable(grid.0, grid.1)
                    });

                    if !path_blocked {
                        continue;
                    }

                    // パスが阻塞された場合、再計算が必要
                    debug!(
                        "PATH: Soul {:?} path blocked by obstacle, recalculating",
                        entity
                    );
                }
            }
        }

        let has_task = !matches!(*task, AssignedTask::None);
        let idle_can_move = match idle.behavior {
            IdleBehavior::Sitting | IdleBehavior::Sleeping => false,
            _ => true,
        };

        // タスクがなく、かつアイドル移動が不要なら探索不要
        if !has_task && !idle_can_move {
            continue;
        }

        // デバッグログ: どのソウルがパス探索を行うか
        if has_task && path.waypoints.is_empty() {
            info!(
                "PATHFIND_DEBUG: Soul {:?} seeking path from {:?} to {:?}",
                entity, start_grid, goal_grid
            );
        }

        if start_grid == goal_grid {
            path.waypoints = vec![destination.0];
            path.current_index = 0;
            continue;
        }

        if let Some(grid_path) = pathfinding::find_path(
            &*world_map,
            &mut *pf_context,
            start_grid,
            goal_grid,
        )
        .or_else(|| {
            // 通常のパスが見つからない場合、ターゲットの隣接マスへのパスを試みる
            // これはターゲットが木や岩（非歩行可能）の上にある場合に有効
            debug!(
                "PATH: Soul {:?} failed find_path, trying find_path_to_adjacent",
                entity
            );
            pathfinding::find_path_to_adjacent(&*world_map, &mut *pf_context, start_grid, goal_grid)
        }) {
            path.waypoints = grid_path
                .iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
            path.current_index = 0;
            debug!("PATH: Soul {:?} found new path", entity);
        } else {
            debug!("PATH: Soul {:?} failed to find path", entity);
            path.waypoints.clear();

            // タスク実行中なら放棄
            if !matches!(*task, AssignedTask::None) {
                info!(
                    "PATH: Soul {:?} abandoning task due to unreachable destination",
                    entity
                );
                unassign_task(
                    &mut commands,
                    entity,
                    transform.translation.truncate(),
                    &mut task,
                    &mut path,
                    inventory_opt.as_deref_mut(),
                    None, // Dropped resource
                    &mut queries,
                    &*world_map,
                    true,
                );
            }
        }
    }
}

/// 移動システム
pub fn soul_movement(
    time: Res<Time>,
    world_map: Res<WorldMap>,
    mut query: Query<(
        Entity,
        &mut Transform,
        &mut Path,
        &mut AnimationState,
        &DamnedSoul,
        &IdleState,
        Option<&StressBreakdown>,
        Option<&PushingWheelbarrow>,
    )>,
) {
    for (_entity, mut transform, mut path, mut anim, soul, idle, breakdown_opt, pushing_wb) in
        query.iter_mut()
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

            // 目的地への距離が十分近い場合は到着とみなす (1.0)
            if distance > 1.0 {
                let base_speed = SOUL_SPEED_BASE;
                let motivation_bonus = soul.motivation * SOUL_SPEED_MOTIVATION_BONUS;
                let laziness_penalty = soul.laziness * SOUL_SPEED_LAZINESS_PENALTY;
                let mut speed =
                    (base_speed + motivation_bonus - laziness_penalty).max(SOUL_SPEED_MIN);

                if idle.behavior == IdleBehavior::ExhaustedGathering {
                    speed *= SOUL_SPEED_EXHAUSTED_MULTIPLIER;
                }
                if idle.behavior == IdleBehavior::Escaping {
                    speed *= ESCAPE_SPEED_MULTIPLIER;
                }
                if pushing_wb.is_some_and(|wb| wb.get().is_some()) {
                    speed *= SOUL_SPEED_WHEELBARROW_MULTIPLIER;
                }

                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;

                // --- 物理衝突チェック (Global Impassability) ---
                let next_pos = current_pos + velocity;
                let mut moved = false;

                if world_map.is_walkable_world(next_pos) {
                    // 通常移動
                    transform.translation.x = next_pos.x;
                    transform.translation.y = next_pos.y;
                    moved = true;
                } else {
                    // スライディング衝突解決
                    let next_pos_x = current_pos + Vec2::new(velocity.x, 0.0);
                    if world_map.is_walkable_world(next_pos_x) {
                        transform.translation.x = next_pos_x.x;
                        moved = true;
                    } else {
                        let next_pos_y = current_pos + Vec2::new(0.0, velocity.y);
                        if world_map.is_walkable_world(next_pos_y) {
                            transform.translation.y = next_pos_y.y;
                            moved = true;
                        }
                    }

                    if !moved && move_dist > 0.01 {
                        // 衝突でスタックした場合、パスをクリアして再計算を要求
                        path.waypoints.clear();
                        path.current_index = 0;
                    }
                }

                anim.is_moving = moved;
                if direction.x.abs() > 0.1 {
                    anim.facing_right = direction.x > 0.0;
                }
            } else {
                path.current_index += 1;
                anim.is_moving = false;
            }
        } else {
            anim.is_moving = false;
        }
    }
}

/// アニメーションシステム
pub fn animation_system(
    time: Res<Time>,
    game_assets: Res<GameAssets>,
    mut query: Query<(
        &mut Transform,
        &mut Sprite,
        &mut AnimationState,
        &DamnedSoul,
        &IdleState,
        Option<&StressBreakdown>,
        Option<&ConversationExpression>,
    )>,
) {
    for (mut transform, mut sprite, mut anim, soul, idle, breakdown_opt, expression_opt) in
        query.iter_mut()
    {
        // 進行方向に応じて左右反転（facing_right は movement 側で更新）
        sprite.flip_x = anim.facing_right;
        let desired_image = select_soul_image(&game_assets, idle, breakdown_opt, expression_opt);
        if sprite.image != *desired_image {
            sprite.image = desired_image.clone();
        }

        // 浮遊アニメーション（translation はロジック座標と干渉するため変更しない）
        anim.bob_timer += time.delta_secs();
        let sway = (anim.bob_timer * SOUL_FLOAT_SWAY_SPEED).sin();

        let speed_scale = if anim.is_moving { 1.3 } else { 1.0 };
        let pulse_speed = (SOUL_FLOAT_PULSE_SPEED_BASE + (1.0 - soul.laziness) * 0.4) * speed_scale;
        let pulse = (anim.bob_timer * pulse_speed).sin();
        let pulse_amplitude = if anim.is_moving {
            SOUL_FLOAT_PULSE_AMPLITUDE_MOVE
        } else {
            SOUL_FLOAT_PULSE_AMPLITUDE_IDLE
        };
        let base_scale = if anim.is_moving { 1.02 } else { 1.0 };

        transform.scale = Vec3::new(
            base_scale + pulse * (pulse_amplitude * 0.6),
            base_scale + pulse * pulse_amplitude,
            1.0,
        );

        let tilt = if anim.is_moving {
            SOUL_FLOAT_SWAY_TILT_MOVE
        } else {
            SOUL_FLOAT_SWAY_TILT_IDLE
        };
        transform.rotation = Quat::from_rotation_z(sway * tilt);
    }
}
