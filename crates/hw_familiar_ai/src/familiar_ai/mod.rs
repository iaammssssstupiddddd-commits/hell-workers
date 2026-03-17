use bevy::prelude::*;

use hw_core::system_sets::FamiliarAiSystemSet;

pub mod decide;
pub mod execute;
pub mod perceive;

pub struct FamiliarAiCorePlugin;

impl Plugin for FamiliarAiCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<decide::resources::FamiliarTaskDelegationTimer>()
            .init_resource::<decide::resources::ReachabilityFrameCache>()
            .init_resource::<decide::resources::FamiliarDelegationPerfMetrics>()
            .init_resource::<decide::blueprint_auto_gather::BlueprintAutoGatherTimer>()
            .register_type::<decide::encouragement::EncouragementCooldown>()
            .register_type::<hw_core::familiar::FamiliarAiState>()
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
                decide::following::following_familiar_system.in_set(FamiliarAiSystemSet::Decide),
            )
            .add_systems(
                Update,
                (
                    decide::state_decision::familiar_ai_state_system,
                    ApplyDeferred,
                    decide::task_delegation::familiar_task_delegation_system,
                )
                    .chain()
                    .in_set(FamiliarAiSystemSet::Decide),
            )
            .add_systems(
                Update,
                (
                    decide::blueprint_auto_gather::blueprint_auto_gather_system,
                    ApplyDeferred,
                    decide::encouragement::encouragement_decision_system,
                )
                    .chain()
                    .after(decide::state_decision::familiar_ai_state_system)
                    .in_set(FamiliarAiSystemSet::Decide),
            )
            .add_systems(
                Update,
                (
                    execute::state_apply::familiar_state_apply_system,
                    execute::state_log::handle_state_changed_system,
                )
                    .in_set(FamiliarAiSystemSet::Execute),
            )
            .add_systems(
                Update,
                (
                    execute::max_soul_logic::max_soul_logic_system,
                    execute::squad_logic::squad_logic_system,
                    execute::encouragement_apply::encouragement_apply_system,
                    execute::encouragement_apply::cleanup_encouragement_cooldowns_system,
                )
                    .in_set(FamiliarAiSystemSet::Execute),
            );
    }
}
