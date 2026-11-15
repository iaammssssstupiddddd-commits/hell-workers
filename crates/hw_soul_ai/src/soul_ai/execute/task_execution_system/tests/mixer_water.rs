//! Actual product handlers, never ordinary-world/native acceptance evidence.
use super::*;
use crate::water_observations::TankPourReceipts;
use hw_core::{
    constants::MUD_MIXER_CAPACITY,
    relationships::{DeliveringTo, StoredIn, StoredItems},
};
use hw_jobs::{Building, mud_mixer::MudMixerStorage};
use hw_logistics::{
    BelongsTo,
    transport_request::{TransportPriority, TransportRequest, TransportRequestKind},
};

struct Fixture {
    app: App,
    tank: Entity,
    mixer: Entity,
    bucket: Entity,
    worker: Entity,
    request: Entity,
}

fn fixture(observe: bool, pouring: bool, return_stock: bool) -> Fixture {
    let mut app = task_execution_test_app();
    app.init_resource::<WorldEpoch>();
    if observe {
        app.init_resource::<TankPourReceipts>();
    }
    let tank_pos = WorldMap::grid_to_world(25, 50);
    let mixer_pos = WorldMap::grid_to_world(30, 50);
    let tank = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::Tank,
                is_provisional: false,
            },
            Transform::from_translation(tank_pos.extend(0.)),
            Stockpile {
                capacity: 50,
                resource_type: Some(ResourceType::Water),
            },
        ))
        .id();
    let mixer = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::MudMixer,
                is_provisional: false,
            },
            Transform::from_translation(mixer_pos.extend(0.)),
            MudMixerStorage::default(),
            Stockpile {
                capacity: MUD_MIXER_CAPACITY as usize,
                resource_type: Some(ResourceType::Water),
            },
        ))
        .id();
    if return_stock {
        app.world_mut().spawn((
            Stockpile {
                capacity: 10,
                resource_type: None,
            },
            BelongsTo(tank),
            Transform::from_translation(tank_pos.extend(0.)),
        ));
    }
    let bucket = app
        .world_mut()
        .spawn((
            ResourceItem(if pouring {
                ResourceType::BucketWater
            } else {
                ResourceType::BucketEmpty
            }),
            BelongsTo(tank),
            DeliveringTo(mixer),
            Transform::default(),
        ))
        .id();
    let worker = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::BucketTransport(BucketTransportData {
            bucket,
            source: BucketTransportSource::Tank {
                tank,
                needs_fill: true,
            },
            destination: BucketTransportDestination::Mixer(mixer),
            amount: if pouring { 5 } else { 0 },
            phase: if pouring {
                BucketTransportPhase::Pouring { progress: 0. }
            } else {
                BucketTransportPhase::Filling { progress: 0. }
            },
        }),
    );
    let request = app
        .world_mut()
        .spawn(TransportRequest {
            kind: TransportRequestKind::DeliverWaterToMixer,
            anchor: mixer,
            resource_type: ResourceType::Water,
            issued_by: worker,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        })
        .id();
    app.world_mut().entity_mut(worker).insert((
        Transform::from_translation(tank_pos.extend(0.)),
        Destination(tank_pos),
        Inventory(Some(bucket)),
        WorkingOn(request),
        ActiveTaskIdentity::new(request, request, WorkType::HaulWaterToMixer),
    ));
    Fixture {
        app,
        tank,
        mixer,
        bucket,
        worker,
        request,
    }
}

