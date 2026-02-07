//! 入力関連のプラグイン

use crate::game_state::PlayMode;
use crate::interface::camera::{MainCamera, PanCamera, PanCameraPlugin};
use crate::interface::selection::{build_mode_cancel_system, handle_mouse_input};
use crate::systems::GameSystemSet;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

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
                build_mode_cancel_system,
                debug_toggle_system,
            )
                .in_set(GameSystemSet::Input),
        );
    }
}

/// UI パネル上にカーソルがある間は PanCamera を無効化する
fn pan_camera_ui_guard_system(
    mut q_camera: Query<&mut PanCamera, With<MainCamera>>,
    q_blockers: Query<&RelativeCursorPosition, With<crate::interface::ui::UiInputBlocker>>,
    q_buttons: Query<&Interaction, With<Button>>,
) {
    let pointer_over_ui = q_blockers.iter().any(RelativeCursorPosition::cursor_over)
        || q_buttons
            .iter()
            .any(|interaction| matches!(*interaction, Interaction::Hovered | Interaction::Pressed));

    if let Ok(mut pan_camera) = q_camera.single_mut() {
        pan_camera.enabled = !pointer_over_ui;
    }
}

/// F12キーでデバッグ情報の表示をトグル
pub fn debug_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<crate::DebugVisible>,
) {
    if buttons.just_pressed(KeyCode::F12) {
        visible.0 = !visible.0;
        info!("DEBUG: Visible = {}", visible.0);
    }
}
