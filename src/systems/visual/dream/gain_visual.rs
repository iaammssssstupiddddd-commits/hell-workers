//! Dream 獲得時のビジュアル表現（UIパーティクル・ポップアップテキスト）
//!
//! 今後 Dream 獲得表現を拡張する際はこのファイルを編集してください。

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

/// 睡眠中の Soul からポップアップテキストおよび UI パーティクルを生成するシステム。
/// Dream 獲得量が `DREAM_POPUP_THRESHOLD` を超えるたびに発火する。
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
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    q_ui_root: Query<Entity, With<crate::interface::ui::components::UiRoot>>,
    ui_nodes: Res<crate::interface::ui::components::UiNodeRegistry>,
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

    if let Some(entity) = ui_nodes.get_slot(crate::interface::ui::components::UiSlot::DreamPoolIcon) {
        if let Ok((computed, transform)) = q_ui_transform.get(entity) {
            let center = transform.translation * computed.inverse_scale_factor();
            target_pos = center;
        }
    }

    for (transform, idle, dream, participating_in, mut visual_state) in q_souls.iter_mut() {
        let is_sleeping = idle.behavior == IdleBehavior::Sleeping
            || (idle.behavior == IdleBehavior::Gathering
                && idle.gathering_behavior == GatheringBehavior::Sleeping
                && participating_in.is_some());
        let gain_rate = dream_gain_rate(dream.quality);

        if !is_sleeping || gain_rate <= 0.0 {
            // 状態が切り替わった（起きる等）場合、持っている蓄積を開放する
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
                    &format!("+{:.1} Dream", amount),
                    popup_pos,
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

                if let Ok(start_pos) = camera.world_to_viewport(camera_transform, popup_pos) {
                    super::ui_particle::spawn_ui_particle(
                        &mut commands,
                        start_pos,
                        target_pos,
                        ui_root,
                        &assets,
                        amount,
                    );
                }
            }

            visual_state.popup_timer = 0.0;
            continue;
        }

        visual_state.popup_accumulated += gain_rate * dt;

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
                    assets.font_ui.clone(),
                );

                commands.entity(popup_entity).insert(DreamGainPopup {
                    floating_text: FloatingText {
                        lifetime: config.lifetime,
                        config,
                    },
                });

                // UIパーティクルの発生
                if let Ok(start_pos) = camera.world_to_viewport(camera_transform, popup_pos) {
                    super::ui_particle::spawn_ui_particle(
                        &mut commands,
                        start_pos,
                        target_pos,
                        ui_root,
                        &assets,
                        amount,
                    );
                }
            }
        }
    }
}

/// Dream 獲得ポップアップの表示更新（フェードアウト・移動）システム
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
