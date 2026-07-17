use super::*;

#[test]
fn idle_guard_leaves_task_context_components_unchanged() {
    let mut world = World::new();
    let soul = spawn_task_execution_soul(&mut world, AssignedTask::None);
    world.clear_trackers();

    let mut schedule = Schedule::default();
    schedule.add_systems(idle_guard_probe_system);
    schedule.run(&mut world);

    assert_component_unchanged::<DamnedSoul>(&mut world, soul);
    assert_component_unchanged::<AssignedTask>(&mut world, soul);
    assert_component_unchanged::<Destination>(&mut world, soul);
    assert_component_unchanged::<Path>(&mut world, soul);
    assert_component_unchanged::<Inventory>(&mut world, soul);
}

#[test]
fn active_task_without_working_on_remains_in_task_execution_query() {
    let mut world = World::new();
    world.init_resource::<ActiveTaskProbe>();
    let soul = spawn_task_execution_soul(
        &mut world,
        AssignedTask::GeneratePower(GeneratePowerData {
            tile: Entity::PLACEHOLDER,
            tile_pos: Vec2::ZERO,
            phase: GeneratePowerPhase::GoingToTile,
        }),
    );
    world.entity_mut(soul).insert(ActiveTaskIdentity::new(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        WorkType::GeneratePower,
    ));
    world.clear_trackers();

    let mut schedule = Schedule::default();
    schedule.add_systems(active_task_without_working_on_probe_system);
    schedule.run(&mut world);

    assert!(
        world
            .resource::<ActiveTaskProbe>()
            .reached_without_working_on
    );
}

#[test]
fn exhausted_task_path_budget_defers_without_changing_task_or_reservations() {
    let mut app = task_execution_test_app();
    app.world_mut()
        .insert_resource(RuntimePathSearchBudget::new(0));

    let tile_pos = WorldMap::grid_to_world(20, 20);
    let tile = app
        .world_mut()
        .spawn((
            Transform::from_translation(tile_pos.extend(0.0)),
            Designation {
                work_type: WorkType::GeneratePower,
            },
        ))
        .id();
    let assignment = app.world_mut().spawn_empty().id();
    let initial_destination = WorldMap::grid_to_world(2, 2);
    let initial_waypoints = vec![WorldMap::grid_to_world(3, 3)];
    let soul = app
        .world_mut()
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            AssignedTask::GeneratePower(GeneratePowerData {
                tile,
                tile_pos,
                phase: GeneratePowerPhase::GoingToTile,
            }),
            Destination(initial_destination),
            Path {
                waypoints: initial_waypoints.clone(),
                current_index: 0,
                planned_destination: Some(initial_destination),
                validated_obstacle_version: 7,
            },
            Inventory::default(),
            ActiveTaskIdentity::new(assignment, tile, WorkType::GeneratePower),
            WorkingOn(tile),
        ))
        .id();

    app.world_mut().clear_trackers();
    app.update();

    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::GeneratePower(GeneratePowerData {
            tile: actual_tile,
            tile_pos: actual_pos,
            phase: GeneratePowerPhase::GoingToTile,
        })) if *actual_tile == tile && *actual_pos == tile_pos
    ));
    assert_eq!(
        app.world().get::<Destination>(soul).unwrap().0,
        initial_destination
    );
    let path = app.world().get::<Path>(soul).unwrap();
    assert_eq!(path.waypoints, initial_waypoints);
    assert_eq!(path.current_index, 0);
    assert_eq!(path.planned_destination, Some(initial_destination));
    assert_eq!(path.validated_obstacle_version, 7);
    assert!(
        app.world()
            .get::<ActiveTaskIdentity>(soul)
            .is_some_and(|identity| identity.current_target_entity == tile)
    );
    assert!(
        app.world()
            .get::<WorkingOn>(soul)
            .is_some_and(|working_on| working_on.0 == tile)
    );
    assert!(
        app.world()
            .resource::<TaskNotificationReceipts>()
            .reservation_ops
            .is_empty()
    );
    assert_eq!(app.world().resource::<RuntimePathSearchBudget>().used(), 0);
    assert_component_unchanged::<DamnedSoul>(app.world_mut(), soul);
    assert_component_unchanged::<AssignedTask>(app.world_mut(), soul);
    assert_component_unchanged::<Destination>(app.world_mut(), soul);
    assert_component_unchanged::<Path>(app.world_mut(), soul);
    assert_component_unchanged::<Inventory>(app.world_mut(), soul);
}

#[test]
fn identity_preflight_requires_identity_and_rejects_present_target_mismatch() {
    let mut world = World::new();
    let assignment = world.spawn_empty().id();
    let current_target = world.spawn_empty().id();
    let other_target = world.spawn_empty().id();
    let identity = ActiveTaskIdentity::new(assignment, current_target, WorkType::Chop);

    assert!(!has_consistent_task_identity(None, None));
    assert!(!has_consistent_task_identity(Some(&identity), None));
    assert!(has_consistent_task_identity(
        Some(&identity),
        Some(&WorkingOn(current_target))
    ));
    assert!(!has_consistent_task_identity(
        Some(&identity),
        Some(&WorkingOn(other_target))
    ));

    let mut detached = identity;
    detached.detach_from_working_on();
    assert!(has_consistent_task_identity(Some(&detached), None));
    assert!(!has_consistent_task_identity(
        Some(&detached),
        Some(&WorkingOn(current_target))
    ));
}
