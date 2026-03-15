//! 入力関連のプラグイン

use crate::interface::selection::handle_mouse_input;
use crate::interface::ui::UiInputState;
use crate::systems::GameSystemSet;
use bevy::camera_controller::pan_camera::{PanCamera, PanCameraPlugin};
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
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
