use super::*;
use hw_core::constants::TILE_SIZE;
use hw_jobs::events::TankWaterDelivered;
use hw_logistics::{BelongsTo, BucketStorage, Stockpile};

fn district() -> ContractDistrict {
    ContractDistrict {
        min: IVec2::splat(30),
        max: IVec2::splat(40),
    }
}

fn storage_at(grid: (i32, i32)) -> Transform {
    Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0))
}

fn station(world: &mut World, (x, y): (i32, i32)) -> (Entity, Entity) {
    let center = WorldMap::grid_to_world(x, y) + Vec2::splat(TILE_SIZE * 0.5);
    let tank = world
        .spawn((
            Building {
                kind: BuildingType::Tank,
                ..default()
            },
            Stockpile {
                capacity: 50,
                resource_type: Some(hw_logistics::ResourceType::Water),
            },
            WaterworksTarget(1),
            Transform::from_translation(center.extend(0.0)),
        ))
        .id();
    let [left, _] = [x + 2, x + 3].map(|storage_x| {
        world
            .spawn((
                BelongsTo(tank),
                BucketStorage,
                Stockpile {
                    capacity: 10,
                    resource_type: None,
                },
                storage_at((storage_x, y)),
            ))
            .id()
    });
    (tank, left)
}

fn setup() -> (App, Entity, Entity, Entity) {
    let mut app = App::new();
    app.init_resource::<GameTime>()
        .init_resource::<DreamPool>()
        .add_message::<UserFacingNotification>()
        .add_observer(inspect_delivery);
    let mut contract = WaterworksContract::new(district(), 100.0);
    contract.designation = 1;
    let root = app.world_mut().spawn(contract).id();
    let (tank, storage) = station(app.world_mut(), (32, 32));
    (app, root, tank, storage)
}

#[test]
fn waterworks_eligible_station_gets_one_payment_at_its_delivery_time() {
    for (offset, expected) in [(-1.0, FULL_REWARD), (0.0, FULL_REWARD), (1.0, LATE_REWARD)] {
        let (mut app, root, tank, _) = setup();
        let world = app.world_mut();
        let deadline = world.get::<WaterworksContract>(root).unwrap().deadline;
        world.resource_mut::<GameTime>().seconds = deadline + offset;
        let other = world
            .spawn(Building {
                kind: BuildingType::Tank,
                ..default()
            })
            .id();
        world.trigger(TankWaterDelivered { tank: other });
        claim(world);
        assert_eq!(
            world.get::<WaterworksContract>(root).unwrap().outcome,
            ContractOutcome::Active
        );
        let balance = world.resource::<DreamPool>().points;
        world.trigger(TankWaterDelivered { tank });
        world.flush();
        assert!(world.get::<WaterworksTarget>(tank).is_none());
        world.resource_mut::<GameTime>().seconds = deadline + 86400.;
        world.despawn(tank);
        claim(world);
        claim(world);
        assert_eq!(
            world.resource::<DreamPool>().points,
            balance + expected as f32
        );
        assert_eq!(
            world.get::<WaterworksContract>(root).unwrap().outcome,
            ContractOutcome::Paid {
                at: deadline + offset,
                reward: expected
            }
        );
    }
}

#[test]
fn waterworks_district_checks_entire_storage_and_drops_moved_target() {
    let (mut app, root, tank, storage) = setup();
    app.world_mut().resource_mut::<GameTime>().seconds = 101.;
    assert!(validate_candidate(app.world()).is_ok());
    app.world_mut()
        .entity_mut(storage)
        .insert(storage_at((41, 32)));
    assert!(validate_candidate(app.world()).is_err());
    app.world_mut().trigger(TankWaterDelivered { tank });
    assert_eq!(
        app.world().get::<WaterworksContract>(root).unwrap().outcome,
        ContractOutcome::Active
    );
    app.add_systems(Update, release_moved_target);
    app.update();
    assert!(app.world().get::<WaterworksTarget>(tank).is_none());
    assert_eq!(
        app.world()
            .get::<WaterworksContract>(root)
            .unwrap()
            .designation,
        1
    );
    assert!(validate_candidate(app.world()).is_ok());
}

#[test]
fn waterworks_clock_crossing_is_scored_at_the_simulated_delivery_tick() {
    use crate::systems::GameSystemSet;
    let (mut app, root, tank, _) = setup();
    let deadline = app
        .world()
        .get::<WaterworksContract>(root)
        .unwrap()
        .deadline;
    app.world_mut().resource_mut::<GameTime>().seconds = deadline - 30.;
    let mut time = Time::<Virtual>::default();
    time.advance_by(std::time::Duration::from_secs(1));
    app.insert_resource(time)
        .configure_sets(
            Update,
            (GameSystemSet::Spatial, GameSystemSet::Logic).chain(),
        )
        .add_systems(
            Update,
            crate::systems::time::game_time_system.in_set(GameSystemSet::Spatial),
        )
        .add_systems(
            Update,
            (move |mut commands: Commands| commands.trigger(TankWaterDelivered { tank }))
                .in_set(GameSystemSet::Logic),
        );
    app.update();
    assert_eq!(
        app.world().get::<WaterworksContract>(root).unwrap().outcome,
        ContractOutcome::Inspected {
            at: deadline + 30.,
            reward: LATE_REWARD
        }
    );
}

