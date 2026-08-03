use super::*;

#[test]
fn prerequisites_are_reported_before_rehydrate_mutates_the_world() {
    let mut world = World::new();
    let durable_entity = world.spawn(DamnedSoul::default()).id();

    assert_eq!(
        validate_rehydrate_prerequisites(&world)
            .unwrap_err()
            .missing_resources,
        vec![
            std::any::type_name::<crate::assets::GameAssets>(),
            std::any::type_name::<crate::plugins::startup::Building3dHandles>(),
            std::any::type_name::<hw_core::visual::SoulTaskHandles>(),
            std::any::type_name::<Time<Virtual>>(),
            std::any::type_name::<WorldMap>(),
        ],
    );
    assert!(world.get_entity(durable_entity).is_ok());
}

#[test]
fn presentation_cleanup_removes_only_rehydrate_owned_shells() {
    let mut world = World::new();
    world.init_resource::<hw_visual::SoulProxyOwnerCache>();

    let soul_proxy = world
        .spawn(hw_visual::visual3d::SoulProxy3d {
            owner: Entity::PLACEHOLDER,
            billboard: false,
        })
        .id();
    let mask_proxy = world
        .spawn(hw_visual::visual3d::SoulMaskProxy3d {
            owner: Entity::PLACEHOLDER,
        })
        .id();
    let shadow_proxy = world
        .spawn(hw_visual::visual3d::SoulShadowProxy3d {
            owner: Entity::PLACEHOLDER,
        })
        .id();
    let familiar_proxy = world
        .spawn(hw_visual::visual3d::FamiliarProxy3d {
            owner: Entity::PLACEHOLDER,
        })
        .id();
    let building_visual = world
        .spawn(hw_visual::visual3d::Building3dVisual {
            owner: Entity::PLACEHOLDER,
        })
        .id();
    let range_indicator = world
        .spawn(crate::entities::familiar::FamiliarRangeIndicator(
            Entity::PLACEHOLDER,
        ))
        .id();
    let durable_entity = world.spawn(Tree).id();

    {
        let mut cache = world.resource_mut::<hw_visual::SoulProxyOwnerCache>();
        cache.soul_proxy.insert(Entity::PLACEHOLDER, soul_proxy);
    }

    clear_rehydrate_presentation(&mut world);

    for entity in [
        soul_proxy,
        mask_proxy,
        shadow_proxy,
        familiar_proxy,
        building_visual,
        range_indicator,
    ] {
        assert!(world.get_entity(entity).is_err());
    }
    assert!(world.get_entity(durable_entity).is_ok());
    assert!(
        world
            .resource::<hw_visual::SoulProxyOwnerCache>()
            .soul_proxy
            .is_empty()
    );
}

#[test]
fn soul_shell_rehydrate_is_idempotent() {
    let mut world = World::new();
    let soul = world
        .spawn((
            DamnedSoul::default(),
            SoulIdentity {
                name: "test soul".to_string(),
                gender: Gender::Male,
            },
            Transform::from_xyz(2.0, 3.0, 0.0),
        ))
        .id();
    let handles = empty_building_3d_handles();

    assert_eq!(rehydrate_soul_shells(&mut world, &handles), 1);
    world.flush();
    assert!(
        world
            .get::<crate::entities::damned_soul::Destination>(soul)
            .is_some()
    );
    assert_eq!(
        world
            .query::<&hw_visual::visual3d::SoulProxy3d>()
            .iter(&world)
            .count(),
        1
    );
    assert_eq!(
        world
            .query::<&hw_visual::visual3d::SoulMaskProxy3d>()
            .iter(&world)
            .count(),
        1
    );
    assert_eq!(
        world
            .query::<&hw_visual::visual3d::SoulShadowProxy3d>()
            .iter(&world)
            .count(),
        1
    );

    assert_eq!(rehydrate_soul_shells(&mut world, &handles), 0);
    world.flush();
    assert_eq!(
        world
            .query::<&hw_visual::visual3d::SoulProxy3d>()
            .iter(&world)
            .count(),
        1
    );
    assert_eq!(
        world
            .query::<&hw_visual::visual3d::SoulMaskProxy3d>()
            .iter(&world)
            .count(),
        1
    );
    assert_eq!(
        world
            .query::<&hw_visual::visual3d::SoulShadowProxy3d>()
            .iter(&world)
            .count(),
        1
    );
}

#[test]
fn resting_soul_shell_remains_hidden_after_rehydrate() {
    use hw_core::relationships::RestingIn;
    use hw_core::soul::{IdleBehavior, IdleState};
    use hw_jobs::RestArea;

    let mut world = World::new();
    let rest_area = world.spawn(RestArea { capacity: 1 }).id();
    let soul = world
        .spawn((
            DamnedSoul::default(),
            IdleState {
                behavior: IdleBehavior::Resting,
                ..default()
            },
            RestingIn(rest_area),
            Transform::from_xyz(2.0, 3.0, 0.0),
        ))
        .id();
    world.flush();

    assert_eq!(
        rehydrate_soul_shells(&mut world, &empty_building_3d_handles()),
        1
    );
    world.flush();

    assert_eq!(world.get::<Visibility>(soul), Some(&Visibility::Hidden));
}

