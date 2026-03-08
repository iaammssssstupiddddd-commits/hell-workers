use bevy::prelude::*;

use hw_core::system_sets::FamiliarAiSystemSet;

pub mod perceive;

pub struct FamiliarAiCorePlugin;

impl Plugin for FamiliarAiCorePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                perceive::state_detection::detect_state_changes_system,
                perceive::state_detection::detect_command_changes_system,
            )
                .in_set(FamiliarAiSystemSet::Perceive),
        );
    }
}
