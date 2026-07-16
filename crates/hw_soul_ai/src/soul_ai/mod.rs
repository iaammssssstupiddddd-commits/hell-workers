use bevy::prelude::*;

use hw_core::system_sets::SoulAiSystemSet;
use hw_world::RuntimePathSearchBudget;

pub mod building_completed;
pub mod decide;
pub mod execute;
pub mod helpers;
pub mod pathfinding;
pub mod perceive;
pub mod update;

pub struct SoulAiCorePlugin;

impl Plugin for SoulAiCorePlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "profiling")]
        app.init_resource::<execute::task_execution::TaskExecutionPerfMetrics>()
            .init_resource::<pathfinding::RuntimePathDeferMetrics>()
            .init_resource::<update::slow_simulation::SlowSimulationPerfMetrics>();

        app.init_resource::<helpers::gathering::GatheringUpdateTimer>()
            .init_resource::<perceive::escaping::EscapeDetectionTimer>()
            .init_resource::<perceive::escaping::EscapeBehaviorTimer>()
            .init_resource::<decide::drifting::DriftingDecisionTimer>()
            .init_resource::<decide::work::auto_build::BlueprintAutoBuildTimer>()
            .init_resource::<update::slow_simulation::SlowSimulationClock>()
            .init_resource::<update::state_sanity::StateSanityAudit>()
            .init_resource::<RuntimePathSearchBudget>()
            .register_type::<helpers::gathering::GatheringSpot>()
            .register_type::<execute::task_execution::types::AssignedTask>()
            .add_systems(
                PreUpdate,
                pathfinding::reset_runtime_path_search_budget_system,
            )
            .add_systems(
                Update,
                execute::task_unassign_apply::handle_soul_task_unassign_system
                    .in_set(SoulAiSystemSet::Perceive),
            )
            .add_systems(
                Update,
                (
                    helpers::gathering::tick_gathering_timer_system,
                    update::gathering_tick::gathering_grace_tick_system,
                )
                    .in_set(SoulAiSystemSet::Update),
            )
            .add_systems(
                Update,
                update::state_sanity::update_state_sanity_trigger_system
                    .after(update::slow_simulation::slow_simulation_driver_system)
                    .in_set(SoulAiSystemSet::Update),
            )
            .add_systems(
                Update,
                (
                    update::state_sanity::ensure_rest_area_component_system,
                    update::state_sanity::clear_stale_working_on_system,
                    update::state_sanity::clear_stale_task_identity_system,
                    update::state_sanity::reconcile_rest_state_system,
                )
                    .run_if(update::state_sanity::state_sanity_should_run)
                    .after(update::state_sanity::update_state_sanity_trigger_system)
                    .in_set(SoulAiSystemSet::Update),
            )
            .add_systems(
                Update,
                update::state_sanity::clear_state_sanity_trigger_system
                    .run_if(update::state_sanity::state_sanity_should_run)
                    .after(update::state_sanity::reconcile_rest_state_system)
                    .in_set(SoulAiSystemSet::Update),
            )
            .add_systems(
                Update,
                (
                    update::slow_simulation::advance_slow_simulation_clock_system,
                    update::slow_simulation::slow_simulation_driver_system,
                )
                    .chain()
                    .in_set(SoulAiSystemSet::Update),
            )
            .add_systems(
                Update,
                decide::idle_behavior::mark_needs_idle_decision_system
                    .after(update::slow_simulation::slow_simulation_driver_system)
                    .before(update::state_sanity::update_state_sanity_trigger_system)
                    .in_set(SoulAiSystemSet::Update),
            )
            .add_systems(
                Update,
                (
                    execute::designation_apply::apply_designation_requests_system,
                    execute::gathering_apply::gathering_apply_system,
                    execute::gathering_spawn::gathering_spawn_logic_system,
                    execute::task_assignment_apply::apply_task_assignment_requests_system,
                    execute::drifting::drifting_behavior_system.after(
                        execute::task_assignment_apply::apply_task_assignment_requests_system,
                    ),
                    execute::drifting::despawn_at_edge_system
                        .after(execute::drifting::drifting_behavior_system),
                    // Assignment uses Commands for WorkingOn and ActiveTaskIdentity. Apply them
                    // before task execution so an accepted assignment has one coherent identity.
                    ApplyDeferred
                        .after(
                            execute::task_assignment_apply::apply_task_assignment_requests_system,
                        )
                        .after(execute::drifting::drifting_behavior_system)
                        .before(execute::task_execution_system::task_execution_system),
                    execute::task_execution_system::task_execution_system
                        .after(
                            execute::task_assignment_apply::apply_task_assignment_requests_system,
                        )
                        .after(execute::drifting::drifting_behavior_system),
                    execute::task_execution::move_plant::apply_pending_building_move_system
                        .after(execute::task_execution_system::task_execution_system),
                    execute::idle_behavior_apply::idle_behavior_apply_system,
                    execute::escaping_apply::escaping_apply_system,
                    execute::cleanup::cleanup_commanded_souls_system,
                )
                    .in_set(SoulAiSystemSet::Execute),
            )
            .add_systems(
                Update,
                (
                    decide::work::auto_refine::mud_mixer_auto_refine_system,
                    decide::work::auto_build::blueprint_auto_build_system,
                    decide::idle_behavior::idle_behavior_decision_system,
                    decide::idle_behavior::clear_idle_decision_wake_system
                        .after(decide::idle_behavior::idle_behavior_decision_system),
                    decide::separation::gathering_separation_system
                        .after(decide::idle_behavior::idle_behavior_decision_system),
                    decide::escaping::escaping_decision_system
                        .after(decide::idle_behavior::idle_behavior_decision_system),
                    decide::drifting::drifting_decision_system
                        .after(decide::escaping::escaping_decision_system),
                    decide::gathering_mgmt::gathering_maintenance_decision,
                    decide::gathering_mgmt::gathering_merge_decision,
                    decide::gathering_mgmt::gathering_recruitment_decision,
                    decide::gathering_mgmt::gathering_leave_decision,
                )
                    .in_set(SoulAiSystemSet::Decide),
            )
            .add_observer(update::vitals::on_task_completed_motivation_bonus)
            .add_observer(update::vitals::on_encouraged_effect)
            .add_observer(update::vitals::on_soul_recruited_effect)
            .add_observer(building_completed::on_building_completed)
            .add_systems(
                Update,
                (
                    pathfinding::soul_stuck_escape_system
                        .in_set(SoulAiSystemSet::Actor)
                        .before(pathfinding::pathfinding_system),
                    pathfinding::pathfinding_system.in_set(SoulAiSystemSet::Actor),
                ),
            );
    }
}
