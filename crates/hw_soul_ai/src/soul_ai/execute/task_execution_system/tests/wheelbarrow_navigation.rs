use super::*;
use hw_core::{constants::TILE_SIZE, logistics::WheelbarrowDestination, relationships::LoadedIn};
use hw_jobs::{
    Building, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase, mud_mixer::MudMixerStorage,
};
use hw_logistics::Wheelbarrow;

fn mixer_fixture() -> (App, Entity, Entity, Entity, Entity) {
    let mut app = task_execution_test_app();
    let mixer_pos = (WorldMap::grid_to_world(70, 47) + WorldMap::grid_to_world(71, 48)) / 2.0;
    let end = WorldMap::grid_to_world(72, 49);
    assert!(mixer_pos.distance(end) > TILE_SIZE * 1.8);
    let mixer = app
        .world_mut()
        .spawn((
            Transform::from_translation(mixer_pos.extend(0.0)),
            Building {
                kind: BuildingType::MudMixer,
                is_provisional: false,
            },
            MudMixerStorage::default(),
        ))
        .id();
    app.world_mut()
        .resource_mut::<WorldMap>()
        .register_completed_building_footprint(
            BuildingType::MudMixer,
            mixer,
            [(70, 47), (71, 47), (70, 48), (71, 48)],
        );
    let cart = app
        .world_mut()
        .spawn((
            Transform::from_translation(end.extend(0.0)),
            Wheelbarrow { capacity: 5 },
        ))
        .id();
    let item = app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            ResourceItem(ResourceType::Sand),
            LoadedIn(cart),
        ))
        .id();
    let request = app.world_mut().spawn_empty().id();
    let worker = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
            wheelbarrow: cart,
            source_pos: Vec2::ZERO,
            destination: WheelbarrowDestination::Mixer {
                entity: mixer,
                resource_type: ResourceType::Sand,
            },
            collect_source: None,
            collect_amount: 0,
            collect_resource_type: None,
            items: vec![item],
            phase: HaulWithWheelbarrowPhase::GoingToDestination,
        }),
    );
    app.world_mut().entity_mut(worker).insert((
        Transform::from_translation(end.extend(0.0)),
        Destination(end),
        Inventory(Some(cart)),
        WorkingOn(request),
        ActiveTaskIdentity::new(request, request, WorkType::WheelbarrowHaul),
    ));
    app.world_mut().get_mut::<Path>(worker).unwrap().waypoints = vec![end];
    (app, worker, mixer, cart, item)
}

#[test]
fn mixer_wheelbarrow_reaches_its_legal_diagonal_endpoint_then_unloads() {
    let (mut app, worker, mixer, cart, item) = mixer_fixture();
    app.update();
    assert!(matches!(
        app.world().get::<AssignedTask>(worker),
        Some(AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
            phase: HaulWithWheelbarrowPhase::Unloading,
            ..
        }))
    ));
    assert_eq!(app.world().get::<LoadedIn>(item).unwrap().0, cart);
    assert_eq!(app.world().get::<MudMixerStorage>(mixer).unwrap().sand, 0);
    assert!(
        app.world()
            .resource::<TaskNotificationReceipts>()
            .completed_domain
            .is_empty()
    );
    app.update();
    assert_eq!(app.world().get::<MudMixerStorage>(mixer).unwrap().sand, 1);
    assert!(app.world().get_entity(item).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world()
            .resource::<TaskNotificationReceipts>()
            .completed_domain
            .len(),
        1
    );
}

