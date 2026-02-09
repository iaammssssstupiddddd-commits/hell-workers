//! 手押し車の追従・ビジュアルシステム
//!
//! 魂が手押し車を使用中、手押し車を魂の前方にオフセットして追従させる。

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{AnimationState, DamnedSoul};
use crate::relationships::{LoadedItems, PushedBy};
use crate::systems::logistics::Wheelbarrow;

/// 手押し車を使用中の魂に追従させるシステム
pub fn wheelbarrow_follow_system(
    q_souls: Query<(&Transform, &AnimationState), With<DamnedSoul>>,
    mut q_wheelbarrows: Query<
        (&mut Transform, &mut Sprite, &PushedBy, &LoadedItems),
        (With<Wheelbarrow>, Without<DamnedSoul>),
    >,
    game_assets: Res<GameAssets>,
) {
    for (mut wb_tf, mut wb_sprite, pushed_by, loaded_items) in &mut q_wheelbarrows {
        let Ok((soul_tf, anim)) = q_souls.get(pushed_by.0) else {
            continue;
        };

        // 向きに応じたオフセット（facing_right なら右前方、そうでなければ左前方）
        let offset_x = if anim.facing_right {
            WHEELBARROW_OFFSET
        } else {
            -WHEELBARROW_OFFSET
        };

        wb_tf.translation.x = soul_tf.translation.x + offset_x;
        wb_tf.translation.y = soul_tf.translation.y;
        wb_tf.translation.z = soul_tf.translation.z - 0.1;

        // 積載状態でスプライト切替
        let target_image = if loaded_items.is_empty() {
            &game_assets.wheelbarrow_empty
        } else {
            &game_assets.wheelbarrow_loaded
        };
        if wb_sprite.image != *target_image {
            wb_sprite.image = target_image.clone();
        }

        // 歩行bob同期（魂の50%振幅、uniform scale）
        if anim.is_moving {
            let bob = (anim.bob_timer.sin() * ANIM_BOB_AMPLITUDE * 0.5) + 1.0;
            wb_tf.scale = Vec3::splat(bob);
        } else {
            wb_tf.scale = Vec3::ONE;
        }
    }
}
