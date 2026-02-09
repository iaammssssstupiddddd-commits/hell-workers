//! 手押し車の追従・ビジュアルシステム
//!
//! 魂が手押し車を使用中、手押し車を魂の前方にオフセットして追従させる。
//! 経路の実際の移動方向に基づいて手押し車を回転させる。

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{AnimationState, DamnedSoul};
use crate::relationships::{LoadedItems, PushedBy};
use crate::systems::logistics::Wheelbarrow;

/// 手押し車の前フレーム位置と現在の回転角度を追跡するコンポーネント
#[derive(Component, Default)]
pub struct WheelbarrowMovement {
    pub prev_pos: Option<Vec2>,
    pub current_angle: f32,
}

/// 手押し車を使用中の魂に追従させるシステム
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
        (With<Wheelbarrow>, Without<DamnedSoul>),
    >,
    game_assets: Res<GameAssets>,
) {
    for (wb_entity, mut wb_tf, mut wb_sprite, pushed_by, loaded_items, movement) in
        &mut q_wheelbarrows
    {
        let Ok((soul_tf, anim)) = q_souls.get(pushed_by.0) else {
            continue;
        };

        let soul_pos = soul_tf.translation.truncate();

        // WheelbarrowMovement がなければ初期化
        let mut movement = match movement {
            Some(m) => m,
            None => {
                let initial_angle = if anim.facing_right {
                    -std::f32::consts::FRAC_PI_2
                } else {
                    std::f32::consts::FRAC_PI_2
                };
                commands.entity(wb_entity).insert((
                    WheelbarrowMovement {
                        prev_pos: Some(soul_pos),
                        current_angle: initial_angle,
                    },
                    Visibility::Visible,
                ));
                // 初回フレームはfacing_rightベースで配置
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

        // 移動方向から回転角度を計算（元画像は上向き）
        if let Some(prev) = movement.prev_pos {
            let delta = soul_pos - prev;
            if delta.length_squared() > 0.01 {
                // atan2で方向角を計算（上向き画像なので、右=(-π/2), 上=(0), 左=(π/2), 下=(π)）
                let target_angle = delta.y.atan2(delta.x) - std::f32::consts::FRAC_PI_2;
                movement.current_angle = target_angle;
            }
        }
        movement.prev_pos = Some(soul_pos);

        // 回転角度に応じたオフセット方向（手押し車は魂の進行方向前方に配置）
        let offset_dir = Vec2::from_angle(movement.current_angle + std::f32::consts::FRAC_PI_2);
        wb_tf.translation.x = soul_tf.translation.x + offset_dir.x * WHEELBARROW_OFFSET;
        wb_tf.translation.y = soul_tf.translation.y + offset_dir.y * WHEELBARROW_OFFSET;
        wb_tf.translation.z = soul_tf.translation.z - 0.1;

        // 積載状態でスプライト切替
        // LoadedItems は #[relationship_target] で自動管理されるため、
        // LoadedIn を持つエンティティがない場合はコンポーネント自体が不在になりうる
        let has_loaded_items = loaded_items.is_some_and(|li| !li.is_empty());
        let target_image = if has_loaded_items {
            &game_assets.wheelbarrow_loaded
        } else {
            &game_assets.wheelbarrow_empty
        };
        if wb_sprite.image != *target_image {
            wb_sprite.image = target_image.clone();
        }

        // 回転適用
        wb_tf.rotation = Quat::from_rotation_z(movement.current_angle);

        // 運搬中は目立つサイズにスケール + 歩行bob同期
        let base_scale = WHEELBARROW_ACTIVE_SCALE;
        if anim.is_moving {
            let bob = (anim.bob_timer.sin() * ANIM_BOB_AMPLITUDE * 0.5) + 1.0;
            wb_tf.scale = Vec3::splat(base_scale * bob);
        } else {
            wb_tf.scale = Vec3::splat(base_scale);
        }
    }
}
