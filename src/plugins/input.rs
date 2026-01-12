//! 入力関連のプラグイン

use crate::game_state::PlayMode;
use crate::interface::camera::{camera_movement, camera_zoom};
use crate::interface::selection::{build_mode_cancel_system, handle_mouse_input};
use crate::systems::GameSystemSet;
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::input::{
    egui_wants_any_keyboard_input, egui_wants_any_pointer_input,
};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                camera_movement.run_if(not(egui_wants_any_pointer_input)),
                camera_zoom.run_if(not(egui_wants_any_pointer_input)),
                handle_mouse_input
                    .run_if(in_state(PlayMode::Normal).and(not(egui_wants_any_pointer_input))),
                build_mode_cancel_system.run_if(not(egui_wants_any_keyboard_input)),
                debug_inspector_toggle_system,
            )
                .in_set(GameSystemSet::Input),
        );
    }
}

/// F12キーでデバッグインスペクタの表示をトグル
pub fn debug_inspector_toggle_system(
    buttons: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<crate::DebugInspectorVisible>,
) {
    if buttons.just_pressed(KeyCode::F12) {
        visible.0 = !visible.0;
        info!("DEBUG_INSPECTOR: Visible = {}", visible.0);
    }
}
