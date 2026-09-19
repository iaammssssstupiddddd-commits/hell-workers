use super::*;
use hw_core::relationships::DeliveringTo;
use hw_jobs::{GatherData, GatherPhase};
use hw_logistics::transport_request::{
    ManualHaulPinnedSource, TransportDemand, TransportPriority, TransportRequest,
    TransportRequestKind, TransportRequestState,
};
use hw_logistics::{BelongsTo, StockpileAcceptance, StockpilePolicy};

fn gatherer(world: &mut World, resource: ResourceType) -> (Entity, Entity) {
    let assignment = world.spawn_empty().id();
    let work = if resource == ResourceType::Wood {
        WorkType::Chop
    } else {
        WorkType::Mine
    };
    let soul = spawn_task_execution_soul(
        world,
        AssignedTask::Gather(GatherData {
            target: assignment,
            work_type: work,
            phase: GatherPhase::Done,
        }),
    );
    let mut identity = ActiveTaskIdentity::new(assignment, assignment, work);
    identity.detach_from_working_on();
    world.entity_mut(soul).insert(identity);
    (soul, assignment)
}

fn item(world: &mut World, resource: ResourceType) -> Entity {
    world
        .spawn((
            Transform::default(),
            Visibility::Visible,
            ResourceItem(resource),
        ))
        .id()
}

fn stockpile(world: &mut World, capacity: usize) -> Entity {
    world
        .spawn((
            Transform::default(),
            Stockpile {
                capacity,
                resource_type: None,
            },
            StockpilePolicy::for_capacity(capacity),
        ))
        .id()
}

fn active_haulers(world: &mut World) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &AssignedTask)>();
    q.iter(world)
        .filter_map(|(entity, task)| {
            matches!(
                task,
                AssignedTask::Haul(_)
                    | AssignedTask::HaulToBlueprint(_)
                    | AssignedTask::HaulToMixer(_)
            )
            .then_some(entity)
        })
        .collect()
}

#[test]
fn gather_chain_claims_capacity_once_test() {
    // Distinct sources compete for capacity; a shared source competes for ownership.
    for (items, capacity) in [(2, 1), (1, 2)] {
        let mut app = task_execution_test_app();
        stockpile(app.world_mut(), capacity);
        for _ in 0..items {
            item(app.world_mut(), ResourceType::Wood);
        }
        let workers = [
            gatherer(app.world_mut(), ResourceType::Wood),
            gatherer(app.world_mut(), ResourceType::Wood),
        ];
        app.update();
        let active = active_haulers(app.world_mut());
        assert_eq!(active.len(), 1);
        let winner = active[0];
        let assignment = workers.iter().find(|(soul, _)| *soul == winner).unwrap().1;
        let identity = app.world().get::<ActiveTaskIdentity>(winner).unwrap();
        assert_eq!(identity.assignment_entity, assignment);
        assert_eq!(identity.current_work_type, WorkType::Haul);
        let source = app.world().get::<WorkingOn>(winner).unwrap().0;
        assert_eq!(identity.current_target_entity, source);
        assert!(app.world().get::<DeliveringTo>(source).is_some());
        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert!(matches!(
            receipts.reservation_ops.as_slice(),
            [ResourceReservationOp::ReserveSource { amount: 1, .. }]
        ));
        assert_eq!(receipts.completed_domain.len(), 1);
        assert_ne!(receipts.completed_domain[0].entity, winner);
        assert!(receipts.abandoned.is_empty());
    }
}

#[test]
fn gather_chain_counts_live_reservations_once_test() {
    for capacity in [1, 2] {
        let mut app = task_execution_test_app();
        app.add_systems(
            Update,
            hw_logistics::apply_reservation_requests_system.after(task_execution_system),
        );
        let destination = stockpile(app.world_mut(), capacity);
        let reserved = item(app.world_mut(), ResourceType::Wood);
        app.world_mut()
            .entity_mut(reserved)
            .insert(DeliveringTo(destination));
        let source = item(app.world_mut(), ResourceType::Wood);
        gatherer(app.world_mut(), ResourceType::Wood);
        app.update();
        assert_eq!(active_haulers(app.world_mut()).len(), capacity - 1);
        assert_eq!(
            app.world()
                .resource::<SharedResourceCache>()
                .get_source_reservation(source),
            capacity - 1
        );
    }
}

