//! 入力関連のプラグイン

use crate::app_contexts::TaskContext;
use crate::entities::familiar::Familiar;
use crate::input_actions::{
    InputAction, InputPreUpdateSet, InputResolutionSet, PendingWorldInputCapture,
    ResolvedInputFrame, cancel_or_close_input_action_system, configure_input_resolution_sets,
    input_action_to_ui_intent_system, request_capture_from_menu_buttons_system,
    request_capture_from_resolved_actions_system, reset_pending_world_input_capture_system,
    resolve_input_frame_system, rollback_in_progress_gesture_system,
    sync_world_input_capture_system,
};
use crate::interface::selection::{
    SelectedEntity, handle_mouse_input, pointer_hits_task_area_border,
};
use crate::interface::ui::UiInputState;
use crate::systems::command::TaskArea;
use bevy::camera_controller::pan_camera::{PanCamera, PanCameraPlugin};
use bevy::ecs::system::SystemParam;
use bevy::picking::PickingSystems;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::game_state::{PlayMode, TaskMode};
use hw_core::quality::QualitySettings;
use hw_ui::camera::MainCamera;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PanCameraPlugin);
        app.init_resource::<ResolvedInputFrame>();
        app.init_resource::<PendingWorldInputCapture>();
        app.init_resource::<UiInputState>();
        app.init_resource::<TaskAreaPointerClaim>();
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
                pan_camera_world_input_guard_system
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

#[derive(Resource, Debug, Default)]
struct TaskAreaPointerClaim {
    active_until_release: bool,
}

impl TaskAreaPointerClaim {
    fn update(
        &mut self,
        starts_task_area_drag: bool,
        mouse_buttons: &ButtonInput<MouseButton>,
    ) -> bool {
        let left_active = mouse_buttons.pressed(MouseButton::Left)
            || mouse_buttons.just_released(MouseButton::Left);
        if !left_active {
            self.active_until_release = false;
            return false;
        }

        self.active_until_release |= starts_task_area_drag;
        let blocks_pan_camera = self.active_until_release;
        if mouse_buttons.just_released(MouseButton::Left) {
            self.active_until_release = false;
        }
        blocks_pan_camera
    }
}

#[derive(SystemParam)]
struct PanCameraGuardParams<'w, 's> {
    q_pan_camera: Query<'w, 's, &'static mut PanCamera, With<MainCamera>>,
    ui_input_state: Res<'w, UiInputState>,
    task_context: Res<'w, TaskContext>,
    mouse_buttons: Res<'w, ButtonInput<MouseButton>>,
    resolved_frame: Res<'w, ResolvedInputFrame>,
    play_mode: Res<'w, State<PlayMode>>,
    next_play_mode: Res<'w, NextState<PlayMode>>,
    selected: Res<'w, SelectedEntity>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_world_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    q_task_areas: Query<'w, 's, (Entity, &'static TaskArea), With<Familiar>>,
    task_area_pointer_claim: ResMut<'w, TaskAreaPointerClaim>,
}

/// UI / text input が world input を遮断している間、または task area が一次ポインタを
/// release まで所有している間は PanCamera を無効化する。
fn pan_camera_world_input_guard_system(mut params: PanCameraGuardParams) {
    let direct_area_selection_press = params.mouse_buttons.just_pressed(MouseButton::Left)
        && normal_pointer_ingress_will_run(&params.play_mode, &params.next_play_mode)
        && !params.ui_input_state.world_input_blocked()
        && !params.resolved_frame.pointer_selection_suppressed()
        && hw_ui::camera::world_cursor_pos(&params.q_window, &params.q_world_camera).is_some_and(
            |world_pos| {
                pointer_hits_task_area_border(world_pos, params.selected.0, &params.q_task_areas)
            },
        );
    let starts_task_area_drag = task_mode_uses_area_drag(params.task_context.0)
        || resolved_action_starts_task_area_drag(&params.resolved_frame)
        || direct_area_selection_press;
    let task_area_drag_claimed = params
        .task_area_pointer_claim
        .update(starts_task_area_drag, &params.mouse_buttons);

    if let Ok(mut pan_camera) = params.q_pan_camera.single_mut() {
        pan_camera.enabled = !params.ui_input_state.world_input_blocked()
            && !hw_ui::interaction::text_input_blocks_keybinds(&params.ui_input_state)
            && !task_area_drag_claimed;
    }
}

/// `StateTransition` は CameraGuard より後かつ Update より前に走るため、world selection の
/// run condition は pending state を含む「次の Update での値」で判定する。
fn normal_pointer_ingress_will_run(
    play_mode: &State<PlayMode>,
    next_play_mode: &NextState<PlayMode>,
) -> bool {
    match next_play_mode {
        NextState::Pending(mode) | NextState::PendingIfNeq(mode) => mode == &PlayMode::Normal,
        NextState::Unchanged => play_mode.get() == &PlayMode::Normal,
    }
}

/// 左ボタンを world のエリア指定 gesture に予約する task mode。
///
/// `None` 状態も含め、すでに active な mode の押下 frame から claim を開始する。
fn task_mode_uses_area_drag(task_mode: TaskMode) -> bool {
    matches!(
        task_mode,
        TaskMode::DesignateChop(_)
            | TaskMode::DesignateMine(_)
            | TaskMode::DesignateHaul(_)
            | TaskMode::CancelDesignation(_)
            | TaskMode::AreaSelection(_)
            | TaskMode::AssignTask(_)
            | TaskMode::ZonePlacement(_, _)
            | TaskMode::ZoneRemoval(_, _)
            | TaskMode::FloorPlace(_)
            | TaskMode::WallPlace(_)
            | TaskMode::DreamPlanting(_)
    )
}

/// Resolver が同 frame の後段で task area mode を開始する action。
fn resolved_action_starts_task_area_drag(resolved_frame: &ResolvedInputFrame) -> bool {
    resolved_frame.actions().iter().copied().any(|action| {
        matches!(
            action,
            InputAction::FamiliarChop
                | InputAction::FamiliarMine
                | InputAction::FamiliarHaul
                | InputAction::FamiliarCancelDesignation
        )
    })
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
#[path = "input_tests.rs"]
mod tests;
