//! ゲームロジック関連のプラグイン

use crate::entities::familiar::{familiar_movement, familiar_spawning_system};
use crate::game_state::PlayMode;
use crate::systems::GameSystemSet;
use crate::systems::command::{
    assign_task_system, familiar_command_input_system, task_area_selection_system,
};
use crate::systems::fatigue::{fatigue_penalty_system, fatigue_update_system};
use crate::systems::idle::{gathering_separation_system, idle_behavior_system};
use crate::systems::motivation::motivation_system;
use crate::systems::stress::{stress_system, supervision_stress_system};
use crate::systems::task_execution::task_execution_system;
use crate::systems::task_queue::queue_management_system;
use crate::systems::work::cleanup_commanded_souls_system;
use bevy::prelude::*;

pub struct LogicPlugin;

impl Plugin for LogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                cleanup_commanded_souls_system,
                queue_management_system,
                task_execution_system,
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
                motivation_system,
                fatigue_update_system,
                fatigue_penalty_system,
                idle_behavior_system,
                gathering_separation_system,
                familiar_spawning_system,
                stress_system,
                supervision_stress_system,
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
