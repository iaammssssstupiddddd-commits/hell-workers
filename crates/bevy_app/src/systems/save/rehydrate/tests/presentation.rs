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