#[test]
fn waterworks_rejects_orphan_and_shared_record_in_staging() {
    let (mut app, root, tank, _) = setup();
    let world = app.world_mut();
    world.resource_mut::<GameTime>().seconds = 101.;
    world.entity_mut(tank).remove::<Stockpile>();
    assert!(validate_candidate(world).is_err());
    let record = *world.get::<WaterworksContract>(root).unwrap();
    world.despawn(root);
    assert!(validate_candidate(world).is_err());
    world.entity_mut(tank).insert(record);
    assert!(validate_candidate(world).is_err());
}

#[test]
fn ordinary_delivery_beats_a_guided_project_and_two_deliveries_only_pay_once() {
    let (mut app, root, guided, _) = setup();
    let world = app.world_mut();
    world.resource_mut::<GameTime>().seconds = 101.;
    let (ordinary, _) = station(world, (36, 35));
    world.entity_mut(ordinary).remove::<WaterworksTarget>();
    world.trigger(TankWaterDelivered { tank: ordinary });
    world.trigger(TankWaterDelivered { tank: guided });
    world.flush();
    assert!(world.get::<WaterworksTarget>(guided).is_none());
    assert!(validate_candidate(world).is_ok());
    assert_eq!(
        world.get::<WaterworksContract>(root).unwrap().outcome,
        ContractOutcome::Inspected {
            at: 101.,
            reward: FULL_REWARD
        }
    );
    let balance = world.resource::<DreamPool>().points;
    claim(world);
    claim(world);
    assert_eq!(
        world.resource::<DreamPool>().points,
        balance + FULL_REWARD as f32
    );
}

#[test]
fn existing_station_only_completes_on_a_new_delivery_after_acceptance() {
    let (mut app, root, tank, _) = setup();
    let world = app.world_mut();
    world.despawn(root);
    world.entity_mut(tank).remove::<WaterworksTarget>();
    world.trigger(TankWaterDelivered { tank });
    let id = world.spawn(WaterworksContract::new(district(), 100.)).id();
    world.resource_mut::<GameTime>().seconds = 101.;
    claim(world);
    assert_eq!(
        world.get::<WaterworksContract>(id).unwrap().outcome,
        ContractOutcome::Active
    );
    world.trigger(TankWaterDelivered { tank });
    claim(world);
    let record = world.get::<WaterworksContract>(id).unwrap();
    assert_eq!(record.designation, 0);
    assert_eq!(
        record.outcome,
        ContractOutcome::Paid {
            at: 101.,
            reward: FULL_REWARD
        }
    );
    assert!(validate_candidate(world).is_ok());
}

#[test]
fn todo_follows_ordinary_sites_and_ignores_misplaced_storage() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut app, _, tank, left) = setup();
    let world = app.world_mut();
    world.entity_mut(tank).remove::<WaterworksTarget>();
    let progress = |stations: Stations| stations.progress(district());
    assert_eq!(
        world.run_system_once(progress).unwrap(),
        StationProgress::Water
    );
    let right = world
        .query::<(Entity, &BelongsTo)>()
        .iter(world)
        .find(|(id, owner)| *id != left && owner.0 == tank)
        .unwrap()
        .0;
    for (a, b) in [
        ((32, 32), (33, 32)),
        ((38, 38), (39, 38)),
        ((40, 32), (41, 32)),
    ] {
        world.entity_mut(left).insert(storage_at(a));
        world.entity_mut(right).insert(storage_at(b));
        assert_eq!(
            world.run_system_once(progress).unwrap(),
            StationProgress::Site
        );
        world.trigger(TankWaterDelivered { tank });
        assert_eq!(
            world
                .query::<&WaterworksContract>()
                .single(world)
                .unwrap()
                .outcome,
            ContractOutcome::Active
        );
    }
}

#[test]
fn corrupt_station_position_is_rejected_before_expanding_its_footprint() {
    let (mut app, _, tank, _) = setup();
    let world = app.world_mut();
    world.resource_mut::<GameTime>().seconds = 101.;
    world
        .entity_mut(tank)
        .insert(Transform::from_xyz(f32::MAX, f32::MAX, 0.));
    assert!(validate_candidate(world).is_err());
    world.trigger(TankWaterDelivered { tank });
    assert_eq!(
        world
            .query::<&WaterworksContract>()
            .single(world)
            .unwrap()
            .outcome,
        ContractOutcome::Active
    );
}
