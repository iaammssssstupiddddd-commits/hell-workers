use super::components::{DreamGainPopup, DreamVisualState};
use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, DreamQuality, DreamState, GatheringBehavior, IdleBehavior, IdleState,
};
use crate::relationships::ParticipatingIn;
use crate::systems::utils::floating_text::{
    FloatingText, FloatingTextConfig, spawn_floating_text, update_floating_text,
};
use bevy::prelude::*;

fn dream_gain_rate(quality: DreamQuality) -> f32 {
    match quality {
        DreamQuality::VividDream => DREAM_RATE_VIVID,
        DreamQuality::NormalDream => DREAM_RATE_NORMAL,
        DreamQuality::NightTerror | DreamQuality::Awake => 0.0,
    }
}

pub fn dream_popup_spawn_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut q_souls: Query<
        (
            &Transform,
            &IdleState,
            &DreamState,
            Option<&ParticipatingIn>,
            &mut DreamVisualState,
        ),
        With<DamnedSoul>,
    >,
) {
    let dt = time.delta_secs();

    for (transform, idle, dream, participating_in, mut visual_state) in q_souls.iter_mut() {
        let is_sleeping = idle.behavior == IdleBehavior::Sleeping
            || (idle.behavior == IdleBehavior::Gathering
                && idle.gathering_behavior == GatheringBehavior::Sleeping
                && participating_in.is_some());
        if !is_sleeping {
            visual_state.popup_accumulated = 0.0;
            continue;
        }

        let gain_rate = dream_gain_rate(dream.quality);
        if gain_rate <= 0.0 {
            continue;
        }

        visual_state.popup_accumulated += gain_rate * dt;
        while visual_state.popup_accumulated >= DREAM_POPUP_THRESHOLD {
            visual_state.popup_accumulated -= DREAM_POPUP_THRESHOLD;

            let config = FloatingTextConfig {
                lifetime: DREAM_POPUP_LIFETIME,
                velocity: Vec2::new(0.0, DREAM_POPUP_VELOCITY_Y),
                initial_color: Color::srgb(0.65, 0.9, 1.0),
                fade_out: true,
            };

            let popup_entity = spawn_floating_text(
                &mut commands,
                "+Dream",
                transform.translation.truncate().extend(Z_FLOATING_TEXT)
                    + Vec3::new(0.0, DREAM_POPUP_OFFSET_Y, 0.0),
                config.clone(),
                Some(DREAM_POPUP_FONT_SIZE),
                assets.font_ui.clone(),
            );

            commands.entity(popup_entity).insert(DreamGainPopup {
                floating_text: FloatingText {
                    lifetime: config.lifetime,
                    config,
                },
            });
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
