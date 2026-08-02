use super::*;

#[test]
fn soul_proxy_projection_maps_world_y_to_negative_z() {
    let transform = Transform::from_xyz(12.0, -7.5, 99.0);

    assert_eq!(soul_proxy_transform(&transform), Vec3::new(12.0, 0.0, 7.5));
}

#[test]
fn familiar_proxy_projection_includes_visual_hover_offset() {
    let transform = Transform::from_xyz(3.0, 8.0, 0.0);
    let visual_offset = FamiliarVisualOffset {
        hover_offset: 1.25,
        tilt_radians: 0.0,
    };

    assert_eq!(
        familiar_proxy_transform(&transform, Some(&visual_offset)),
        Vec3::new(3.0, 0.0, -9.25)
    );
}

#[test]
fn familiar_proxy_reads_hover_offset_from_the_owned_visual_child() {
    let mut app = App::new();
    app.init_resource::<SoulProxyOwnerCache>().add_systems(
        Update,
        (
            register_familiar_proxy_3d_system,
            sync_familiar_proxy_3d_system,
        )
            .chain(),
    );

    app.world_mut().spawn((Camera3dRtt, Transform::default()));
    let owner = app
        .world_mut()
        .spawn((Familiar::default(), Transform::from_xyz(3.0, 8.0, 0.0)))
        .id();
    let visual = app
        .world_mut()
        .spawn((
            FamiliarVisualOwner { owner },
            FamiliarVisualOffset::default(),
        ))
        .id();
    let proxy = app
        .world_mut()
        .spawn((FamiliarProxy3d { owner }, Transform::default()))
        .id();

    app.update();
    app.world_mut()
        .entity_mut(visual)
        .get_mut::<FamiliarVisualOffset>()
        .unwrap()
        .hover_offset = 1.25;
    app.update();

    assert_eq!(
        app.world().get::<Transform>(proxy).unwrap().translation,
        Vec3::new(3.0, 0.0, -9.25)
    );
}

#[test]
fn soul_proxy_cache_registers_and_cleans_up_with_owner() {
    let mut app = App::new();
    app.init_resource::<SoulProxyOwnerCache>().add_systems(
        Update,
        (register_soul_proxy_3d_system, cleanup_soul_proxy_3d_system).chain(),
    );

    let owner = app.world_mut().spawn(DamnedSoul::default()).id();
    let proxy = app
        .world_mut()
        .spawn(SoulProxy3d {
            owner,
            billboard: false,
        })
        .id();

    app.update();
    assert_eq!(
        app.world()
            .resource::<SoulProxyOwnerCache>()
            .soul_proxy
            .get(&owner),
        Some(&proxy)
    );

    app.world_mut().despawn(owner);
    app.update();

    assert!(!app.world().entities().contains(proxy));
    assert!(
        !app.world()
            .resource::<SoulProxyOwnerCache>()
            .soul_proxy
            .contains_key(&owner)
    );
}
