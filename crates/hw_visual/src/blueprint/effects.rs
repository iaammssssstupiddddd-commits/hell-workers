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
    mut q_visuals: Query<(
        Entity,
        &mut BlueprintVisual,
        &BlueprintVisualState,
        &Transform,
    )>,
    material_icon_handles: Res<MaterialIconHandles>,
) {
    for (_, mut visual, state, transform) in q_visuals.iter_mut() {
        for (resource_type, current_count, _) in &state.material_counts {
            let last_count = visual
                .last_delivered
                .get(resource_type)
                .copied()
                .unwrap_or(0);
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
            let next_scale = Vec3::splat(scale);
            // A paused static fixture keeps a completion effect at its
            // current value. Avoid marking every owner Transform changed
            // when that value is already applied; downstream presentation
            // consumers correctly treat Changed<Transform> as real work.
            if transform.scale != next_scale {
                transform.scale = next_scale;
            }
        } else {
            if transform.scale != Vec3::ONE {
                transform.scale = Vec3::ONE;
            }
            commands.entity(entity).remove::<BuildingBounceEffect>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct ChangedTransformCount(usize);

    fn count_changed_transforms(
        transforms: Query<Entity, Changed<Transform>>,
        mut count: ResMut<ChangedTransformCount>,
    ) {
        count.0 += transforms.iter().count();
    }

    #[test]
    fn paused_completion_bounce_does_not_rechange_an_already_applied_scale() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<Time<Virtual>>()
            .init_resource::<ChangedTransformCount>()
            .add_systems(
                Update,
                (building_bounce_animation_system, count_changed_transforms).chain(),
            );
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let paused_time = app.world().resource::<Time<Virtual>>().as_generic();
        app.world_mut().insert_resource(paused_time);
        app.world_mut()
            .spawn((BuildingBounceEffect::completion(), Transform::default()));

        // The first frame observes the newly spawned Transform. Subsequent
        // paused frames must not create a false change when scale stays one.
        app.update();
        app.world_mut().resource_mut::<ChangedTransformCount>().0 = 0;
        app.update();

        assert_eq!(app.world().resource::<ChangedTransformCount>().0, 0);
    }
}
