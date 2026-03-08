//! インターフェース関連のプラグイン

use crate::systems::GameSystemSet;
use crate::interface::ui::plugins;
use bevy::prelude::*;
use crate::plugins::interface_debug::debug_spawn_system;
use hw_ui::{plugins::foundation::UiFoundationPlugin, HwUiPlugin};

pub struct InterfacePlugin;

impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((HwUiPlugin, UiFoundationPlugin))
            .add_systems(
            Update,
            debug_spawn_system
                .run_if(|debug: Res<crate::DebugVisible>| debug.0)
                .in_set(GameSystemSet::Interface),
        );
        plugins::register_ui_plugins(app);
    }
}
