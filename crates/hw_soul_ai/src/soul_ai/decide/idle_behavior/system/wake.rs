use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::relationships::{RestAreaReservedFor, RestingIn, WorkingOn};
use hw_core::soul::{IdleState, RestAreaCooldown};
use hw_jobs::ActiveTaskIdentity;

use crate::soul_ai::helpers::query_types::NeedsIdleDecision;

type IdleDecisionTaskStartQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<IdleState>,
        Or<(Added<WorkingOn>, Added<ActiveTaskIdentity>)>,
    ),
>;

type IdleDecisionRestChangedQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<IdleState>,
        Or<(
            Added<RestingIn>,
            Added<RestAreaReservedFor>,
            Added<RestAreaCooldown>,
        )>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct IdleDecisionWakeParams<'w, 's> {
    q_initial_idle: Query<'w, 's, Entity, Added<IdleState>>,
    q_task_started: IdleDecisionTaskStartQuery<'w, 's>,
    q_rest_changed: IdleDecisionRestChangedQuery<'w, 's>,
    q_idle: Query<'w, 's, (), With<IdleState>>,
    removed_working: RemovedComponents<'w, 's, WorkingOn>,
    removed_task_identity: RemovedComponents<'w, 's, ActiveTaskIdentity>,
    removed_resting: RemovedComponents<'w, 's, RestingIn>,
    removed_reserved: RemovedComponents<'w, 's, RestAreaReservedFor>,
    removed_cooldown: RemovedComponents<'w, 's, RestAreaCooldown>,
}

/// Marks only decision-relevant state transitions for an immediate `dt = 0`
/// reevaluation. Timer writes to `IdleState` deliberately do not wake this
/// path; cadence ticks own normal timer progression.
pub(crate) fn mark_needs_idle_decision_system(
    mut commands: Commands,
    mut wake: IdleDecisionWakeParams,
) {
    for entity in wake
        .q_initial_idle
        .iter()
        .chain(wake.q_task_started.iter())
        .chain(wake.q_rest_changed.iter())
    {
        commands.entity(entity).insert(NeedsIdleDecision);
    }
    for entity in wake
        .removed_working
        .read()
        .chain(wake.removed_task_identity.read())
        .chain(wake.removed_resting.read())
        .chain(wake.removed_reserved.read())
        .chain(wake.removed_cooldown.read())
    {
        if wake.q_idle.get(entity).is_ok() {
            commands.entity(entity).insert(NeedsIdleDecision);
        }
    }
}

/// Wake markers are one-frame notifications. Update→Decide `ApplyDeferred`
/// makes them visible to the current decision pass; this cleanup runs after it
/// so they cannot turn into a steady-state per-frame gate.
pub(crate) fn clear_idle_decision_wake_system(
    mut commands: Commands,
    q_woken: Query<Entity, With<NeedsIdleDecision>>,
) {
    for entity in q_woken.iter() {
        commands.entity(entity).remove::<NeedsIdleDecision>();
    }
}