#[test]
fn stored_water_shell_is_hidden_while_a_stored_bucket_remains_visible() {
    use bevy::asset::{AssetApp, AssetPlugin};
    use hw_core::relationships::StoredIn;
    use hw_logistics::{ResourceItem, Stockpile};

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<Gltf>()
        .init_asset::<WorldAsset>();

    let asset_server = app.world().resource::<AssetServer>().clone();
    let game_assets = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        crate::plugins::startup::create_game_assets(&asset_server, &mut images)
    };
    let storage = app
        .world_mut()
        .spawn((
            Stockpile {
                capacity: 2,
                resource_type: Some(ResourceType::Water),
            },
            Transform::default(),
        ))
        .id();
    let bucket_storage = app
        .world_mut()
        .spawn((
            Stockpile {
                capacity: 1,
                resource_type: None,
            },
            Transform::default(),
        ))
        .id();
    let water = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::Water),
            StoredIn(storage),
            Transform::default(),
        ))
        .id();
    let bucket = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::BucketEmpty),
            StoredIn(bucket_storage),
            Transform::default(),
        ))
        .id();
    app.world_mut().flush();

    rehydrate_shells(
        app.world_mut(),
        &game_assets,
        &empty_building_3d_handles(),
        &empty_soul_task_handles(),
    );
    app.world_mut().flush();

    assert_eq!(
        app.world().get::<Visibility>(water),
        Some(&Visibility::Hidden)
    );
    assert_ne!(
        app.world().get::<Visibility>(bucket),
        Some(&Visibility::Hidden)
    );
}

#[test]
fn familiar_shell_rehydrate_restores_patrol_from_durable_task_area() {
    use bevy::asset::{AssetApp, AssetPlugin};
    use hw_core::familiar::{ActiveCommand, Familiar, FamiliarCommand};

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<Gltf>()
        .init_asset::<WorldAsset>();

    let asset_server = app.world().resource::<AssetServer>().clone();
    let game_assets = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        crate::plugins::startup::create_game_assets(&asset_server, &mut images)
    };
    let handles_3d = empty_building_3d_handles();
    let soul_handles = empty_soul_task_handles();
    let patrol = app
        .world_mut()
        .spawn((
            Familiar::default(),
            Transform::default(),
            TaskArea::from_points(Vec2::splat(-32.0), Vec2::splat(32.0)),
        ))
        .id();
    let idle = app
        .world_mut()
        .spawn((Familiar::default(), Transform::from_xyz(64.0, 0.0, 0.0)))
        .id();

    rehydrate_shells(app.world_mut(), &game_assets, &handles_3d, &soul_handles);
    app.world_mut().flush();

    assert_eq!(
        app.world().get::<ActiveCommand>(patrol).unwrap().command,
        FamiliarCommand::Patrol
    );
    assert_eq!(
        app.world().get::<ActiveCommand>(idle).unwrap().command,
        FamiliarCommand::Idle
    );
}

#[test]
fn familiar_rehydrate_keeps_two_rest_area_supply_sources_active() {
    use bevy::asset::{AssetApp, AssetPlugin};
    use hw_core::familiar::Familiar;
    use hw_core::logistics::ResourceType;
    use hw_core::relationships::ManagedBy;
    use hw_familiar_ai::familiar_ai::decide::blueprint_auto_gather::{
        BlueprintAutoGatherTimer, blueprint_auto_gather_system,
    };
    use hw_jobs::{TargetBlueprint, TaskSlots};
    use hw_logistics::transport_request::{
        TransportPriority, TransportRequest, TransportRequestKind,
    };
    use hw_world::WalkabilityConnectivityCache;

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<Gltf>()
        .init_asset::<WorldAsset>()
        .init_resource::<WorldMap>()
        .init_resource::<WalkabilityConnectivityCache>()
        .init_resource::<BlueprintAutoGatherTimer>()
        .add_systems(Update, blueprint_auto_gather_system);

    let asset_server = app.world().resource::<AssetServer>().clone();
    let game_assets = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        crate::plugins::startup::create_game_assets(&asset_server, &mut images)
    };
    let handles_3d = empty_building_3d_handles();
    let soul_handles = empty_soul_task_handles();
    let familiar_pos = WorldMap::grid_to_world(40, 40);
    let familiar = app
        .world_mut()
        .spawn((
            Familiar::default(),
            Transform::from_translation(familiar_pos.extend(0.0)),
            TaskArea::from_points(
                WorldMap::grid_to_world(35, 35),
                WorldMap::grid_to_world(45, 45),
            ),
        ))
        .id();

    rehydrate_shells(app.world_mut(), &game_assets, &handles_3d, &soul_handles);
    app.world_mut().flush();

    for occupied_grid in [(38, 38), (42, 38)] {
        let blueprint = app
            .world_mut()
            .spawn(Blueprint::new(BuildingType::RestArea, vec![occupied_grid]))
            .id();
        app.world_mut().spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
                anchor: blueprint,
                resource_type: ResourceType::Wood,
                issued_by: familiar,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TargetBlueprint(blueprint),
        ));
    }
    let trees = [(39, 41), (41, 41)].map(|grid| {
        app.world_mut()
            .spawn((
                Tree,
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            ))
            .id()
    });

    app.update();

    for tree in trees {
        let tree_ref = app.world().entity(tree);
        assert_eq!(
            tree_ref
                .get::<Designation>()
                .map(|designation| designation.work_type),
            Some(WorkType::Chop)
        );
        assert_eq!(
            tree_ref.get::<ManagedBy>().map(|owner| owner.0),
            Some(familiar)
        );
        assert!(tree_ref.contains::<TaskSlots>());
    }
}
