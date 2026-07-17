//! 入力関連のプラグイン

use crate::input_actions::{
    InputPreUpdateSet, InputResolutionSet, ResolvedInputFrame, cancel_or_close_input_action_system,
    configure_input_resolution_sets, input_action_to_ui_intent_system, resolve_input_frame_system,
};
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
        app.init_resource::<ResolvedInputFrame>();
        configure_input_resolution_sets(app);
        app.add_systems(
            PreUpdate,
            (
                resolve_input_frame_system.in_set(InputPreUpdateSet::Resolve),
                pan_camera_ui_guard_system.in_set(GameSystemSet::Input),
            ),
        );
        app.add_systems(
            Update,
            handle_mouse_input
                .run_if(in_state(PlayMode::Normal))
                .in_set(InputResolutionSet::PointerIngress),
        );
        app.add_systems(
            Update,
            (
                cancel_or_close_input_action_system,
                input_action_to_ui_intent_system,
            )
                .chain()
                .in_set(InputResolutionSet::Consume),
        );
        app.add_systems(
            Update,
            (
                debug_toggle_system,
                render3d_toggle_system,
                rtt_quality_cycle_system,
                rtt_directional_light_toggle_system,
                rtt_terrain_toggle_system,
                rtt_scene_objects_toggle_system,
            )
                .in_set(GameSystemSet::Input),
        );
    }
}

/// UI パネル上にカーソルがある間、またはテキスト入力中は PanCamera を無効化する
fn pan_camera_ui_guard_system(
    mut q_camera: Query<&mut PanCamera, With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
) {
    if let Ok(mut pan_camera) = q_camera.single_mut() {
        pan_camera.enabled = !ui_input_state.pointer_over_ui
            && !hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state);
    }
}

/// F3キーで 3D表示をトグル
fn render3d_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    ui_input_state: Res<UiInputState>,
    mut render3d: ResMut<crate::Render3dVisible>,
) {
    if hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state) {
        return;
    }
    if buttons.just_pressed(KeyCode::F3) {
        render3d.0 = !render3d.0;
    }
}

/// F4キーで RtT 品質を High -> Medium -> Low で循環させる。
fn rtt_quality_cycle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    ui_input_state: Res<UiInputState>,
    mut quality: ResMut<QualitySettings>,
) {
    if hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state) {
        return;
    }
    if buttons.just_pressed(KeyCode::F4) {
        quality.rtt = quality.rtt.next();
        info!("RTT quality changed: {:?}", quality.rtt);
    }
}

/// F6 キーで RtT 用 DirectionalLight をトグルする。
fn rtt_directional_light_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    ui_input_state: Res<UiInputState>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state) {
        return;
    }
    if buttons.just_pressed(KeyCode::F6) {
        perf_toggles.directional_light_enabled = !perf_toggles.directional_light_enabled;
        info!(
            "RtT directional light enabled: {}",
            perf_toggles.directional_light_enabled
        );
    }
}

/// F7 キーで RtT terrain をトグルする。
fn rtt_terrain_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    ui_input_state: Res<UiInputState>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state) {
        return;
    }
    if buttons.just_pressed(KeyCode::F7) {
        perf_toggles.terrain_enabled = !perf_toggles.terrain_enabled;
        info!("RtT terrain enabled: {}", perf_toggles.terrain_enabled);
    }
}

/// F8 キーで RtT scene object をトグルする。
fn rtt_scene_objects_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    ui_input_state: Res<UiInputState>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state) {
        return;
    }
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
    ui_input_state: Res<UiInputState>,
    mut visible: ResMut<crate::DebugVisible>,
    mut config_store: ResMut<GizmoConfigStore>,
    mut settings: ResMut<hw_core::GameSettings>,
    q_checkboxes: Query<(Entity, &hw_ui::components::SettingsCheckboxMarker)>,
    mut commands: Commands,
) {
    if hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state) {
        return;
    }
    if buttons.just_pressed(KeyCode::F12) {
        visible.0 = !visible.0;
        settings.debug_gizmos_enabled = visible.0;
        for (_, config, _) in config_store.iter_mut() {
            config.enabled = visible.0;
        }

        // 設定画面の Debug Gizmos チェックボックスにも反映（Checked は widget 状態の実体）
        for (entity, marker) in q_checkboxes.iter() {
            if marker.0 == hw_ui::components::SettingsField::DebugGizmos {
                if visible.0 {
                    commands.entity(entity).insert(bevy::ui::Checked);
                } else {
                    commands.entity(entity).remove::<bevy::ui::Checked>();
                }
            }
        }
    }
}
