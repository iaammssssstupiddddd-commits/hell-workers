use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Virtual;

use hw_core::constants::REST_AREA_CAPACITY;
use hw_core::relationships::{RestAreaReservedFor, RestingIn, WorkingOn};
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};
use hw_jobs::{ActiveTaskIdentity, AssignedTask};
use hw_jobs::{Building, BuildingType, RestArea};

#[cfg(feature = "profiling")]
use super::slow_simulation::SlowSimulationPerfMetrics;

/// Keeps expensive full-world consistency sweeps off the steady-state path.
/// Relevant relationship/building transitions wake the audit immediately;
/// otherwise it runs at most once per virtual second as a defensive fallback.
/// `IdleState` timer writes are intentionally excluded from this trigger.
#[derive(Resource)]
pub struct StateSanityAudit {
    timer: Timer,
    dirty: bool,
    due: bool,
}

impl Default for StateSanityAudit {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            dirty: true,
            due: false,
        }
    }
}

// Task progress updates `AssignedTask` frequently. Assignment
// identity/relationship boundaries are the invariant-relevant events; treating
// every progress write as a wake-up would put the full consistency sweep back
// on the 60 Hz path.
type TaskSanityDirtyQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        Added<AssignedTask>,
        Added<WorkingOn>,
        Added<ActiveTaskIdentity>,
    )>,
>;

type BuildingSanityDirtyQuery<'w, 's> = Query<'w, 's, (), Or<(Added<Building>, Changed<Building>)>>;
type RestSanityDirtyQuery<'w, 's> =
    Query<'w, 's, (), Or<(Added<RestingIn>, Added<RestAreaReservedFor>)>>;

#[derive(SystemParam)]
pub(crate) struct StateSanitySignals<'w, 's> {
    q_task_dirty: TaskSanityDirtyQuery<'w, 's>,
    q_building_dirty: BuildingSanityDirtyQuery<'w, 's>,
    q_rest_dirty: RestSanityDirtyQuery<'w, 's>,
    removed_working: RemovedComponents<'w, 's, WorkingOn>,
    removed_identity: RemovedComponents<'w, 's, ActiveTaskIdentity>,
    removed_resting: RemovedComponents<'w, 's, RestingIn>,
    removed_reserved: RemovedComponents<'w, 's, RestAreaReservedFor>,
}

pub(crate) fn update_state_sanity_trigger_system(
    time: Res<Time<Virtual>>,
    mut audit: ResMut<StateSanityAudit>,
    mut signals: StateSanitySignals,
) {
    if audit.timer.tick(time.delta()).just_finished() {
        audit.due = true;
    }
    let removed_any = signals.removed_working.read().count() != 0
        || signals.removed_identity.read().count() != 0
        || signals.removed_resting.read().count() != 0
        || signals.removed_reserved.read().count() != 0;
    audit.dirty |= !signals.q_task_dirty.is_empty()
        || !signals.q_building_dirty.is_empty()
        || !signals.q_rest_dirty.is_empty()
        || removed_any;
}

pub fn state_sanity_should_run(audit: Res<StateSanityAudit>) -> bool {
    audit.dirty || audit.due
}

pub fn clear_state_sanity_trigger_system(
    mut audit: ResMut<StateSanityAudit>,
    #[cfg(feature = "profiling")] mut metrics: ResMut<SlowSimulationPerfMetrics>,
) {
    audit.dirty = false;
    audit.due = false;
    #[cfg(feature = "profiling")]
    {
        metrics.state_sanity_audits = metrics.state_sanity_audits.saturating_add(1);
    }
}

/// AssignedTask が None なのに WorkingOn が残っている不整合を解消する。
pub fn clear_stale_working_on_system(
    mut commands: Commands,
    q_souls: Query<(Entity, &AssignedTask), With<WorkingOn>>,
) {
    for (entity, task) in q_souls.iter() {
        if matches!(task, AssignedTask::None) {
            commands.entity(entity).remove::<WorkingOn>();
        }
    }
}

/// AssignedTask が None なのに runtime identity が残っている不整合を解消する。
pub fn clear_stale_task_identity_system(
    mut commands: Commands,
    q_souls: Query<(Entity, &AssignedTask), With<ActiveTaskIdentity>>,
) {
    for (entity, task) in q_souls.iter() {
        if matches!(task, AssignedTask::None) {
            commands.entity(entity).remove::<ActiveTaskIdentity>();
        }
    }
}

/// Building.kind と RestArea コンポーネントの整合性を保つ。
pub fn ensure_rest_area_component_system(
    mut commands: Commands,
    q_buildings: Query<(Entity, &Building, Option<&RestArea>)>,
) {
    for (entity, building, rest_area_opt) in q_buildings.iter() {
        if building.kind == BuildingType::RestArea && rest_area_opt.is_none() {
            commands.entity(entity).insert(RestArea {
                capacity: REST_AREA_CAPACITY,
            });
        }
    }
}

/// 休憩状態と休憩リレーションの整合性を保つ。
pub fn reconcile_rest_state_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &mut IdleState,
        Option<&RestingIn>,
        Option<&RestAreaReservedFor>,
    )>,
    mut q_visibility: Query<&mut Visibility, With<DamnedSoul>>,
) {
    for (entity, mut idle, resting_in, reserved_for) in q_souls.iter_mut() {
        if idle.behavior == IdleBehavior::Resting && resting_in.is_none() {
            idle.behavior = if reserved_for.is_some() {
                IdleBehavior::GoingToRest
            } else {
                IdleBehavior::Wandering
            };
            idle.idle_timer = 0.0;
        }

        if resting_in.is_none()
            && reserved_for.is_some()
            && !matches!(
                idle.behavior,
                IdleBehavior::GoingToRest | IdleBehavior::Resting
            )
        {
            commands.entity(entity).remove::<RestAreaReservedFor>();
        }

        if idle.behavior != IdleBehavior::Resting
            && (resting_in.is_some() || reserved_for.is_some())
            && matches!(
                idle.behavior,
                IdleBehavior::Escaping | IdleBehavior::Drifting | IdleBehavior::ExhaustedGathering
            )
        {
            commands
                .entity(entity)
                .remove::<(RestingIn, RestAreaReservedFor)>();
            if let Ok(mut visibility) = q_visibility.get_mut(entity) {
                *visibility = Visibility::Visible;
            }
        }
    }
}
