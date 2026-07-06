use bevy::prelude::*;

use crate::components::PauseMenu;

/// `Time<Virtual>` が一時停止中のときだけ Pause メニューを表示する。
pub fn update_pause_menu_visibility(
    time: Res<Time<Virtual>>,
    mut q_pause_menu: Query<&mut Node, With<PauseMenu>>,
) {
    let display = if time.is_paused() {
        Display::Flex
    } else {
        Display::None
    };

    if let Ok(mut node) = q_pause_menu.single_mut() {
        node.display = display;
    }
}
