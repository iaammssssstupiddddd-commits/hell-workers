//! ポップアップ・完成テキスト・バウンスアニメーション

use bevy::prelude::*;
use hw_core::constants::*;

use super::POPUP_LIFETIME;
use super::components::{BlueprintVisual, BuildingBounceEffect, CompletionText, DeliveryPopup};
use crate::animations::update_bounce_animation;
use crate::floating_text::{
    FloatingText, FloatingTextConfig, spawn_floating_text, update_floating_text,
};
use crate::handles::MaterialIconHandles;
use hw_core::visual_mirror::construction::BlueprintVisualState;

pub fn material_delivery_vfx_system(
    mut commands: Commands,
    mut q_visuals: Query<(Entity, &mut BlueprintVisual, &BlueprintVisualState, &Transform)>,
    material_icon_handles: Res<MaterialIconHandles>,
) {
    for (_, mut visual, state, transform) in q_visuals.iter_mut() {
        for (resource_type, current_count, _) in &state.material_counts {
            let last_count = visual.last_delivered.get(resource_type).copied().unwrap_or(0);
            if *current_count > last_count {
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
                    material_icon_handles.font_ui.clone(),
                );

                commands.entity(popup_entity).insert(DeliveryPopup {
                    floating_text: FloatingText {
                        lifetime: config.lifetime,
                        config,
                    },
                });
            }
            visual.last_delivered.insert(*resource_type, *current_count);
        }
    }
}

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
        let (should_despawn, new_position, alpha) =
            update_floating_text(&time, &mut floating_text, transform.translation);

        if should_despawn {
            commands.entity(entity).despawn();
            continue;
        }

        popup.floating_text = (*floating_text).clone();

        transform.translation = new_position;
        color.0 = color.0.with_alpha(alpha);
    }
}

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
        let (should_despawn, new_position, alpha) =
            update_floating_text(&time, &mut floating_text, transform.translation);

        if should_despawn {
            commands.entity(entity).despawn();
            continue;
        }

        completion.floating_text = (*floating_text).clone();

        transform.translation = new_position;
        color.0 = color.0.with_alpha(alpha);
    }
}

pub fn building_bounce_animation_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_bounces: Query<(Entity, &mut BuildingBounceEffect, &mut Transform)>,
) {
    for (entity, mut bounce, mut transform) in q_bounces.iter_mut() {
        if let Some(scale) = update_bounce_animation(&time, &mut bounce.bounce_animation) {
            transform.scale = Vec3::splat(scale);
        } else {
            transform.scale = Vec3::ONE;
            commands.entity(entity).remove::<BuildingBounceEffect>();
        }
    }
}