#[test]
fn actual_tank_take_requires_full_bucket_and_found_route_without_partial_consumption() {
    for observe in [false, true] {
        for count in [0, 1, 4, 5] {
            for route in ["found", "unreachable", "deferred"] {
                let Fixture {
                    mut app,
                    tank,
                    mixer,
                    bucket,
                    worker,
                    request,
                } = fixture(observe, false, true);
                let inputs: Vec<_> = (0..count)
                    .map(|_| {
                        app.world_mut()
                            .spawn((
                                ResourceItem(ResourceType::Water),
                                StoredIn(tank),
                                Transform::default(),
                                Visibility::Hidden,
                            ))
                            .id()
                    })
                    .collect();
                if route == "unreachable" {
                    for y in 0..hw_core::constants::MAP_HEIGHT {
                        app.world_mut()
                            .resource_mut::<WorldMap>()
                            .add_grid_obstacle((27, y));
                    }
                } else if route == "deferred" {
                    app.insert_resource(RuntimePathSearchBudget::new(0));
                }
                app.update();
                let taken = count == 5 && route == "found";
                assert_eq!(
                    app.world().get::<ResourceItem>(bucket).unwrap().0,
                    if taken {
                        ResourceType::BucketWater
                    } else {
                        ResourceType::BucketEmpty
                    }
                );
                assert!(
                    inputs
                        .iter()
                        .all(|&e| app.world().get_entity(e).is_err() == taken)
                );
                if taken {
                    assert_eq!(app.world().get::<DeliveringTo>(bucket).unwrap().0, mixer);
                    assert!(
                        matches!(app.world().get::<AssignedTask>(worker),Some(AssignedTask::BucketTransport(d))
                if d.amount==count && matches!(d.phase,BucketTransportPhase::GoingToDestination))
                    );
                } else if count == 5 && route == "deferred" {
                    assert!(matches!(app.world().get::<AssignedTask>(worker),
                        Some(AssignedTask::BucketTransport(d))
                        if d.amount == 0 && matches!(d.phase, BucketTransportPhase::Filling { progress: 0. })));
                    assert_eq!(
                        app.world().get::<Inventory>(worker).unwrap().0,
                        Some(bucket)
                    );
                    assert_eq!(app.world().get::<WorkingOn>(worker).unwrap().0, request);
                    assert_eq!(app.world().get::<DeliveringTo>(bucket).unwrap().0, mixer);
                    assert!(
                        app.world()
                            .resource::<TaskNotificationReceipts>()
                            .reservation_ops
                            .is_empty()
                    );
                }
                if observe {
                    let rows = app.world().resource::<TankPourReceipts>();
                    assert!(rows.failure.is_none(), "{:?}", rows.failure);
                    assert_eq!(rows.tank_takes.len(), usize::from(taken));
                    if taken {
                        assert_eq!(rows.tank_takes[0].context.assignment, request);
                        assert_eq!(rows.tank_takes[0].inputs.len(), count as usize);
                        assert_eq!(rows.tank_takes[0].route_found, route == "found");
                    }
                }
            }
        }
    }
}

#[test]
fn actual_mixer_pour_requires_full_capacity_and_preserves_water_on_retry() {
    for observe in [false, true] {
        for available in [0, 2, 5] {
            for returning in [false, true] {
                let Fixture {
                    mut app,
                    tank,
                    mixer,
                    bucket,
                    worker,
                    ..
                } = fixture(observe, true, returning);
                let before = MUD_MIXER_CAPACITY as usize - available;
                for _ in 0..before {
                    app.world_mut().spawn((
                        ResourceItem(ResourceType::Water),
                        StoredIn(mixer),
                        Transform::default(),
                    ));
                }
                app.update();
                assert_eq!(
                    app.world().get::<StoredItems>(mixer).unwrap().len(),
                    before + if available == 5 { 5 } else { 0 }
                );
                assert_eq!(
                    app.world().get::<ResourceItem>(bucket).unwrap().0,
                    if available < 5 {
                        ResourceType::BucketWater
                    } else {
                        ResourceType::BucketEmpty
                    }
                );
                assert!(
                    app.world()
                        .resource::<TaskNotificationReceipts>()
                        .completed_domain
                        .is_empty()
                );
                if available == 5 && returning {
                    assert!(
                        matches!(app.world().get::<AssignedTask>(worker),Some(AssignedTask::BucketTransport(d))
            if matches!(d.phase,BucketTransportPhase::ReturningBucket))
                    );
                }
                if available < 5 {
                    assert!(matches!(
                        app.world().get::<AssignedTask>(worker),
                        Some(AssignedTask::None)
                    ));
                    assert_eq!(app.world().get::<Inventory>(worker).unwrap().0, None);
                    assert!(app.world().get::<WorkingOn>(worker).is_none());
                    assert!(app.world().get::<ActiveTaskIdentity>(worker).is_none());
                    assert!(app.world().get::<DeliveringTo>(bucket).is_none());
                    assert_eq!(app.world().get::<BelongsTo>(bucket).unwrap().0, tank);
                    assert_eq!(
                        app.world()
                            .resource::<TaskNotificationReceipts>()
                            .reservation_ops,
                        vec![ResourceReservationOp::ReleaseMixerDestination {
                            target: mixer,
                            resource_type: ResourceType::Water
                        }]
                    );
                }
                if observe {
                    let rows = app.world().resource::<TankPourReceipts>();
                    assert!(rows.failure.is_none(), "{:?}", rows.failure);
                    assert_eq!(rows.mixer_pours.len(), usize::from(available == 5));
                    if available == 5 {
                        assert_eq!(rows.mixer_pours[0].outputs.len(), 5);
                        assert_eq!(rows.mixer_pours[0].returning, returning);
                    }
                }
            }
        }
    }
}

