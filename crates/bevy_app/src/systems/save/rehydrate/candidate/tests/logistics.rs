use super::*;

#[test]
fn task_logistics_validation_accepts_consistent_durable_owners() {
    let mut world = candidate_world();
    let familiar = world.spawn(Familiar::default()).id();
    let task = world
        .spawn((
            hw_jobs::Designation {
                work_type: hw_core::jobs::WorkType::Haul,
            },
            ManagedBy(familiar),
        ))
        .id();
    let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
    let wheelbarrow = world
        .spawn((
            ResourceItem(hw_core::logistics::ResourceType::Wheelbarrow),
            Wheelbarrow { capacity: 2 },
            BelongsTo(parking),
            ParkedAt(parking),
            Transform::default(),
        ))
        .id();
    world.spawn((
        ResourceItem(hw_core::logistics::ResourceType::Sand),
        LoadedIn(wheelbarrow),
        Transform::default(),
    ));
    world.flush();

    validate_task_logistics_candidate(&world).unwrap();
    assert!(world.get::<ManagedTasks>(familiar).unwrap().contains(task));
    assert!(world.get::<LoadedItems>(wheelbarrow).is_some());
    assert!(world.get::<ParkedWheelbarrows>(parking).is_some());
}

#[test]
fn task_logistics_validation_rejects_missing_inventory_entity() {
    let mut world = candidate_world();
    let missing = world.spawn_empty().id();
    world.despawn(missing);
    world.spawn((
        DamnedSoul::default(),
        Inventory(Some(missing)),
        Transform::default(),
    ));

    assert!(
        validate_task_logistics_candidate(&world)
            .unwrap_err()
            .contains("missing entity")
    );
}

#[test]
fn task_logistics_validation_rejects_over_capacity_loaded_items() {
    let mut world = candidate_world();
    let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
    let wheelbarrow = world
        .spawn((
            ResourceItem(hw_core::logistics::ResourceType::Wheelbarrow),
            Wheelbarrow { capacity: 1 },
            BelongsTo(parking),
            Transform::default(),
        ))
        .id();
    for _ in 0..2 {
        world.spawn((
            ResourceItem(hw_core::logistics::ResourceType::Wood),
            LoadedIn(wheelbarrow),
            Transform::default(),
        ));
    }
    world.flush();

    assert!(
        validate_task_logistics_candidate(&world)
            .unwrap_err()
            .contains("exceeding capacity")
    );
}

#[test]
fn task_logistics_validation_rejects_non_loadable_wheelbarrow_cargo() {
    let mut world = candidate_world();
    let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
    let wheelbarrow = world
        .spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow { capacity: 1 },
            BelongsTo(parking),
            Transform::default(),
        ))
        .id();
    world.spawn((
        ResourceItem(ResourceType::BucketEmpty),
        LoadedIn(wheelbarrow),
        Transform::default(),
    ));
    world.flush();

    assert!(
        validate_task_logistics_candidate(&world)
            .unwrap_err()
            .contains("non-loadable resource type")
    );
}

#[test]
fn task_logistics_validation_rejects_wheelbarrow_in_a_durable_container() {
    let mut world = candidate_world();
    let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
    let stockpile = world
        .spawn(Stockpile {
            capacity: 1,
            resource_type: None,
        })
        .id();
    world.spawn((
        ResourceItem(ResourceType::Wheelbarrow),
        Wheelbarrow { capacity: 1 },
        BelongsTo(parking),
        StoredIn(stockpile),
        Transform::default(),
    ));
    world.flush();

    assert!(
        validate_task_logistics_candidate(&world)
            .unwrap_err()
            .contains("cannot have a durable container owner")
    );
}

#[test]
fn task_logistics_validation_rejects_missing_belongs_to_owner() {
    let mut world = candidate_world();
    let missing = world.spawn_empty().id();
    world.despawn(missing);
    world.spawn((
        ResourceItem(ResourceType::Wood),
        BelongsTo(missing),
        Transform::default(),
    ));

    assert!(
        validate_task_logistics_candidate(&world)
            .unwrap_err()
            .contains("references missing owner")
    );
}

#[test]
fn task_logistics_validation_rejects_stockpile_content_type_mismatch() {
    let mut world = candidate_world();
    let stockpile = world
        .spawn(Stockpile {
            capacity: 2,
            resource_type: Some(ResourceType::Wood),
        })
        .id();
    world.spawn((
        ResourceItem(ResourceType::Rock),
        StoredIn(stockpile),
        Transform::default(),
    ));
    world.flush();

    assert!(
        validate_task_logistics_candidate(&world)
            .unwrap_err()
            .contains("resource type does not match")
    );
}

#[test]
fn task_logistics_validation_recognizes_bucket_storage_from_durable_tank_owner() {
    let mut world = candidate_world();
    let tank = world
        .spawn(Building {
            kind: BuildingType::Tank,
            ..default()
        })
        .id();
    let storage = world
        .spawn((
            Stockpile {
                capacity: 2,
                resource_type: None,
            },
            BelongsTo(tank),
        ))
        .id();
    world.spawn((
        ResourceItem(ResourceType::BucketEmpty),
        BelongsTo(tank),
        StoredIn(storage),
        Transform::default(),
    ));
    world.flush();

    validate_task_logistics_candidate(&world).unwrap();
    assert!(world.get::<BucketStorage>(storage).is_none());
}

