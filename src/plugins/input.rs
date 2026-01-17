//! 入力関連のプラグイン

use crate::game_state::PlayMode;
use crate::interface::camera::PanCameraPlugin;
use crate::interface::selection::{build_mode_cancel_system, handle_mouse_input};
use crate::systems::GameSystemSet;
use bevy::prelude::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PanCameraPlugin);
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
