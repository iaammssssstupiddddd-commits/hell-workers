//! Dream 獲得時のビジュアル表現（UIパーティクル・ポップアップテキスト）

use super::components::{DreamGainPopup, DreamVisualState};
use super::dream_bubble_material::DreamBubbleUiMaterial;
use crate::floating_text::{
    FloatingText, FloatingTextConfig, spawn_floating_text, update_floating_text,
};
use crate::handles::MaterialIconHandles;
use bevy::prelude::*;
use hw_core::camera::MainCamera;
use hw_core::constants::*;
use hw_core::relationships::ParticipatingIn;
use hw_core::soul::{DamnedSoul, DreamState, GatheringBehavior, IdleBehavior, IdleState};
use hw_core::ui_nodes::{UiNodeRegistry, UiRoot, UiSlot};

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn dream_popup_spawn_system(
    mut commands: Commands,
    time: Res<Time>,
    handles: Res<MaterialIconHandles>,
    mut ui_bubble_materials: ResMut<Assets<DreamBubbleUiMaterial>>,
    mut q_souls: Query<(
        &Transform,
        &DamnedSoul,
        &IdleState,
        &DreamState,
        Option<&ParticipatingIn>,
        &mut DreamVisualState,
    )>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_ui_root: Query<Entity, With<UiRoot>>,
    ui_nodes: Res<UiNodeRegistry>,
    q_ui_transform: Query<(&ComputedNode, &UiGlobalTransform)>,
) {
    let dt = time.delta_secs();

    let Some((camera, camera_transform)) = q_camera.iter().next() else {
        return;
    };
    let Some(ui_root) = q_ui_root.iter().next() else {
        return;
    };

    let viewport_size = camera
        .logical_viewport_size()
        .unwrap_or(Vec2::new(1920.0, 1080.0));

    let mut target_pos = Vec2::new(viewport_size.x - 80.0, 40.0);

    if let Some(entity) = ui_nodes.get_slot(UiSlot::DreamPoolIcon)
        && let Ok((computed, transform)) = q_ui_transform.get(entity) {
            let center = transform.translation * computed.inverse_scale_factor();
            target_pos = center;
        }

    for (transform, soul, idle, _dream, participating_in, mut visual_state) in q_souls.iter_mut() {
        let is_sleeping = idle.behavior == IdleBehavior::Sleeping
            || (idle.behavior == IdleBehavior::Gathering
                && idle.gathering_behavior == GatheringBehavior::Sleeping
                && participating_in.is_some());
        let is_draining = is_sleeping && soul.dream > 0.0;

        if !is_draining {
            if visual_state.popup_accumulated > 0.0 {
                let amount = visual_state.popup_accumulated;
                visual_state.popup_accumulated = 0.0;

                let config = FloatingTextConfig {
                    lifetime: DREAM_POPUP_LIFETIME,
                    velocity: Vec2::new(0.0, DREAM_POPUP_VELOCITY_Y),
                    initial_color: Color::srgb(0.65, 0.9, 1.0),
                    fade_out: true,
                };

                let popup_pos = transform.translation.truncate().extend(Z_FLOATING_TEXT)
                    + Vec3::new(0.0, DREAM_POPUP_OFFSET_Y, 0.0);

                let popup_entity = spawn_floating_text(
                    &mut commands,
                    format!("+{:.1} Dream", amount),
                    popup_pos,
                    config.clone(),
                    Some(DREAM_POPUP_FONT_SIZE),
                    handles.font_ui.clone(),
                );

                commands.entity(popup_entity).insert(DreamGainPopup {
                    floating_text: FloatingText {
                        lifetime: config.lifetime,
                        config,
                    },
                });

                if let Ok(start_pos) = camera.world_to_viewport(camera_transform, popup_pos) {
                    super::ui_particle::spawn_ui_particle(
                        &mut commands,
                        start_pos,
                        target_pos,
                        ui_root,
                        &mut ui_bubble_materials,
                        amount,
                    );
                }
            }

            visual_state.popup_timer = 0.0;
            continue;
        }

        visual_state.popup_accumulated += DREAM_DRAIN_RATE.min(soul.dream) * dt;

        visual_state.popup_timer += dt;
        if visual_state.popup_timer >= DREAM_POPUP_INTERVAL {
            visual_state.popup_timer -= f32::max(DREAM_POPUP_INTERVAL, dt);

            if visual_state.popup_accumulated >= DREAM_POPUP_THRESHOLD {
                let amount = visual_state.popup_accumulated;
                visual_state.popup_accumulated = 0.0;

                let config = FloatingTextConfig {
                    lifetime: DREAM_POPUP_LIFETIME,
                    velocity: Vec2::new(0.0, DREAM_POPUP_VELOCITY_Y),
                    initial_color: Color::srgb(0.65, 0.9, 1.0),
                    fade_out: true,
                };

                let popup_pos = transform.translation.truncate().extend(Z_FLOATING_TEXT)
                    + Vec3::new(0.0, DREAM_POPUP_OFFSET_Y, 0.0);

                let popup_entity = spawn_floating_text(
                    &mut commands,
                    "+Dream",
                    popup_pos,
                    config.clone(),
                    Some(DREAM_POPUP_FONT_SIZE),
                    handles.font_ui.clone(),
                );

                commands.entity(popup_entity).insert(DreamGainPopup {
                    floating_text: FloatingText {
                        lifetime: config.lifetime,
                        config,
                    },
                });

                if let Ok(start_pos) = camera.world_to_viewport(camera_transform, popup_pos) {
                    super::ui_particle::spawn_ui_particle(
                        &mut commands,
                        start_pos,
                        target_pos,
                        ui_root,
                        &mut ui_bubble_materials,
                        amount,
                    );
                }
            }
        }
    }
}

pub fn dream_popup_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_popups: Query<(
        Entity,
        &mut DreamGainPopup,
        &mut FloatingText,
        &mut Transform,
        &mut TextColor,
    )>,
) {
    for (entity, mut popup, mut floating_text, mut transform, mut color) in q_popups.iter_mut() {
        let (should_despawn, new_position, alpha) =
            update_floating_text(&time, &mut floating_text, transform.translation);

        if should_despawn {
            commands.entity(entity).try_despawn();
            continue;
        }

        popup.floating_text = (*floating_text).clone();
        transform.translation = new_position;
        color.0 = color.0.with_alpha(alpha);
    }
}
