//! Actual pouring handler fixtures, not ordinary-world acceptance.
use super::*;
use crate::water_observations::TankPourReceipts;
use hw_core::relationships::StoredIn;
use hw_jobs::{
    BucketTransportData, BucketTransportDestination, BucketTransportPhase, BucketTransportSource,
    Building,
};

#[test]
fn real_river_fill_changes_only_the_carried_bucket_with_optional_observer() {
    for observe in [false, true] {
        for blocked in [false, true] {
            let mut app = task_execution_test_app();
            app.init_resource::<WorldEpoch>();
            if observe {
                app.init_resource::<TankPourReceipts>();
            }
            let tank_pos = WorldMap::grid_to_world(30, 50);
            let tank = app
                .world_mut()
                .spawn((
                    Transform::from_translation(tank_pos.extend(0.)),
                    Building {
                        kind: BuildingType::Tank,
                        is_provisional: false,
                    },
                    Stockpile {
                        capacity: 50,
                        resource_type: Some(ResourceType::Water),
                    },
                ))
                .id();
            let bucket = app
                .world_mut()
                .spawn((
                    ResourceItem(ResourceType::BucketEmpty),
                    hw_logistics::BelongsTo(tank),
                    Transform::default(),
                ))
                .id();
            let worker = spawn_task_execution_soul(
                app.world_mut(),
                AssignedTask::BucketTransport(BucketTransportData {
                    bucket,
                    source: BucketTransportSource::River,
                    destination: BucketTransportDestination::Tank(tank),
                    amount: 1,
                    phase: BucketTransportPhase::Filling { progress: 1. },
                }),
            );
            let request = app
                .world_mut()
                .spawn(hw_logistics::transport_request::TransportRequest {
                    kind: hw_logistics::transport_request::TransportRequestKind::GatherWaterToTank,
                    anchor: tank,
                    resource_type: ResourceType::Water,
                    issued_by: worker,
                    priority: hw_logistics::transport_request::TransportPriority::Normal,
                    stockpile_group: vec![],
                })
                .id();
            let pos = WorldMap::grid_to_world(25, 50);
            app.world_mut().entity_mut(worker).insert((
                Transform::from_translation(pos.extend(0.)),
                Destination(pos),
                Inventory(Some(bucket)),
                WorkingOn(request),
                ActiveTaskIdentity::new(request, request, WorkType::GatherWater),
            ));
            if blocked {
                for y in 0..hw_core::constants::MAP_HEIGHT {
                    app.world_mut()
                        .resource_mut::<WorldMap>()
                        .add_grid_obstacle((27, y));
                }
            }
            app.update();
            assert_eq!(
                app.world().get::<ResourceItem>(bucket).unwrap().0,
                if blocked {
                    ResourceType::BucketEmpty
                } else {
                    ResourceType::BucketWater
                }
            );
            assert!(
                app.world()
                    .get::<hw_core::relationships::StoredItems>(tank)
                    .is_none()
            );
            if !blocked {
                assert_eq!(
                    app.world().get::<Inventory>(worker).unwrap().0,
                    Some(bucket)
                );
                assert_eq!(
                    app.world()
                        .get::<hw_core::relationships::DeliveringTo>(bucket)
                        .unwrap()
                        .0,
                    tank
                );
                assert!(
                    matches!(app.world().get::<AssignedTask>(worker), Some(AssignedTask::BucketTransport(data))
                    if matches!(data.phase, BucketTransportPhase::GoingToDestination))
                );
            }
            if observe {
                let receipts = app.world().resource::<TankPourReceipts>();
                assert!(receipts.failure.is_none(), "{:?}", receipts.failure);
                assert_eq!(receipts.river_fills.len(), usize::from(!blocked));
                if !blocked {
                    assert_eq!(receipts.river_fills[0].assignment, request);
                    assert_eq!(
                        receipts.river_fills[0].bucket_before,
                        Some(ResourceType::BucketEmpty)
                    );
                }
            }
        }
    }
}

