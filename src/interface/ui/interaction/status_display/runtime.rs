//! fps / speed button highlight の中継レイヤー（hw_ui 側実装へ委譲）

use crate::interface::ui::components::{SpeedButtonMarker, UiNodeRegistry};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

pub fn update_fps_display_system(
    time: Res<Time>,
    fps_counter: Local<hw_ui::interaction::status_display::FpsCounter>,
    ui_nodes: Res<UiNodeRegistry>,
    q_text: Query<&mut Text>,
) {
    hw_ui::interaction::status_display::update_fps_display_system(
        time,
        fps_counter,
        ui_nodes,
        q_text,
    );
}

pub fn update_speed_button_highlight_system(
    time: Res<Time<Virtual>>,
    theme: Res<UiTheme>,
    q_buttons: Query<(&SpeedButtonMarker, &mut BackgroundColor, &mut BorderColor)>,
) {
    hw_ui::interaction::status_display::update_speed_button_highlight_system(
        time, theme, q_buttons,
    );
}
