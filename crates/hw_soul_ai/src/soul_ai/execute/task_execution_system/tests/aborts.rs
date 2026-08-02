use super::*;
use hw_core::constants::MAP_HEIGHT;
use hw_core::relationships::{DeliveringTo, IncomingDeliveries, TaskWorkers};
use hw_jobs::{HaulToBlueprintData, HaulToBpPhase};

#[test]
fn unreachable_blueprint_haul_source_retryably_releases_assignment_and_delivery() {
    let mut app = task_execution_test_app();
    {
        let mut world_map = app.world_mut().resource_mut::<WorldMap>();
        for y in 0..MAP_HEIGHT {
            world_map.add_grid_obstacle((50, y));
        }
    }

    let soul_pos = WorldMap::grid_to_world(25, 50);
    let item_pos = WorldMap::grid_to_world(75, 50);
    let blueprint = app.world_mut().spawn_empty().id();
    let item = app
        .world_mut()
        .spawn((
            Transform::from_translation(item_pos.extend(0.0)),
            Visibility::Visible,
            ResourceItem(ResourceType::Wood),
            DeliveringTo(blueprint),
        ))
        .id();
    let assignment = app.world_mut().spawn_empty().id();
    let soul = app
        .world_mut()
        .spawn((
            Transform::from_translation(soul_pos.extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                item,
                blueprint,
                phase: HaulToBpPhase::GoingToItem,
            }),
            Destination(item_pos),
            Path::default(),
            Inventory::default(),
            ActiveTaskIdentity::new(assignment, assignment, WorkType::Haul),
            WorkingOn(assignment),
        ))
        .id();
    app.world_mut().flush();

    assert_eq!(
        app.world()
            .get::<IncomingDeliveries>(blueprint)
            .map(IncomingDeliveries::len),
        Some(1)
    );
    assert_eq!(
        app.world()
            .get::<TaskWorkers>(assignment)
            .map(TaskWorkers::len),
        Some(1)
    );

    app.update();

    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert!(receipts.completed_domain.is_empty());
    assert!(receipts.completed_visual.is_empty());
    assert!(receipts.abandoned.is_empty());
    assert_eq!(
        receipts.reservation_ops,
        vec![ResourceReservationOp::ReleaseSource {
            source: item,
            amount: 1,
        }]
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
    assert!(app.world().get::<WorkingOn>(soul).is_none());
    assert!(app.world().get::<DeliveringTo>(item).is_none());
    assert!(
        app.world()
            .get::<IncomingDeliveries>(blueprint)
            .is_none_or(IncomingDeliveries::is_empty)
    );
    assert!(
        app.world()
            .get::<TaskWorkers>(assignment)
            .is_none_or(TaskWorkers::is_empty)
    );
    assert_eq!(app.world().get::<Inventory>(soul).unwrap().0, None);
}

#[test]
fn stockpile_reject_retryably_aborts_without_completion_or_abandonment_notifications() {
    let mut app = task_execution_test_app();
    let item = app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::Visible,
            ResourceItem(ResourceType::Wood),
        ))
        .id();
    let stockpile = app
        .world_mut()
        .spawn((
            Transform::default(),
            Stockpile {
                capacity: 1,
                resource_type: Some(ResourceType::Rock),
            },
        ))
        .id();
    let assignment = app.world_mut().spawn_empty().id();
    let soul = app
        .world_mut()
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            AssignedTask::Haul(HaulData {
                item,
                stockpile,
                phase: HaulPhase::Dropping,
            }),
            Destination(Vec2::ZERO),
            Path::default(),
            Inventory(Some(item)),
            ActiveTaskIdentity::new(assignment, stockpile, WorkType::Haul),
            WorkingOn(stockpile),
        ))
        .id();

    app.update();

    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert!(receipts.completed_domain.is_empty());
    assert!(receipts.completed_visual.is_empty());
    assert!(receipts.abandoned.is_empty());
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
    assert!(app.world().get::<WorkingOn>(soul).is_none());
}

#[test]
fn missing_identity_retryably_unassigns_without_completion_notification() {
    let mut app = task_execution_test_app();
    let target = app.world_mut().spawn_empty().id();
    let soul = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::Build(BuildData {
            blueprint: target,
            phase: BuildPhase::Done,
        }),
    );
    app.world_mut().entity_mut(soul).insert(WorkingOn(target));

    app.update();

    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert!(receipts.completed_domain.is_empty());
    assert!(receipts.completed_visual.is_empty());
    assert!(receipts.abandoned.is_empty());
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
    assert!(app.world().get::<WorkingOn>(soul).is_none());
}

#[test]
fn vanished_blueprint_done_phase_aborts_without_completion_notification() {
    let mut app = task_execution_test_app();
    let target = app.world_mut().spawn_empty().id();
    let assignment = app.world_mut().spawn_empty().id();
    let soul = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::Build(BuildData {
            blueprint: target,
            phase: BuildPhase::Done,
        }),
    );
    app.world_mut().entity_mut(soul).insert((
        ActiveTaskIdentity::new(assignment, target, WorkType::Build),
        WorkingOn(target),
    ));

    app.update();

    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert!(receipts.completed_domain.is_empty());
    assert!(receipts.completed_visual.is_empty());
    assert!(receipts.abandoned.is_empty());
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
    assert!(app.world().get::<WorkingOn>(soul).is_none());
}

#[test]
fn bucket_abort_releases_active_reservations_without_terminal_notifications() {
    let mut app = task_execution_test_app();
    let bucket = app.world_mut().spawn_empty().id();
    let tank = app.world_mut().spawn_empty().id();
    let mixer = app.world_mut().spawn_empty().id();
    let assignment = app.world_mut().spawn_empty().id();
    let soul = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::BucketTransport(BucketTransportData {
            bucket,
            source: BucketTransportSource::Tank {
                tank,
                needs_fill: true,
            },
            destination: BucketTransportDestination::Mixer(mixer),
            amount: 0,
            phase: BucketTransportPhase::GoingToBucket,
        }),
    );
    app.world_mut().entity_mut(soul).insert((
        ActiveTaskIdentity::new(assignment, mixer, WorkType::HaulWaterToMixer),
        WorkingOn(mixer),
    ));

    app.update();

    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert!(receipts.completed_domain.is_empty());
    assert!(receipts.completed_visual.is_empty());
    assert!(receipts.abandoned.is_empty());
    assert_eq!(
        receipts.reservation_ops,
        vec![
            ResourceReservationOp::ReleaseSource {
                source: bucket,
                amount: 1,
            },
            ResourceReservationOp::ReleaseSource {
                source: tank,
                amount: 1,
            },
            ResourceReservationOp::ReleaseMixerDestination {
                target: mixer,
                resource_type: ResourceType::Water,
            },
        ]
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
    assert!(app.world().get::<WorkingOn>(soul).is_none());
}
