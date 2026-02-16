//! 使い魔の移動

use bevy::prelude::*;

use crate::entities::damned_soul::Path;

use super::components::{Familiar, FamiliarAnimation};

/// 使い魔の移動システム
pub fn familiar_movement(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Path, &mut FamiliarAnimation), With<Familiar>>,
) {
    for (mut transform, mut path, mut anim) in query.iter_mut() {
        if anim.hover_offset != 0.0 {
            transform.translation.y -= anim.hover_offset;
            anim.hover_offset = 0.0;
        }

        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let to_target = target - current_pos;
            let distance = to_target.length();

            if distance > 1.0 {
                let speed = 100.0;
                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;
                let next_pos = current_pos + velocity;
                transform.translation.x = next_pos.x;
                transform.translation.y = next_pos.y;
                let moved = move_dist > 0.0;

                anim.is_moving = moved;
                if moved && move_dist > 0.0 {
                    debug!(
                        "FAM_MOV: Moving towards waypoint. dist: {:.1}, move: {:.1}",
                        distance, move_dist
                    );
                }
                if direction.x.abs() > 0.1 {
                    anim.facing_right = direction.x > 0.0;
                }
            } else {
                info!("FAM_MOV: Reached waypoint index {}", path.current_index);
                path.current_index += 1;
            }
        } else {
            anim.is_moving = false;
        }
    }
}
