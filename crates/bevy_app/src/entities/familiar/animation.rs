//! 使い魔のアニメーション

use bevy::prelude::*;

use hw_core::constants::*;

use super::components::Familiar;

/// 使い魔のアニメーション更新システム
pub fn familiar_animation_system(
    time: Res<Time>,
    game_assets: Res<crate::assets::GameAssets>,
    mut q_animations: Query<&mut super::components::FamiliarAnimation, With<Familiar>>,
    mut q_visuals: Query<
        (
            &hw_visual::FamiliarVisualOwner,
            &mut Sprite,
            &mut Transform,
            &mut hw_visual::FamiliarVisualOffset,
        ),
        Without<Familiar>,
    >,
) {
    for (owner, mut sprite, mut transform, mut offset) in q_visuals.iter_mut() {
        let Ok(mut anim) = q_animations.get_mut(owner.owner) else {
            continue;
        };

        let desired_flip_x = !anim.facing_right;
        if sprite.flip_x != desired_flip_x {
            sprite.flip_x = desired_flip_x;
        }

        if anim.is_moving {
            anim.timer += time.delta_secs();
            anim.frame = ((anim.timer * FAMILIAR_MOVE_ANIMATION_FPS) as usize)
                % FAMILIAR_MOVE_ANIMATION_FRAMES;
        } else {
            anim.timer = 0.0;
            anim.frame = 0;
        }

        let desired_image = match anim.frame {
            0 => game_assets.familiar.clone(),
            1 => game_assets.familiar_anim_2.clone(),
            2 => game_assets.familiar_anim_3.clone(),
            _ => game_assets.familiar_anim_4.clone(),
        };
        if sprite.texture_atlas.is_some() {
            sprite.texture_atlas = None;
        }
        if sprite.image != desired_image {
            sprite.image = desired_image;
        }

        anim.hover_timer += time.delta_secs() * FAMILIAR_HOVER_SPEED;
        let hover_amplitude = if anim.is_moving {
            FAMILIAR_HOVER_AMPLITUDE_MOVE
        } else {
            FAMILIAR_HOVER_AMPLITUDE_IDLE
        };
        let hover_offset = anim.hover_timer.sin() * hover_amplitude;
        anim.hover_offset = hover_offset;

        let dir_tilt = if anim.is_moving {
            if anim.facing_right { -0.04 } else { 0.04 }
        } else {
            0.0
        };
        let wobble_tilt = (anim.hover_timer * 0.8).sin() * FAMILIAR_HOVER_TILT_AMPLITUDE;
        let tilt_radians = dir_tilt + wobble_tilt;
        let desired_translation = Vec3::Y * hover_offset;
        let desired_rotation = Quat::from_rotation_z(tilt_radians);
        if transform.translation != desired_translation {
            transform.translation = desired_translation;
        }
        if transform.rotation != desired_rotation {
            transform.rotation = desired_rotation;
        }
        offset.hover_offset = hover_offset;
        offset.tilt_radians = tilt_radians;
    }
}
