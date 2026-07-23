//! Transport Request State Machine
//!
//! TaskWorkers の有無に基づいて TransportRequestState を自動更新します。

use bevy::prelude::*;
use hw_core::ecs::drain_removed_where;
use hw_core::relationships::TaskWorkers;

use crate::transport_request::{TransportRequest, TransportRequestState};

type StateRequestQuery<'w, 's> = Query<
    'w,
    's,
    (&'static TaskWorkers, &'static mut TransportRequestState),
    (With<TransportRequest>, Changed<TaskWorkers>),
>;

pub fn transport_request_state_sync_system(mut q_requests: StateRequestQuery) {
    for (workers, mut state) in q_requests.iter_mut() {
        if workers.is_empty() {
            if *state != TransportRequestState::Pending {
                *state = TransportRequestState::Pending;
            }
        } else if *state == TransportRequestState::Pending {
            *state = TransportRequestState::Claimed;
        }
    }
}

type ExistingTransportRequestQuery<'w, 's> = Query<'w, 's, (), With<TransportRequest>>;
type UnclaimedTransportRequestQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut TransportRequestState,
    (With<TransportRequest>, Without<TaskWorkers>),
>;

/// Reopen requests whose final worker removal also removed the relationship target.
///
/// `WorkingOn` owns `TaskWorkers`. When its final source is removed, Bevy removes the empty
/// target component, so `Changed<TaskWorkers>` cannot observe that transition.
pub fn transport_request_task_workers_reconcile_system(
    mut removed_task_workers: RemovedComponents<TaskWorkers>,
    q_existing_requests: ExistingTransportRequestQuery,
    mut q_unclaimed_requests: UnclaimedTransportRequestQuery,
) {
    let mut removed_request_workers = Vec::new();
    drain_removed_where(&mut removed_task_workers, |entity| {
        let is_transport_request = q_existing_requests.get(entity).is_ok();
        if is_transport_request {
            removed_request_workers.push(entity);
        }
        is_transport_request
    });

    for request_entity in removed_request_workers {
        if let Ok(mut state) = q_unclaimed_requests.get_mut(request_entity) {
            *state = TransportRequestState::Pending;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::ScheduleRunnerPlugin;

    use super::*;
    use crate::transport_request::arbitration::WheelbarrowArbitrationRuntime;
    use crate::transport_request::{
        TransportDemand, TransportPriority, TransportRequestKind, TransportRequestMetrics,
        WheelbarrowArbitrationDiagnostics, WheelbarrowLease, wheelbarrow_arbitration_system,
    };
    use crate::{ResourceItem, ResourceType, SharedResourceCache, Stockpile, Wheelbarrow};
    use hw_core::relationships::{ParkedAt, WorkingOn};
    use hw_core::system_sets::{GameSystemSet, SoulAiSystemSet};
    use hw_jobs::{Designation, TaskSlots, WorkType};

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    enum TestSet {
        Arbitrate,
        Reconcile,
    }

    #[derive(Resource, Default)]
    struct WorkerToRelease(Option<Entity>);

    fn remove_working_on_once(mut commands: Commands, mut worker: ResMut<WorkerToRelease>) {
        let Some(worker_entity) = worker.0.take() else {
            return;
        };
        commands.entity(worker_entity).remove::<WorkingOn>();
    }

    fn test_transport_request(anchor: Entity) -> TransportRequest {
        TransportRequest {
            kind: TransportRequestKind::DepositToStockpile,
            anchor,
            resource_type: ResourceType::Wood,
            issued_by: Entity::PLACEHOLDER,
            priority: TransportPriority::Normal,
            stockpile_group: Vec::new(),
        }
    }

    #[test]
    fn actor_worker_removal_reopens_request_before_next_logic_arbitration() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.init_resource::<WorkerToRelease>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<TransportRequestMetrics>()
            .init_resource::<WheelbarrowArbitrationRuntime>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>();
        app.configure_sets(Update, (GameSystemSet::Logic, GameSystemSet::Actor).chain());
        app.configure_sets(Update, SoulAiSystemSet::Actor.in_set(GameSystemSet::Actor));
        app.configure_sets(Update, TestSet::Arbitrate.in_set(GameSystemSet::Logic));
        app.configure_sets(
            Update,
            TestSet::Reconcile
                .after(SoulAiSystemSet::Actor)
                .in_set(GameSystemSet::Actor),
        );
        app.add_systems(
            Update,
            (
                wheelbarrow_arbitration_system.in_set(TestSet::Arbitrate),
                ApplyDeferred
                    .after(TestSet::Arbitrate)
                    .before(GameSystemSet::Actor)
                    .in_set(GameSystemSet::Logic),
                remove_working_on_once.in_set(SoulAiSystemSet::Actor),
                (ApplyDeferred, ApplyDeferred)
                    .chain()
                    .after(SoulAiSystemSet::Actor)
                    .before(TestSet::Reconcile)
                    .in_set(GameSystemSet::Actor),
                transport_request_task_workers_reconcile_system.in_set(TestSet::Reconcile),
            ),
        );

        // Start lifecycle readers before the fixture creates its relationship target.
        app.update();

        let anchor = app
            .world_mut()
            .spawn((
                Stockpile {
                    capacity: 3,
                    resource_type: Some(ResourceType::Wood),
                },
                Transform::default(),
            ))
            .id();
        let request_entity = app
            .world_mut()
            .spawn((
                test_transport_request(anchor),
                TransportRequestState::Claimed,
                TransportDemand {
                    desired_slots: 1,
                    inflight: 0,
                },
                Designation {
                    work_type: WorkType::Haul,
                },
                TaskSlots::new(1),
                Transform::default(),
            ))
            .id();
        let worker_entity = app.world_mut().spawn(WorkingOn(request_entity)).id();
        let parking_entity = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((
            Wheelbarrow { capacity: 10 },
            ParkedAt(parking_entity),
            Transform::from_xyz(256.0, 0.0, 0.0),
        ));
        for offset in [0.0, 8.0, 16.0] {
            app.world_mut().spawn((
                ResourceItem(ResourceType::Wood),
                Transform::from_xyz(256.0 + offset, 0.0, 0.0),
                Visibility::Visible,
            ));
        }
        app.world_mut().flush();
        assert!(app.world().entity(request_entity).contains::<TaskWorkers>());

        app.world_mut().resource_mut::<WorkerToRelease>().0 = Some(worker_entity);
        app.update();

        assert!(!app.world().entity(request_entity).contains::<TaskWorkers>());
        assert_eq!(
            app.world().get::<TransportRequestState>(request_entity),
            Some(&TransportRequestState::Pending)
        );
        assert!(
            !app.world()
                .entity(request_entity)
                .contains::<WheelbarrowLease>()
        );

        // Arbitration runs in Logic before the actor-tail reconcile. The following update must
        // evaluate the real request and grant it a lease rather than merely matching a proxy
        // query for Pending requests.
        app.update();
        let lease = app
            .world()
            .get::<WheelbarrowLease>(request_entity)
            .expect("the reopened request must reach real arbitration on the next Logic frame");
        assert_eq!(lease.items.len(), 3);
    }
}
