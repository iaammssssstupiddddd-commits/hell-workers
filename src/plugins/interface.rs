//! インターフェース関連のプラグイン

use crate::systems::GameSystemSet;
use crate::interface::ui::plugins;
use bevy::prelude::*;
use crate::plugins::interface_debug::debug_spawn_system;

pub struct InterfacePlugin;

impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        plugins::register_ui_plugins(app);
        app.add_systems(
            Update,
            debug_spawn_system
                .run_if(|debug: Res<crate::DebugVisible>| debug.0)
                .in_set(GameSystemSet::Interface),
        );
    }
}