#[test]
fn mixer_wheelbarrow_moving_does_not_unload_remotely() {
    let (mut app, worker, mixer, cart, item) = mixer_fixture();
    let far = WorldMap::grid_to_world(75, 49);
    app.world_mut()
        .get_mut::<Transform>(worker)
        .unwrap()
        .translation = far.extend(0.0);
    app.update();
    assert!(matches!(
        app.world().get::<AssignedTask>(worker),
        Some(AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
            phase: HaulWithWheelbarrowPhase::GoingToDestination,
            ..
        }))
    ));
    assert_eq!(app.world().get::<Inventory>(worker).unwrap().0, Some(cart));
    assert_eq!(app.world().get::<LoadedIn>(item).unwrap().0, cart);
    assert_eq!(app.world().get::<MudMixerStorage>(mixer).unwrap().sand, 0);
    assert!(
        app.world()
            .resource::<TaskNotificationReceipts>()
            .completed_domain
            .is_empty()
    );
}

#[test]
fn mixer_wheelbarrow_deferred_preserves_even_a_nearby_stale_destination() {
    let (mut app, worker, mixer, cart, item) = mixer_fixture();
    app.world_mut()
        .get_mut::<Path>(worker)
        .unwrap()
        .waypoints
        .clear();
    app.world_mut()
        .insert_resource(RuntimePathSearchBudget::new(0));
    let before = format!("{:?}", app.world().get::<AssignedTask>(worker).unwrap());
    let dest = app.world().get::<Destination>(worker).unwrap().0;
    app.world_mut().clear_trackers();
    app.update();
    assert_eq!(
        format!("{:?}", app.world().get::<AssignedTask>(worker).unwrap()),
        before
    );
    assert_eq!(app.world().get::<Destination>(worker).unwrap().0, dest);
    assert_eq!(app.world().get::<LoadedIn>(item).unwrap().0, cart);
    assert_eq!(app.world().get::<MudMixerStorage>(mixer).unwrap().sand, 0);
    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert!(receipts.completed_domain.is_empty());
    assert!(receipts.reservation_ops.is_empty());
    assert_component_unchanged::<AssignedTask>(app.world_mut(), worker);
    assert_component_unchanged::<Destination>(app.world_mut(), worker);
    assert_component_unchanged::<Path>(app.world_mut(), worker);
}

#[test]
fn mixer_wheelbarrow_unreachable_or_missing_target_keeps_abort_cleanup() {
    for missing in [false, true] {
        let (mut app, worker, mixer, cart, item) = mixer_fixture();
        app.world_mut()
            .get_mut::<Path>(worker)
            .unwrap()
            .waypoints
            .clear();
        if missing {
            app.world_mut().despawn(mixer);
        } else {
            let far = WorldMap::grid_to_world(75, 49);
            app.world_mut()
                .get_mut::<Transform>(worker)
                .unwrap()
                .translation = far.extend(0.0);
            for y in 0..hw_core::constants::MAP_HEIGHT {
                app.world_mut()
                    .resource_mut::<WorldMap>()
                    .add_grid_obstacle((73, y));
            }
        }
        app.update();
        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<Inventory>(worker).unwrap().0.is_none());
        assert!(app.world().get::<WorkingOn>(worker).is_none());
        assert!(app.world().get::<ActiveTaskIdentity>(worker).is_none());
        assert!(app.world().get::<LoadedIn>(item).is_none());
        assert_eq!(
            *app.world().get::<Visibility>(item).unwrap(),
            Visibility::Visible
        );
        assert!(app.world().get_entity(cart).is_ok());
        assert!(
            app.world()
                .resource::<TaskNotificationReceipts>()
                .completed_domain
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<TaskNotificationReceipts>()
                .completed_visual
                .is_empty()
        );
        assert_eq!(
            app.world()
                .resource::<TaskNotificationReceipts>()
                .reservation_ops,
            vec![
                ResourceReservationOp::ReleaseSource {
                    source: cart.into(),
                    amount: 1
                },
                ResourceReservationOp::ReleaseMixerDestination {
                    target: mixer,
                    resource_type: ResourceType::Sand
                }
            ]
        );
        if !missing {
            assert_eq!(app.world().get::<MudMixerStorage>(mixer).unwrap().sand, 0);
        }
    }
}