#[test]
fn real_tank_pour_branch_keeps_gameplay_identical_with_and_without_observer() {
    #[derive(Resource, Default)]
    struct Delivered(Vec<(Entity, usize)>);
    for (observe, full) in [(false, true), (true, true), (false, false)] {
        let mut app = task_execution_test_app();
        app.init_resource::<WorldEpoch>()
            .init_resource::<Delivered>()
            .add_observer(
                |delivery: On<hw_jobs::events::TankWaterDelivered>,
                 stored: Query<&hw_core::relationships::StoredItems>,
                 mut delivered: ResMut<Delivered>| {
                    let count = stored.get(delivery.tank).map_or(0, |items| items.len());
                    delivered.0.push((delivery.tank, count));
                },
            );
        if observe {
            app.init_resource::<TankPourReceipts>();
        }
        let tank = app
            .world_mut()
            .spawn((
                Transform::default(),
                Building {
                    kind: BuildingType::Tank,
                    is_provisional: false,
                },
                Stockpile {
                    capacity: 50,
                    resource_type: Some(ResourceType::Water),
                },
            ))
            .id();
        let bucket = app
            .world_mut()
            .spawn((
                ResourceItem(if full {
                    ResourceType::BucketWater
                } else {
                    ResourceType::BucketEmpty
                }),
                Transform::default(),
            ))
            .id();
        let worker = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::BucketTransport(BucketTransportData {
                bucket,
                source: BucketTransportSource::River,
                destination: BucketTransportDestination::Tank(tank),
                amount: 5,
                phase: BucketTransportPhase::Pouring { progress: 0. },
            }),
        );
        let request = app
            .world_mut()
            .spawn(hw_logistics::transport_request::TransportRequest {
                kind: hw_logistics::transport_request::TransportRequestKind::GatherWaterToTank,
                anchor: tank,
                resource_type: ResourceType::Water,
                issued_by: worker,
                priority: hw_logistics::transport_request::TransportPriority::Normal,
                stockpile_group: Vec::new(),
            })
            .id();
        app.world_mut().entity_mut(worker).insert((
            Inventory(Some(bucket)),
            WorkingOn(request),
            ActiveTaskIdentity::new(request, request, WorkType::GatherWater),
        ));
        app.update();
        assert_eq!(
            app.world().get::<ResourceItem>(bucket).unwrap().0,
            ResourceType::BucketEmpty
        );
        assert_eq!(app.world().get::<Inventory>(worker).unwrap().0, None);
        if !full {
            assert!(app.world().resource::<Delivered>().0.is_empty());
            assert!(
                app.world()
                    .get::<hw_core::relationships::StoredItems>(tank)
                    .is_none()
            );
            continue;
        }
        assert_eq!(app.world().resource::<Delivered>().0, vec![(tank, 5)]);
        let items = app
            .world()
            .get::<hw_core::relationships::StoredItems>(tank)
            .unwrap();
        assert_eq!(items.len(), 5);
        for entity in items.iter() {
            assert_eq!(
                app.world().get::<ResourceItem>(entity).unwrap().0,
                ResourceType::Water
            );
            assert_eq!(app.world().get::<StoredIn>(entity).unwrap().0, tank);
        }
        if observe {
            let receipts = app.world().resource::<TankPourReceipts>();
            assert!(receipts.failure.is_none(), "{:?}", receipts.failure);
            assert_eq!(receipts.rows.len(), 1);
            assert_eq!(receipts.rows[0].target, request);
            assert_eq!(receipts.rows[0].assignment, request);
            assert_ne!(receipts.rows[0].target, bucket);
            assert_eq!(
                (
                    receipts.rows[0].worker,
                    receipts.rows[0].tank,
                    receipts.rows[0].bucket
                ),
                (worker, tank, bucket)
            );
        } else {
            assert!(!app.world().contains_resource::<TankPourReceipts>());
        }
    }
}