#[test]
fn gather_chain_rejection_does_not_retain_cycle_claim_test() {
    for rejection in [
        "policy",
        "target",
        "owner",
        "source-owner-pending",
        "receiver-owner-pending",
        "pin",
        "source-reserved",
        "working",
        "delivery",
    ] {
        let mut app = task_execution_test_app();
        let destination = stockpile(app.world_mut(), 2);
        let source = item(app.world_mut(), ResourceType::Wood);
        let owner = app.world_mut().spawn_empty().id();
        let order = app.world_mut().spawn_empty().id();
        match rejection {
            "policy" => {
                app.world_mut()
                    .get_mut::<StockpilePolicy>(destination)
                    .unwrap()
                    .acceptance = StockpileAcceptance::Only(ResourceType::Rock)
            }
            "target" => {
                app.world_mut()
                    .get_mut::<StockpilePolicy>(destination)
                    .unwrap()
                    .target_amount = 0
            }
            "owner" => {
                app.world_mut().entity_mut(source).insert(BelongsTo(owner));
            }
            "source-owner-pending" | "receiver-owner-pending" => {
                app.world_mut()
                    .entity_mut(owner)
                    .insert(hw_jobs::DeconstructionPending { order });
                let target = if rejection == "source-owner-pending" {
                    source
                } else {
                    destination
                };
                app.world_mut().entity_mut(target).insert(BelongsTo(owner));
            }
            "pin" => {
                app.world_mut()
                    .entity_mut(source)
                    .insert(ManualHaulPinnedSource);
            }
            "source-reserved" => app
                .world_mut()
                .resource_mut::<SharedResourceCache>()
                .reserve_source(source, 1),
            "working" => {
                app.world_mut().spawn(WorkingOn(source));
            }
            "delivery" => {
                app.world_mut()
                    .entity_mut(source)
                    .insert(DeliveringTo(destination));
            }
            _ => unreachable!(),
        }
        gatherer(app.world_mut(), ResourceType::Wood);
        app.update();
        assert!(active_haulers(app.world_mut()).is_empty(), "{rejection}");
        assert!(
            app.world()
                .resource::<TaskNotificationReceipts>()
                .reservation_ops
                .is_empty(),
            "{rejection}"
        );
        // Restore eligibility and start another execution cycle with the same source.
        *app.world_mut()
            .get_mut::<StockpilePolicy>(destination)
            .unwrap() = StockpilePolicy::for_capacity(2);
        app.world_mut()
            .entity_mut(owner)
            .remove::<hw_jobs::DeconstructionPending>();
        app.world_mut()
            .entity_mut(source)
            .remove::<(BelongsTo, ManualHaulPinnedSource, DeliveringTo)>();
        app.world_mut()
            .resource_mut::<SharedResourceCache>()
            .release_source(source, 1);
        let workers: Vec<_> = app
            .world_mut()
            .query::<(Entity, &WorkingOn)>()
            .iter(app.world())
            .filter_map(|(entity, working)| (working.0 == source).then_some(entity))
            .collect();
        for worker in workers {
            app.world_mut().entity_mut(worker).remove::<WorkingOn>();
        }
        gatherer(app.world_mut(), ResourceType::Wood);
        app.update();
        assert_eq!(active_haulers(app.world_mut()).len(), 1, "{rejection}");
    }
}

#[test]
fn gather_chain_mixer_capacity_is_shared_test() {
    for reserved in [0, 1] {
        let mut app = task_execution_test_app();
        let mixer = app
            .world_mut()
            .spawn((
                Transform::default(),
                hw_jobs::mud_mixer::MudMixerStorage {
                    rock: hw_core::constants::MUD_MIXER_CAPACITY - 1,
                    ..default()
                },
            ))
            .id();
        app.world_mut().spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToMixerSolid,
                anchor: mixer,
                resource_type: ResourceType::Rock,
                issued_by: mixer,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TransportDemand {
                desired_slots: 2,
                inflight: 0,
            },
            TransportRequestState::Pending,
        ));
        if reserved == 1 {
            app.world_mut()
                .resource_mut::<SharedResourceCache>()
                .reserve_mixer_destination(mixer, ResourceType::Rock);
        }
        for _ in 0..2 {
            item(app.world_mut(), ResourceType::Rock);
            gatherer(app.world_mut(), ResourceType::Rock);
        }
        app.update();
        assert_eq!(active_haulers(app.world_mut()).len(), 1 - reserved);
        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert_eq!(
            receipts
                .reservation_ops
                .iter()
                .filter(|op| matches!(op, ResourceReservationOp::ReserveSource { .. }))
                .count(),
            1 - reserved
        );
        assert_eq!(
            receipts
                .reservation_ops
                .iter()
                .filter(|op| matches!(op, ResourceReservationOp::ReserveMixerDestination { .. }))
                .count(),
            1 - reserved
        );
    }
}

