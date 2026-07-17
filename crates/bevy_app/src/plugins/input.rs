//! 入力関連のプラグイン

use crate::input_actions::{
    InputAction, InputPreUpdateSet, InputResolutionSet, PendingWorldInputCapture,
    ResolvedInputFrame, cancel_or_close_input_action_system, configure_input_resolution_sets,
    input_action_to_ui_intent_system, request_capture_from_menu_buttons_system,
    request_capture_from_resolved_actions_system, reset_pending_world_input_capture_system,
    resolve_input_frame_system, rollback_in_progress_gesture_system,
    sync_world_input_capture_system,
};
use crate::interface::selection::handle_mouse_input;
use crate::interface::ui::UiInputState;
use bevy::camera_controller::pan_camera::{PanCamera, PanCameraPlugin};
use bevy::picking::PickingSystems;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_core::quality::QualitySettings;
use hw_ui::camera::MainCamera;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PanCameraPlugin);
        app.init_resource::<ResolvedInputFrame>();
        app.init_resource::<PendingWorldInputCapture>();
        app.init_resource::<UiInputState>();
        configure_input_resolution_sets(app);
        app.add_systems(
            PreUpdate,
            (
                (
                    reset_pending_world_input_capture_system,
                    request_capture_from_menu_buttons_system,
                )
                    .chain()
                    .in_set(InputPreUpdateSet::CaptureRequest),
                resolve_input_frame_system.in_set(InputPreUpdateSet::Resolve),
                (
                    request_capture_from_resolved_actions_system,
                    sync_world_input_capture_system,
                )
                    .chain()
                    .in_set(InputPreUpdateSet::CaptureTransition),
                rollback_in_progress_gesture_system.in_set(InputPreUpdateSet::Rollback),
                pan_camera_ui_guard_system
                    .in_set(InputPreUpdateSet::CameraGuard)
                    .before(PickingSystems::Hover),
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
                .in_set(InputResolutionSet::Consume),
        );
    }
}

/// UI が world input を遮断している間、またはテキスト入力中は PanCamera を無効化する。
fn pan_camera_ui_guard_system(
    mut q_camera: Query<&mut PanCamera, With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
) {
    if let Ok(mut pan_camera) = q_camera.single_mut() {
        pan_camera.enabled = !ui_input_state.world_input_blocked()
            && !hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state);
    }
}

/// F3キーで 3D表示をトグル
fn render3d_toggle_system(
    resolved_frame: Res<ResolvedInputFrame>,
    mut render3d: ResMut<crate::Render3dVisible>,
) {
    if resolved_frame.contains(InputAction::ToggleRender3d) {
        render3d.0 = !render3d.0;
    }
}

/// F4キーで RtT 品質を High -> Medium -> Low で循環させる。
fn rtt_quality_cycle_system(
    resolved_frame: Res<ResolvedInputFrame>,
    mut quality: ResMut<QualitySettings>,
) {
    if resolved_frame.contains(InputAction::CycleRttQuality) {
        quality.rtt = quality.rtt.next();
        info!("RTT quality changed: {:?}", quality.rtt);
    }
}

/// F6 キーで RtT 用 DirectionalLight をトグルする。
fn rtt_directional_light_toggle_system(
    resolved_frame: Res<ResolvedInputFrame>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if resolved_frame.contains(InputAction::ToggleRttDirectionalLight) {
        perf_toggles.directional_light_enabled = !perf_toggles.directional_light_enabled;
        info!(
            "RtT directional light enabled: {}",
            perf_toggles.directional_light_enabled
        );
    }
}

/// F7 キーで RtT terrain をトグルする。
fn rtt_terrain_toggle_system(
    resolved_frame: Res<ResolvedInputFrame>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if resolved_frame.contains(InputAction::ToggleRttTerrain) {
        perf_toggles.terrain_enabled = !perf_toggles.terrain_enabled;
        info!("RtT terrain enabled: {}", perf_toggles.terrain_enabled);
    }
}

