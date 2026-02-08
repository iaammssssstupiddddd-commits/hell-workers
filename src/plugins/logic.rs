//! ゲームロジック関連のプラグイン

use crate::entities::familiar::{familiar_movement, familiar_spawning_system};
use crate::game_state::PlayMode;
use crate::systems::GameSystemSet;
use crate::systems::command::{
    AreaEditClipboard, AreaEditHistory, AreaEditPresets, AreaEditSession, assign_task_system,
    familiar_command_input_system, task_area_edit_history_shortcuts_system,
    task_area_selection_system,
};
use crate::systems::obstacle::obstacle_cleanup_system;
use crate::systems::soul_ai::SoulAiPlugin;
use crate::world::regrowth::{RegrowthManager, tree_regrowth_system};
use bevy::prelude::*;

pub struct LogicPlugin;

impl Plugin for LogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SoulAiPlugin);

        // パスファインディング用の作業メモリを登録
        app.init_resource::<RegrowthManager>();
        app.init_resource::<AreaEditSession>();
        app.init_resource::<AreaEditHistory>();
        app.init_resource::<AreaEditClipboard>();
        app.init_resource::<AreaEditPresets>();

        app.add_systems(
            Update,
            (
                assign_task_system.run_if(in_state(PlayMode::TaskDesignation)),
                familiar_command_input_system.run_if(
                    |selected: Res<crate::interface::selection::SelectedEntity>| {
                        selected.0.is_some()
                    },
                ),
                task_area_selection_system.run_if(in_state(PlayMode::TaskDesignation)),
                task_area_edit_history_shortcuts_system.run_if(in_state(PlayMode::TaskDesignation)),
                familiar_spawning_system,
                tree_regrowth_system,
                obstacle_cleanup_system,
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
