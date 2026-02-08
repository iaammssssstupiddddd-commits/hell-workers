use crate::interface::ui::{SectionFolded, UiInputState, UiNodeRegistry, UnassignedFolded, UiTheme};
use crate::systems::GameSystemSet;
use bevy::prelude::*;

pub struct UiFoundationPlugin;

impl Plugin for UiFoundationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SectionFolded>();
        app.register_type::<UnassignedFolded>();
        app.init_resource::<UiInputState>();
        app.init_resource::<UiNodeRegistry>();
        app.init_resource::<UiTheme>();
        app.add_systems(
            PreUpdate,
            crate::interface::ui::update_ui_input_state_system.in_set(GameSystemSet::Interface),
        );
    }
}