/// F8 キーで RtT scene object をトグルする。
fn rtt_scene_objects_toggle_system(
    resolved_frame: Res<ResolvedInputFrame>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    if resolved_frame.contains(InputAction::ToggleRttSceneObjects) {
        perf_toggles.scene_objects_enabled = !perf_toggles.scene_objects_enabled;
        info!(
            "RtT scene objects enabled: {}",
            perf_toggles.scene_objects_enabled
        );
    }
}

/// F12キーでデバッグ情報の表示をトグル
pub fn debug_toggle_system(
    resolved_frame: Res<ResolvedInputFrame>,
    mut visible: ResMut<crate::DebugVisible>,
    mut config_store: ResMut<GizmoConfigStore>,
    mut settings: ResMut<hw_core::GameSettings>,
    q_checkboxes: Query<(Entity, &hw_ui::components::SettingsCheckboxMarker)>,
    mut commands: Commands,
) {
    if resolved_frame.contains(InputAction::ToggleDebug) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::InputModifiers;
    use crate::test_support::minimal_app;

    #[test]
    fn resolved_render_debug_actions_reach_each_existing_owner() {
        let mut app = minimal_app();
        app.init_resource::<ResolvedInputFrame>()
            .init_resource::<crate::Render3dVisible>()
            .init_resource::<QualitySettings>()
            .init_resource::<crate::RenderPerfToggles>()
            .init_resource::<crate::DebugVisible>()
            .init_resource::<GizmoConfigStore>()
            .init_resource::<hw_core::GameSettings>()
            .add_systems(
                Update,
                (
                    render3d_toggle_system,
                    rtt_quality_cycle_system,
                    rtt_directional_light_toggle_system,
                    rtt_terrain_toggle_system,
                    rtt_scene_objects_toggle_system,
                    debug_toggle_system,
                ),
            );
        let expected_quality = app.world().resource::<QualitySettings>().rtt.next();
        let initial_directional = app
            .world()
            .resource::<crate::RenderPerfToggles>()
            .directional_light_enabled;
        let initial_terrain = app
            .world()
            .resource::<crate::RenderPerfToggles>()
            .terrain_enabled;
        let initial_scene_objects = app
            .world()
            .resource::<crate::RenderPerfToggles>()
            .scene_objects_enabled;
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![
                    InputAction::ToggleRender3d,
                    InputAction::CycleRttQuality,
                    InputAction::ToggleRttDirectionalLight,
                    InputAction::ToggleRttTerrain,
                    InputAction::ToggleRttSceneObjects,
                    InputAction::ToggleDebug,
                ],
                None,
                false,
            );

        app.update();

        assert!(!app.world().resource::<crate::Render3dVisible>().0);
        assert_eq!(
            app.world().resource::<QualitySettings>().rtt,
            expected_quality
        );
        let perf = app.world().resource::<crate::RenderPerfToggles>();
        assert_eq!(perf.directional_light_enabled, !initial_directional);
        assert_eq!(perf.terrain_enabled, !initial_terrain);
        assert_eq!(perf.scene_objects_enabled, !initial_scene_objects);
        assert!(app.world().resource::<crate::DebugVisible>().0);
        assert!(
            app.world()
                .resource::<hw_core::GameSettings>()
                .debug_gizmos_enabled
        );
    }

    #[test]
    fn pan_camera_capture_guard_restores_enabled_without_changing_mouse_setting() {
        let mut app = minimal_app();
        app.init_resource::<UiInputState>()
            .add_systems(Update, pan_camera_ui_guard_system);
        let mut controller = PanCamera::default();
        controller.mouse_pan_settings.enabled = false;
        let camera = app.world_mut().spawn((controller, MainCamera)).id();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = true;

        app.update();

        let controller = app.world().entity(camera).get::<PanCamera>().unwrap();
        assert!(!controller.enabled);
        assert!(!controller.mouse_pan_settings.enabled);

        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = false;
        app.update();

        let controller = app.world().entity(camera).get::<PanCamera>().unwrap();
        assert!(controller.enabled);
        assert!(!controller.mouse_pan_settings.enabled);
    }
}