#[test]
fn mixer_pour_rejects_missing_wrong_target_and_noncanonical_amount_without_consuming() {
    for case in [
        "missing",
        "wrong-resource",
        "not-mixer",
        "partial-one",
        "partial-four",
    ] {
        let mut fixture = fixture(true, true, true);
        match case {
            "missing" => {
                fixture.app.world_mut().despawn(fixture.mixer);
            }
            "wrong-resource" => {
                fixture
                    .app
                    .world_mut()
                    .get_mut::<Stockpile>(fixture.mixer)
                    .unwrap()
                    .resource_type = Some(ResourceType::Sand);
            }
            "not-mixer" => {
                fixture
                    .app
                    .world_mut()
                    .entity_mut(fixture.mixer)
                    .remove::<MudMixerStorage>();
            }
            _ => {
                let mut task = fixture
                    .app
                    .world_mut()
                    .get_mut::<AssignedTask>(fixture.worker)
                    .unwrap();
                let AssignedTask::BucketTransport(data) = &mut *task else {
                    panic!("expected bucket transport");
                };
                data.amount = if case == "partial-one" { 1 } else { 4 };
            }
        }
        fixture.app.update();
        assert_eq!(
            fixture
                .app
                .world()
                .get::<ResourceItem>(fixture.bucket)
                .unwrap()
                .0,
            ResourceType::BucketWater,
            "{case}"
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<Inventory>(fixture.worker)
                .unwrap()
                .0,
            None,
            "{case}"
        );
        assert!(
            matches!(
                fixture.app.world().get::<AssignedTask>(fixture.worker),
                Some(AssignedTask::None)
            ),
            "{case}"
        );
        assert!(
            fixture
                .app
                .world()
                .get::<DeliveringTo>(fixture.bucket)
                .is_none(),
            "{case}"
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<BelongsTo>(fixture.bucket)
                .unwrap()
                .0,
            fixture.tank
        );
        let rows = fixture.app.world().resource::<TankPourReceipts>();
        assert!(rows.failure.is_none());
        assert!(rows.mixer_pours.is_empty());
        assert!(
            fixture
                .app
                .world()
                .resource::<TaskNotificationReceipts>()
                .completed_domain
                .is_empty()
        );
    }
}

fn add_carrier(fixture: &mut Fixture, bucket: Entity, mixer: Entity, filling: bool) -> Entity {
    let task = AssignedTask::BucketTransport(BucketTransportData {
        bucket,
        source: BucketTransportSource::Tank {
            tank: fixture.tank,
            needs_fill: true,
        },
        destination: BucketTransportDestination::Mixer(mixer),
        amount: if filling { 0 } else { 5 },
        phase: if filling {
            BucketTransportPhase::Filling { progress: 0. }
        } else {
            BucketTransportPhase::Pouring { progress: 0. }
        },
    });
    let pos = fixture
        .app
        .world()
        .get::<Transform>(fixture.worker)
        .unwrap()
        .translation;
    let worker = spawn_task_execution_soul(fixture.app.world_mut(), task);
    let request = fixture
        .app
        .world_mut()
        .spawn(TransportRequest {
            kind: TransportRequestKind::DeliverWaterToMixer,
            anchor: mixer,
            resource_type: ResourceType::Water,
            issued_by: worker,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        })
        .id();
    fixture.app.world_mut().entity_mut(worker).insert((
        Transform::from_translation(pos),
        Inventory(Some(bucket)),
        WorkingOn(request),
        ActiveTaskIdentity::new(request, request, WorkType::HaulWaterToMixer),
    ));
    worker
}