#[test]
fn task_logistics_validation_rejects_noncanonical_manual_request_slots() {
    let mut world = candidate_world();
    let familiar = world.spawn(Familiar::default()).id();
    let stockpile = world
        .spawn(Stockpile {
            capacity: 2,
            resource_type: None,
        })
        .id();
    let source = world
        .spawn((
            ResourceItem(ResourceType::Wood),
            ManualHaulPinnedSource,
            Transform::default(),
        ))
        .id();
    world.spawn((
        TransportRequest {
            kind: TransportRequestKind::DepositToStockpile,
            anchor: stockpile,
            resource_type: ResourceType::Wood,
            issued_by: familiar,
            priority: hw_logistics::transport_request::TransportPriority::Normal,
            stockpile_group: Vec::new(),
        },
        TransportRequestFixedSource(source),
        ManualTransportRequest,
        TransportDemand {
            desired_slots: 2,
            inflight: 0,
        },
        TransportPolicy::default(),
        ManagedBy(familiar),
        Designation {
            work_type: WorkType::Haul,
        },
        TaskSlots::new(2),
        Priority(0),
        Transform::default(),
    ));
    world.flush();

    assert!(
        validate_task_logistics_candidate(&world)
            .unwrap_err()
            .contains("non-canonical active shape")
    );
}

#[test]
fn task_logistics_validation_rejects_invalid_mud_mixer_sources_counts_and_capacity() {
    let mut wrong_source = candidate_world();
    let mixer = wrong_source.spawn(MudMixerStorage::default()).id();
    let source = wrong_source.spawn(StoredByMixer(mixer)).id();
    assert_eq!(
        validate_task_logistics_candidate(&wrong_source).unwrap_err(),
        format!("durable item owner relation source {source:?} is not a ResourceItem")
    );

    let mut wrong_count = candidate_world();
    let mixer = wrong_count
        .spawn(MudMixerStorage {
            mud: 1,
            ..default()
        })
        .id();
    assert_eq!(
        validate_task_logistics_candidate(&wrong_count).unwrap_err(),
        format!("MudMixer {mixer:?} stores 1 mud unit(s), but owns 0 StasisMud item(s)")
    );

    let mut over_capacity = candidate_world();
    let mixer = over_capacity
        .spawn(MudMixerStorage {
            sand: MUD_MIXER_CAPACITY + 1,
            ..default()
        })
        .id();
    assert_eq!(
        validate_task_logistics_candidate(&over_capacity).unwrap_err(),
        format!("MudMixer {mixer:?} exceeds durable capacity")
    );
}

#[test]
fn task_logistics_validation_rejects_invalid_managed_relationships() {
    let mut wrong_role = candidate_world();
    let manager = wrong_role.spawn_empty().id();
    wrong_role.spawn(ManagedBy(manager));
    wrong_role.flush();
    assert_eq!(
        validate_task_logistics_candidate(&wrong_role).unwrap_err(),
        format!("ManagedBy target {manager:?} is neither a Familiar nor a Yard")
    );

    let mut asymmetric = candidate_world();
    let manager = asymmetric.spawn(Familiar::default()).id();
    let task = asymmetric.spawn_empty().id();
    asymmetric
        .entity_mut(manager)
        .insert(ManagedTasks::default());
    asymmetric
        .get_mut::<ManagedTasks>(manager)
        .unwrap()
        .collection_mut_risky()
        .push(task);
    assert_eq!(
        validate_task_logistics_candidate(&asymmetric).unwrap_err(),
        format!("ManagedTasks contains task {task:?} without the matching ManagedBy source")
    );
}

#[test]
fn task_logistics_validation_rejects_invalid_rest_relationships() {
    let mut wrong_source = candidate_world();
    let rest_area = wrong_source.spawn(RestArea { capacity: 1 }).id();
    let source = wrong_source.spawn(RestingIn(rest_area)).id();
    wrong_source.flush();
    assert_eq!(
        validate_task_logistics_candidate(&wrong_source).unwrap_err(),
        format!("RestingIn source {source:?} is not a DamnedSoul")
    );

    let mut wrong_target = candidate_world();
    let target = wrong_target.spawn_empty().id();
    wrong_target.spawn((
        DamnedSoul::default(),
        RestingIn(target),
        IdleState {
            behavior: IdleBehavior::Resting,
            ..default()
        },
    ));
    wrong_target.flush();
    assert_eq!(
        validate_task_logistics_candidate(&wrong_target).unwrap_err(),
        format!("RestingIn target {target:?} is not a RestArea")
    );

    let mut asymmetric = candidate_world();
    let rest_area = asymmetric.spawn(RestArea { capacity: 1 }).id();
    asymmetric.spawn((
        DamnedSoul::default(),
        RestingIn(rest_area),
        IdleState {
            behavior: IdleBehavior::Resting,
            ..default()
        },
    ));
    asymmetric.flush();
    asymmetric
        .get_mut::<RestAreaOccupants>(rest_area)
        .unwrap()
        .collection_mut_risky()
        .clear();
    assert_eq!(
        validate_task_logistics_candidate(&asymmetric).unwrap_err(),
        format!("RestingIn/RestAreaOccupants are not symmetric for {rest_area:?}")
    );
}
