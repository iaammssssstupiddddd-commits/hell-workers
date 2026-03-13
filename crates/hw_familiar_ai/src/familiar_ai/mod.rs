use bevy::prelude::*;

use hw_core::system_sets::FamiliarAiSystemSet;

pub mod decide;
pub mod execute;
pub mod perceive;

pub struct FamiliarAiCorePlugin;

impl Plugin for FamiliarAiCorePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<decide::encouragement::EncouragementCooldown>()
            .add_systems(
                Update,
                (
                    perceive::state_detection::detect_state_changes_system,
                    perceive::state_detection::detect_command_changes_system,
                )
                    .in_set(FamiliarAiSystemSet::Perceive),
            )
            .add_systems(
                Update,
                (
                    decide::following::following_familiar_system,
                    decide::state_decision::familiar_ai_state_system,
                )
                    .in_set(FamiliarAiSystemSet::Decide),
            )
            .add_systems(
                Update,
                (
                    execute::state_apply::familiar_state_apply_system,
                    execute::state_log::handle_state_changed_system,
                )
                    .in_set(FamiliarAiSystemSet::Execute),
            );
    }
}
