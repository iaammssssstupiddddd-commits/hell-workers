//! ポップアップ・完成テキスト・バウンスアニメーション

use crate::constants::*;
use bevy::prelude::*;

use super::POPUP_LIFETIME;
use super::components::{BlueprintVisual, BuildingBounceEffect, CompletionText, DeliveryPopup};
use crate::systems::jobs::Blueprint;
use crate::systems::utils::animations::update_bounce_animation;
use crate::systems::utils::floating_text::{
    FloatingText, FloatingTextConfig, spawn_floating_text, update_floating_text,
};

/// 資材搬入時のエフェクト（ポップアップ）を発生させる
pub fn material_delivery_vfx_system(
    mut commands: Commands,
    mut q_visuals: Query<(Entity, &mut BlueprintVisual, &Blueprint, &Transform)>,
) {
    for (_, mut visual, bp, transform) in q_visuals.iter_mut() {
        for (resource_type, &current_count) in &bp.delivered_materials {
            let last_count = visual.last_delivered.get(resource_type).unwrap_or(&0);
            if current_count > *last_count {
                // utilを使用してポップアップ生成
                let config = FloatingTextConfig {
                    lifetime: POPUP_LIFETIME,
                    velocity: Vec2::new(0.0, 20.0),
                    initial_color: Color::srgb(1.0, 1.0, 0.5),
                    fade_out: true,
                };

                let popup_entity = spawn_floating_text(
                    &mut commands,
                    "+1",
                    transform.translation.truncate().extend(Z_FLOATING_TEXT)
                        + Vec3::new(0.0, 10.0, 0.0),
                    config.clone(),
                    Some(12.0),
                );

                // ラッパーコンポーネントを追加
                commands.entity(popup_entity).insert(DeliveryPopup {
                    floating_text: FloatingText {
                        lifetime: config.lifetime,
                        config,
                    },
                });
            }
            visual.last_delivered.insert(*resource_type, current_count);
        }
    }
}

/// 搬入ポップアップのアニメーションと削除
pub fn update_delivery_popup_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_popups: Query<(
        Entity,
        &mut DeliveryPopup,
        &mut FloatingText,
        &mut Transform,
        &mut TextColor,
    )>,
) {
    for (entity, mut popup, mut floating_text, mut transform, mut color) in q_popups.iter_mut() {
        // utilを使用してフローティングテキストを更新
        let (should_despawn, new_position, alpha) =
            update_floating_text(&time, &mut floating_text, transform.translation);

        if should_despawn {
            commands.entity(entity).despawn();
            continue;
        }

        // ラッパーコンポーネントも更新
        popup.floating_text = (*floating_text).clone();

        transform.translation = new_position;
        color.0 = color.0.with_alpha(alpha);
    }
}

/// 完成時テキストのアニメーションと削除
pub fn update_completion_text_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_texts: Query<(
        Entity,
        &mut CompletionText,
        &mut FloatingText,
        &mut Transform,
        &mut TextColor,
    )>,
) {
    for (entity, mut completion, mut floating_text, mut transform, mut color) in q_texts.iter_mut()
    {
        // utilを使用してフローティングテキストを更新
        let (should_despawn, new_position, alpha) =
            update_floating_text(&time, &mut floating_text, transform.translation);

        if should_despawn {
            commands.entity(entity).despawn();
            continue;
        }

        // ラッパーコンポーネントも更新
        completion.floating_text = (*floating_text).clone();

        transform.translation = new_position;
        color.0 = color.0.with_alpha(alpha);
    }
}

/// 完成した建物のバウンスアニメーション
pub fn building_bounce_animation_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_bounces: Query<(Entity, &mut BuildingBounceEffect, &mut Transform)>,
) {
    for (entity, mut bounce, mut transform) in q_bounces.iter_mut() {
        // utilを使用してバウンスアニメーションを更新
        if let Some(scale) = update_bounce_animation(&time, &mut bounce.bounce_animation) {
            transform.scale = Vec3::splat(scale);
        } else {
            // アニメーション完了
            transform.scale = Vec3::ONE;
            commands.entity(entity).remove::<BuildingBounceEffect>();
        }
    }
}
