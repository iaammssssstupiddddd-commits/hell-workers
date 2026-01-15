//! ゲームロジック関連のプラグイン

use crate::entities::familiar::{familiar_movement, familiar_spawning_system};
use crate::game_state::PlayMode;
use crate::systems::GameSystemSet;
use crate::systems::command::{
    assign_task_system, familiar_command_input_system, task_area_selection_system,
};
use crate::systems::soul_ai::SoulAiPlugin;
use crate::systems::task_queue::queue_management_system;
use bevy::prelude::*;

pub struct LogicPlugin;

impl Plugin for LogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SoulAiPlugin);

        app.add_systems(
            Update,
            (
                queue_management_system,
                assign_task_system.run_if(in_state(PlayMode::TaskDesignation)),
            )
                .chain()
                .in_set(GameSystemSet::Logic),
        )
        .add_systems(
            Update,
            (
                familiar_command_input_system.run_if(
                    |selected: Res<crate::interface::selection::SelectedEntity>| {
                        selected.0.is_some()
                    },
                ),
                task_area_selection_system.run_if(in_state(PlayMode::TaskDesignation)),
                familiar_spawning_system,
            )
                .chain()
                .in_set(GameSystemSet::Logic),
        )
        .add_systems(
            Update,
            (familiar_movement).chain().in_set(GameSystemSet::Actor),
        );
    }
}
