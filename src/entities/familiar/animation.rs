//! 使い魔のアニメーション

use bevy::prelude::*;

use crate::constants::*;

use super::components::Familiar;

/// 使い魔のアニメーション更新システム
pub fn familiar_animation_system(
    time: Res<Time>,
    game_assets: Res<crate::assets::GameAssets>,
    mut query: Query<
        (
            &mut Sprite,
            &mut super::components::FamiliarAnimation,
            &mut Transform,
        ),
        With<Familiar>,
    >,
) {
    for (mut sprite, mut anim, mut transform) in query.iter_mut() {
        if anim.hover_offset != 0.0 {
            transform.translation.y -= anim.hover_offset;
        }

        sprite.flip_x = !anim.facing_right;

        if anim.is_moving {
            anim.timer += time.delta_secs();
            anim.frame = ((anim.timer * FAMILIAR_MOVE_ANIMATION_FPS) as usize)
                % FAMILIAR_MOVE_ANIMATION_FRAMES;
        } else {
            anim.timer = 0.0;
            anim.frame = 0;
        }

        sprite.texture_atlas = None;
        sprite.image = match anim.frame {
            0 => game_assets.familiar.clone(),
            1 => game_assets.familiar_anim_2.clone(),
            2 => game_assets.familiar_anim_3.clone(),
            _ => game_assets.familiar_anim_4.clone(),
        };

        anim.hover_timer += time.delta_secs() * FAMILIAR_HOVER_SPEED;
        let hover_amplitude = if anim.is_moving {
            FAMILIAR_HOVER_AMPLITUDE_MOVE
        } else {
            FAMILIAR_HOVER_AMPLITUDE_IDLE
        };
        let hover_offset = anim.hover_timer.sin() * hover_amplitude;
        anim.hover_offset = hover_offset;
        transform.translation.y += hover_offset;

        let dir_tilt = if anim.is_moving {
            if anim.facing_right { -0.04 } else { 0.04 }
        } else {
            0.0
        };
        let wobble_tilt = (anim.hover_timer * 0.8).sin() * FAMILIAR_HOVER_TILT_AMPLITUDE;
        transform.rotation = Quat::from_rotation_z(dir_tilt + wobble_tilt);
    }
}
