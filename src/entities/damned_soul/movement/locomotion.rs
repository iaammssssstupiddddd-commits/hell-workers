//! パス追従による移動

use crate::constants::*;
use crate::entities::damned_soul::{AnimationState, DamnedSoul, IdleBehavior, IdleState, Path, StressBreakdown};
use crate::relationships::PushingWheelbarrow;
use crate::world::map::WorldMap;
use bevy::prelude::*;

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
