//! 入力関連のプラグイン

use crate::interface::selection::handle_mouse_input;
use crate::interface::ui::UiInputState;
use crate::systems::GameSystemSet;
use bevy::camera_controller::pan_camera::{PanCamera, PanCameraPlugin};
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_core::quality::QualitySettings;
use hw_ui::camera::MainCamera;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PanCameraPlugin);
        app.add_systems(
            PreUpdate,
            pan_camera_ui_guard_system.in_set(GameSystemSet::Input),
        );
        app.add_systems(
            Update,
            (
                handle_mouse_input.run_if(in_state(PlayMode::Normal)),
                debug_toggle_system,
                render3d_toggle_system,
                rtt_quality_cycle_system,
                soul_mask_toggle_system,
                rtt_directional_light_toggle_system,
                rtt_extra_directional_light_toggle_system,
                rtt_terrain_toggle_system,
                rtt_scene_objects_toggle_system,
            )
                .in_set(GameSystemSet::Input),
        );
    }
}

/// UI パネル上にカーソルがある間は PanCamera を無効化する
fn pan_camera_ui_guard_system(
    mut q_camera: Query<&mut PanCamera, With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
) {
    if let Ok(mut pan_camera) = q_camera.single_mut() {
        pan_camera.enabled = !ui_input_state.pointer_over_ui;
    }
}

/// F3キーで 3D表示をトグル
fn render3d_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut render3d: ResMut<crate::Render3dVisible>,
) {
    if buttons.just_pressed(KeyCode::F3) {
        render3d.0 = !render3d.0;
    }
}

/// F4キーで RtT 品質を High -> Medium -> Low で循環させる。
fn rtt_quality_cycle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut quality: ResMut<QualitySettings>,
) {
    if buttons.just_pressed(KeyCode::F4) {
        quality.rtt = quality.rtt.next();
        info!("RTT quality changed: {:?}", quality.rtt);
    }
}

/// F5 キーで Soul mask RtT をトグルする。
fn soul_mask_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if buttons.just_pressed(KeyCode::F5) {
        perf_toggles.soul_mask_enabled = !perf_toggles.soul_mask_enabled;
        info!("Soul mask RtT enabled: {}", perf_toggles.soul_mask_enabled);
    }
}

/// F6 キーで RtT 用 DirectionalLight をトグルする。
fn rtt_directional_light_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if buttons.just_pressed(KeyCode::F6) {
        perf_toggles.directional_light_enabled = !perf_toggles.directional_light_enabled;
        info!(
            "RtT directional light enabled: {}",
            perf_toggles.directional_light_enabled
        );
    }
}

/// F9 キーで追加の RtT DirectionalLight をトグルする。
fn rtt_extra_directional_light_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if buttons.just_pressed(KeyCode::F9) {
        perf_toggles.extra_directional_light_enabled =
            !perf_toggles.extra_directional_light_enabled;
        info!(
            "RtT extra directional light enabled: {}",
            perf_toggles.extra_directional_light_enabled
        );
    }
}

/// F7 キーで RtT terrain をトグルする。
fn rtt_terrain_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if buttons.just_pressed(KeyCode::F7) {
        perf_toggles.terrain_enabled = !perf_toggles.terrain_enabled;
        info!("RtT terrain enabled: {}", perf_toggles.terrain_enabled);
    }
}

/// F8 キーで RtT scene object をトグルする。
fn rtt_scene_objects_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if buttons.just_pressed(KeyCode::F8) {
        perf_toggles.scene_objects_enabled = !perf_toggles.scene_objects_enabled;
        info!(
            "RtT scene objects enabled: {}",
            perf_toggles.scene_objects_enabled
        );
    }
}

/// F12キーでデバッグ情報の表示をトグル
pub fn debug_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<crate::DebugVisible>,
    mut config_store: ResMut<GizmoConfigStore>,
) {
    if buttons.just_pressed(KeyCode::F12) {
        visible.0 = !visible.0;
        for (_, config, _) in config_store.iter_mut() {
            config.enabled = visible.0;
        }
    }
}