#[test]
fn gather_chain_resource_and_terminal_contract_test() {
    let mut app = task_execution_test_app();
    app.add_systems(
        Update,
        hw_logistics::apply_reservation_requests_system.after(task_execution_system),
    );
    let destination = stockpile(app.world_mut(), 2);
    for resource in [ResourceType::Wood, ResourceType::Rock] {
        item(app.world_mut(), resource);
        gatherer(app.world_mut(), resource);
    }
    app.update();
    let active = active_haulers(app.world_mut());
    assert_eq!(active.len(), 1);
    let winner = active[0];
    let source = app.world().get::<WorkingOn>(winner).unwrap().0;
    app.world_mut().entity_mut(destination).despawn();
    // Source disappearance exercises the normal pre-pick abort without a forged phase.
    app.world_mut().entity_mut(source).despawn();
    app.update();
    assert!(matches!(
        app.world().get::<AssignedTask>(winner),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(winner).is_none());
    assert!(app.world().get::<WorkingOn>(winner).is_none());
    assert_eq!(
        app.world()
            .resource::<SharedResourceCache>()
            .get_source_reservation(source),
        0
    );
    app.update();
    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert_eq!(receipts.completed_domain.len(), 1);
    assert_eq!(
        receipts
            .reservation_ops
            .iter()
            .filter(|op| matches!(op, ResourceReservationOp::ReleaseSource { .. }))
            .count(),
        1
    );
}

#[test]
fn gather_chain_unassign_releases_source_for_the_next_cycle_test() {
    use crate::soul_ai::execute::task_unassign_apply::handle_soul_task_unassign_system;
    use hw_core::events::SoulTaskUnassignRequest;
    let mut app = task_execution_test_app();
    app.add_message::<SoulTaskUnassignRequest>()
        .add_systems(
            Update,
            handle_soul_task_unassign_system.before(task_execution_system),
        )
        .add_systems(
            Update,
            hw_logistics::apply_reservation_requests_system.after(task_execution_system),
        );
    stockpile(app.world_mut(), 1);
    let source = item(app.world_mut(), ResourceType::Wood);
    let (first, _) = gatherer(app.world_mut(), ResourceType::Wood);
    app.update();
    assert_eq!(
        app.world()
            .resource::<SharedResourceCache>()
            .get_source_reservation(source),
        1
    );
    app.world_mut().write_message(SoulTaskUnassignRequest {
        soul_entity: first,
        emit_abandoned: true,
    });
    app.update();
    assert!(app.world().get::<WorkingOn>(first).is_none());
    assert!(app.world().get::<ActiveTaskIdentity>(first).is_none());
    assert!(app.world().get::<DeliveringTo>(source).is_none());
    assert_eq!(
        app.world()
            .resource::<SharedResourceCache>()
            .get_source_reservation(source),
        0
    );
    let (second, _) = gatherer(app.world_mut(), ResourceType::Wood);
    app.update();
    assert_eq!(active_haulers(app.world_mut()), vec![second]);
    assert_eq!(app.world().get::<WorkingOn>(second).unwrap().0, source);
    assert_eq!(
        app.world()
            .resource::<SharedResourceCache>()
            .get_source_reservation(source),
        1
    );
}

#[test]
fn gather_chain_committed_delivery_survives_policy_change_test() {
    use hw_core::relationships::StoredIn;
    let mut app = task_execution_test_app();
    app.add_systems(First, |mut cache: ResMut<SharedResourceCache>| {
        cache.begin_frame()
    });
    app.add_systems(
        Update,
        hw_logistics::apply_reservation_requests_system.after(task_execution_system),
    );
    let destination = stockpile(app.world_mut(), 2);
    let source = item(app.world_mut(), ResourceType::Wood);
    let (soul, _) = gatherer(app.world_mut(), ResourceType::Wood);
    app.update();
    assert_eq!(
        app.world().get::<DeliveringTo>(source).unwrap().0,
        destination
    );
    let mut policy = app
        .world_mut()
        .get_mut::<StockpilePolicy>(destination)
        .unwrap();
    policy.acceptance = StockpileAcceptance::Only(ResourceType::Rock);
    policy.target_amount = 0;
    // Run the real pickup, navigation and drop phases without rewriting AssignedTask.
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().get::<StoredIn>(source).map(|stored| stored.0),
        Some(destination)
    );
    assert!(app.world().get::<DeliveringTo>(source).is_none());
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world()
            .resource::<SharedResourceCache>()
            .get_source_reservation(source),
        0
    );
    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert_eq!(receipts.completed_domain.len(), 1); // one terminal for the original assignment
    assert!(receipts.abandoned.is_empty());
}
