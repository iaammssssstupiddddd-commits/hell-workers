//! Actual pouring handler fixtures, not ordinary-world acceptance.
use super::*;
use crate::water_observations::TankPourReceipts;
use hw_core::relationships::StoredIn;
use hw_jobs::{
    BucketTransportData, BucketTransportDestination, BucketTransportPhase, BucketTransportSource,
    Building,
};

#[test]
fn real_tank_pour_branch_keeps_gameplay_identical_with_and_without_observer() {
    for observe in [false, true] {
        let mut app = task_execution_test_app();
        app.init_resource::<WorldEpoch>();
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
                ResourceItem(ResourceType::BucketWater),
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
        app.world_mut().entity_mut(worker).insert((
            Inventory(Some(bucket)),
            WorkingOn(bucket),
            ActiveTaskIdentity::new(tank, bucket, WorkType::GatherWater),
        ));
        app.update();
        assert_eq!(
            app.world().get::<ResourceItem>(bucket).unwrap().0,
            ResourceType::BucketEmpty
        );
        assert_eq!(app.world().get::<Inventory>(worker).unwrap().0, None);
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
