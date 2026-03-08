use crate::interface::ui::update_ui_input_state_system;
use crate::systems::GameSystemSet;
use bevy::prelude::*;
use hw_ui::plugins::foundation::UiFoundationPlugin as HwUiFoundationPlugin;

pub struct UiFoundationPlugin;

impl Plugin for UiFoundationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HwUiFoundationPlugin);
        app.add_systems(
            PreUpdate,
            update_ui_input_state_system.in_set(GameSystemSet::Interface),
        );
    }
}
