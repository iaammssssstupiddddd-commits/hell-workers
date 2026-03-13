use bevy::prelude::*;
use hw_core::events::DriftingEscapeStarted;
use hw_soul_ai::soul_ai::helpers::drifting::choose_drift_edge;
use rand::Rng;

use crate::entities::damned_soul::spawn::PopulationManager;
use crate::entities::damned_soul::{DriftPhase, DriftingState, IdleBehavior, IdleState};
use hw_core::relationships::{CommandedBy, ParticipatingIn, RestingIn};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::world::map::WorldMap;
use hw_core::constants::*;

#[derive(Resource)]
pub struct DriftingDecisionTimer {
    pub timer: Timer,
}

impl Default for DriftingDecisionTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(SOUL_ESCAPE_CHECK_INTERVAL, TimerMode::Repeating),
        }
    }
}

/// 未管理状態の Soul を漂流（自然脱走）へ遷移させる
pub fn drifting_decision_system(
    time: Res<Time>,
    mut commands: Commands,
    mut timer: ResMut<DriftingDecisionTimer>,
    population: Res<PopulationManager>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut IdleState,
            &mut crate::entities::damned_soul::Destination,
            &mut crate::entities::damned_soul::Path,
            &AssignedTask,
            Option<&CommandedBy>,
            Option<&RestingIn>,
            Option<&ParticipatingIn>,
            Option<&DriftingState>,
        ),
        With<crate::entities::damned_soul::DamnedSoul>,
    >,
) {
    if !timer.timer.tick(time.delta()).just_finished() {
        return;
    }
    if !population.can_start_escape() {
        return;
    }

    let mut rng = rand::thread_rng();

    for (
        entity,
        transform,
        mut idle,
        mut destination,
        mut path,
        task,
        under_command,
        resting_in,
        participating_in,
        drifting_state,
    ) in q_souls.iter_mut()
    {
        if drifting_state.is_some() {
            continue;
        }
        if under_command.is_some() || !matches!(*task, AssignedTask::None) || resting_in.is_some() {
            continue;
        }
        if matches!(
            idle.behavior,
            IdleBehavior::Resting
                | IdleBehavior::GoingToRest
                | IdleBehavior::ExhaustedGathering
                | IdleBehavior::Escaping
        ) {
            continue;
        }
        if idle.total_idle_time < SOUL_ESCAPE_UNMANAGED_TIME {
            continue;
        }
        if !rng.gen_bool(SOUL_ESCAPE_CHANCE_PER_CHECK) {
            continue;
        }

        if participating_in.is_some() {
            commands.entity(entity).remove::<ParticipatingIn>();
            commands.trigger(crate::events::OnGatheringLeft { entity });
        }

        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        let drifting = DriftingState {
            target_edge: choose_drift_edge(grid),
            phase: DriftPhase::Wandering,
            phase_timer: 0.0,
            phase_duration: rng.gen_range(DRIFT_WANDER_DURATION_MIN..DRIFT_WANDER_DURATION_MAX),
        };

        idle.behavior = IdleBehavior::Drifting;
        idle.idle_timer = 0.0;
        idle.behavior_duration = drifting.phase_duration;
        destination.0 = transform.translation.truncate();
        path.waypoints.clear();
        path.current_index = 0;

        commands.entity(entity).insert(drifting);
        commands.trigger(DriftingEscapeStarted);

        info!(
            "SOUL_DRIFT: {:?} started drifting toward {:?}",
            entity, drifting.target_edge
        );
        break;
    }
}
