//! Dream 獲得時のビジュアル表現（UIパーティクル・ポップアップテキスト）

use super::components::{DreamGainPopup, DreamVisualState};
use super::dream_bubble_material::DreamBubbleUiMaterial;
use crate::floating_text::{
    FloatingText, FloatingTextConfig, spawn_floating_text, update_floating_text,
};
use crate::handles::MaterialIconHandles;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::camera::MainCamera;
use hw_core::constants::*;
use hw_core::relationships::ParticipatingIn;
use hw_core::soul::{DamnedSoul, DreamState, GatheringBehavior, IdleBehavior, IdleState};
use hw_core::ui_nodes::{UiNodeRegistry, UiRoot, UiSlot};

type DreamSoulsQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static DamnedSoul,
        &'static IdleState,
        &'static DreamState,
        Option<&'static ParticipatingIn>,
        &'static mut DreamVisualState,
    ),
>;

#[derive(SystemParam)]
pub struct DreamPopupParams<'w, 's> {
    commands: Commands<'w, 's>,
    time: Res<'w, Time>,
    handles: Res<'w, MaterialIconHandles>,
    ui_bubble_materials: ResMut<'w, Assets<DreamBubbleUiMaterial>>,
    q_souls: DreamSoulsQuery<'w, 's>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    q_ui_root: Query<'w, 's, Entity, With<UiRoot>>,
    ui_nodes: Res<'w, UiNodeRegistry>,
    q_ui_transform: Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform)>,
}

pub fn dream_popup_spawn_system(mut p: DreamPopupParams) {
    let dt = p.time.delta_secs();

    let Some((camera, camera_transform)) = p.q_camera.iter().next() else {
        return;
    };
    let Some(ui_root) = p.q_ui_root.iter().next() else {
        return;
    };

    let viewport_size = camera
        .logical_viewport_size()
        .unwrap_or(Vec2::new(1920.0, 1080.0));

    let mut target_pos = Vec2::new(viewport_size.x - 80.0, 40.0);

    if let Some(entity) = p.ui_nodes.get_slot(UiSlot::DreamPoolIcon)
        && let Ok((computed, transform)) = p.q_ui_transform.get(entity) {
            let center = transform.translation * computed.inverse_scale_factor();
            target_pos = center;
        }

    for (transform, soul, idle, _dream, participating_in, mut visual_state) in p.q_souls.iter_mut() {
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
                    &mut p.commands,
                    format!("+{:.1} Dream", amount),
                    popup_pos,
                    config.clone(),
                    Some(DREAM_POPUP_FONT_SIZE),
                    p.handles.font_ui.clone(),
                );

                p.commands.entity(popup_entity).insert(DreamGainPopup {
                    floating_text: FloatingText {
                        lifetime: config.lifetime,
                        config,
                    },
                });

                if let Ok(start_pos) = camera.world_to_viewport(camera_transform, popup_pos) {
                    super::ui_particle::spawn_ui_particle(
                        &mut p.commands,
                        start_pos,
                        target_pos,
                        ui_root,
                        &mut p.ui_bubble_materials,
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
                    &mut p.commands,
                    "+Dream",
                    popup_pos,
                    config.clone(),
                    Some(DREAM_POPUP_FONT_SIZE),
                    p.handles.font_ui.clone(),
                );

                p.commands.entity(popup_entity).insert(DreamGainPopup {
                    floating_text: FloatingText {
                        lifetime: config.lifetime,
                        config,
                    },
                });

                if let Ok(start_pos) = camera.world_to_viewport(camera_transform, popup_pos) {
                    super::ui_particle::spawn_ui_particle(
                        &mut p.commands,
                        start_pos,
                        target_pos,
                        ui_root,
                        &mut p.ui_bubble_materials,
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
