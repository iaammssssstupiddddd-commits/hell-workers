//! 手押し車の追従・ビジュアルシステム

use bevy::prelude::*;

use crate::handles::HaulItemHandles;
use hw_core::constants::*;
use hw_core::relationships::{LoadedItems, PushedBy};
use hw_core::soul::{AnimationState, DamnedSoul};
use hw_core::visual::WheelbarrowMovement;
use hw_core::visual_mirror::logistics::WheelbarrowMarker;

pub fn wheelbarrow_follow_system(
    mut commands: Commands,
    q_souls: Query<(&Transform, &AnimationState), With<DamnedSoul>>,
    mut q_wheelbarrows: Query<
        (
            Entity,
            &mut Transform,
            &mut Sprite,
            &PushedBy,
            Option<&LoadedItems>,
            Option<&mut WheelbarrowMovement>,
        ),
        (With<WheelbarrowMarker>, Without<DamnedSoul>),
    >,
    handles: Res<HaulItemHandles>,
) {
    for (wb_entity, mut wb_tf, mut wb_sprite, pushed_by, loaded_items, movement) in
        &mut q_wheelbarrows
    {
        let Ok((soul_tf, anim)) = q_souls.get(pushed_by.0) else {
            continue;
        };

        let soul_pos = soul_tf.translation.truncate();

        let mut movement = match movement {
            Some(m) => m,
            None => {
                let initial_angle = if anim.facing_right {
                    -std::f32::consts::FRAC_PI_2
                } else {
                    std::f32::consts::FRAC_PI_2
                };
                if let Ok(mut wb_commands) = commands.get_entity(wb_entity) {
                    wb_commands.try_insert((
                        WheelbarrowMovement {
                            prev_pos: Some(soul_pos),
                            current_angle: initial_angle,
                        },
                        Visibility::Visible,
                    ));
                }
                let offset_x = if anim.facing_right {
                    WHEELBARROW_OFFSET
                } else {
                    -WHEELBARROW_OFFSET
                };
                wb_tf.translation.x = soul_tf.translation.x + offset_x;
                wb_tf.translation.y = soul_tf.translation.y;
                wb_tf.translation.z = soul_tf.translation.z - 0.1;
                wb_tf.rotation = Quat::from_rotation_z(initial_angle);
                wb_tf.scale = Vec3::splat(WHEELBARROW_ACTIVE_SCALE);
                continue;
            }
        };

        if let Some(prev) = movement.prev_pos {
            let delta = soul_pos - prev;
            if delta.length_squared() > 0.01 {
                let target_angle = delta.y.atan2(delta.x) - std::f32::consts::FRAC_PI_2;
                movement.current_angle = target_angle;
            }
        }
        movement.prev_pos = Some(soul_pos);

        let offset_dir = Vec2::from_angle(movement.current_angle + std::f32::consts::FRAC_PI_2);
        wb_tf.translation.x = soul_tf.translation.x + offset_dir.x * WHEELBARROW_OFFSET;
        wb_tf.translation.y = soul_tf.translation.y + offset_dir.y * WHEELBARROW_OFFSET;
        wb_tf.translation.z = soul_tf.translation.z - 0.1;

        let has_loaded_items = loaded_items.is_some_and(|li| !li.is_empty());
        let target_image = if has_loaded_items {
            &handles.wheelbarrow_loaded
        } else {
            &handles.wheelbarrow_empty
        };
        if wb_sprite.image != *target_image {
            wb_sprite.image = target_image.clone();
        }

        wb_tf.rotation = Quat::from_rotation_z(movement.current_angle);

        let base_scale = WHEELBARROW_ACTIVE_SCALE;
        if anim.is_moving {
            let bob = (anim.bob_timer.sin() * ANIM_BOB_AMPLITUDE * 0.5) + 1.0;
            wb_tf.scale = Vec3::splat(base_scale * bob);
        } else {
            wb_tf.scale = Vec3::splat(base_scale);
        }
    }
}