#[test]
fn same_pass_water_claims_prevent_double_take_pour_and_duplicate_bucket_conversion() {
    for case in ["same-tank", "same-mixer", "same-bucket-other-mixer"] {
        let mut fixture = fixture(true, case != "same-tank", true);
        let bucket = if case == "same-bucket-other-mixer" {
            fixture.bucket
        } else {
            fixture
                .app
                .world_mut()
                .spawn((
                    ResourceItem(if case == "same-tank" {
                        ResourceType::BucketEmpty
                    } else {
                        ResourceType::BucketWater
                    }),
                    BelongsTo(fixture.tank),
                    Transform::default(),
                ))
                .id()
        };
        let other_mixer = if case == "same-bucket-other-mixer" {
            fixture
                .app
                .world_mut()
                .spawn((
                    Transform::default(),
                    MudMixerStorage::default(),
                    Stockpile {
                        capacity: 5,
                        resource_type: Some(ResourceType::Water),
                    },
                ))
                .id()
        } else {
            fixture.mixer
        };
        let second = add_carrier(&mut fixture, bucket, other_mixer, case == "same-tank");
        if case == "same-tank" {
            for _ in 0..5 {
                fixture.app.world_mut().spawn((
                    ResourceItem(ResourceType::Water),
                    StoredIn(fixture.tank),
                    Transform::default(),
                    Visibility::Hidden,
                ));
            }
        }
        fixture.app.update();
        let rows = fixture.app.world().resource::<TankPourReceipts>();
        assert!(rows.failure.is_none(), "{:?}", rows.failure);
        if case == "same-tank" {
            assert_eq!(rows.tank_takes.len(), 1);
            assert_eq!(rows.tank_takes[0].inputs.len(), 5);
            assert_eq!(
                [fixture.worker, second]
                    .iter()
                    .filter(|&&w| fixture.app.world().get::<Inventory>(w).unwrap().0.is_some())
                    .count(),
                1
            );
        } else {
            assert_eq!(rows.mixer_pours.len(), 1);
            assert_eq!(rows.mixer_pours[0].outputs.len(), 5);
            let mut query = fixture
                .app
                .world_mut()
                .query::<(&ResourceItem, &StoredIn)>();
            assert_eq!(
                query
                    .iter(fixture.app.world())
                    .filter(|(i, s)| i.0 == ResourceType::Water
                        && [fixture.mixer, other_mixer].contains(&s.0))
                    .count(),
                5
            );
        }
        assert!(
            fixture
                .app
                .world()
                .resource::<TaskNotificationReceipts>()
                .completed_domain
                .is_empty()
        );
    }
}

#[test]
fn rejected_full_bucket_can_be_picked_up_again_and_poured_on_the_next_pass() {
    let mut fixture = fixture(true, true, true);
    let blockers: Vec<_> = (0..3)
        .map(|_| {
            fixture
                .app
                .world_mut()
                .spawn((
                    ResourceItem(ResourceType::Water),
                    StoredIn(fixture.mixer),
                    Transform::default(),
                ))
                .id()
        })
        .collect();
    fixture.app.update();
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ResourceItem>(fixture.bucket)
            .unwrap()
            .0,
        ResourceType::BucketWater
    );
    assert!(matches!(
        fixture.app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    for item in blockers {
        fixture.app.world_mut().despawn(item);
    }
    fixture.app.world_mut().entity_mut(fixture.worker).insert((
        AssignedTask::BucketTransport(BucketTransportData {
            bucket: fixture.bucket,
            source: BucketTransportSource::Tank {
                tank: fixture.tank,
                needs_fill: false,
            },
            destination: BucketTransportDestination::Mixer(fixture.mixer),
            amount: 0,
            phase: BucketTransportPhase::GoingToBucket,
        }),
        WorkingOn(fixture.request),
        ActiveTaskIdentity::new(fixture.request, fixture.request, WorkType::HaulWaterToMixer),
    ));
    fixture.app.update();
    assert!(
        matches!(fixture.app.world().get::<AssignedTask>(fixture.worker),Some(AssignedTask::BucketTransport(d))
        if d.amount==5 && matches!(d.phase,BucketTransportPhase::GoingToDestination))
    );
    let pos = fixture
        .app
        .world()
        .get::<Transform>(fixture.mixer)
        .unwrap()
        .translation;
    fixture
        .app
        .world_mut()
        .entity_mut(fixture.worker)
        .insert(Transform::from_translation(pos));
    fixture.app.update();
    fixture.app.update();
    assert_eq!(
        fixture
            .app
            .world()
            .resource::<TankPourReceipts>()
            .mixer_pours
            .len(),
        1
    );
    assert_eq!(
        fixture
            .app
            .world()
            .resource::<TankPourReceipts>()
            .mixer_pours[0]
            .outputs
            .len(),
        5
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ResourceItem>(fixture.bucket)
            .unwrap()
            .0,
        ResourceType::BucketEmpty
    );
}

#[test]
fn transfer_receipts_reject_foreign_missing_and_unapplied_context_without_repair() {
    use bevy::ecs::world::CommandQueue;
    for take in [false, true] {
        for case in 0..12 {
            let Fixture {
                mut app,
                tank,
                bucket,
                worker,
                request,
                ..
            } = fixture(true, !take, true);
            if take {
                for _ in 0..5 {
                    app.world_mut().spawn((
                        ResourceItem(ResourceType::Water),
                        StoredIn(tank),
                        Transform::default(),
                        Visibility::Hidden,
                    ));
                }
            }
            app.update();
            let before = app.world().get::<ResourceItem>(bucket).unwrap().0;
            let mut queue = CommandQueue::default();
            if take {
                let mut row = app.world().resource::<TankPourReceipts>().tank_takes[0].clone();
                match case {
                    0 => row.context.world_epoch += 1,
                    1 => row.context.assignment = bucket,
                    2 => row.context.work_type = WorkType::Refine,
                    3 => row.context.mixer = tank,
                    4 => row.bucket_before = Some(ResourceType::BucketWater),
                    5 => row.inputs[1] = row.inputs[0],
                    6 => row.inputs[0] = app.world_mut().spawn_empty().id(),
                    7 => {
                        app.world_mut().entity_mut(worker).insert(Inventory(None));
                    }
                    8 => {
                        app.world_mut().entity_mut(worker).remove::<WorkingOn>();
                    }
                    9 => {
                        app.world_mut()
                            .entity_mut(request)
                            .remove::<TransportRequest>();
                    }
                    10 => row.inputs_before[0].stored_in = Some(bucket),
                    _ => row.context.carried_before = None,
                }
                row.queue(&mut Commands::new(&mut queue, app.world()));
            } else {
                let mut row = app.world().resource::<TankPourReceipts>().mixer_pours[0].clone();
                match case {
                    0 => row.context.world_epoch += 1,
                    1 => row.context.assignment = bucket,
                    2 => row.amount = 0,
                    3 => row.amount = 6,
                    4 => row.before_count = row.capacity,
                    5 => row.outputs[1] = row.outputs[0],
                    6 => {
                        app.world_mut()
                            .entity_mut(row.outputs[0])
                            .insert(StoredIn(tank));
                    }
                    7 => {
                        app.world_mut().entity_mut(worker).insert(Inventory(None));
                    }
                    8 => row.returning = false,
                    9 => {
                        app.world_mut()
                            .entity_mut(request)
                            .remove::<TransportRequest>();
                    }
                    10 => row.bucket_before = Some(ResourceType::BucketEmpty),
                    _ => row.context.carried_before = None,
                }
                row.queue(&mut Commands::new(&mut queue, app.world()));
            }
            queue.apply(app.world_mut());
            assert!(app.world().resource::<TankPourReceipts>().failure.is_some());
            assert_eq!(app.world().get::<ResourceItem>(bucket).unwrap().0, before);
        }
    }
}
