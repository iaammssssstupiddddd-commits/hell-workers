use super::*;

#[test]
fn construction_shell_rehydrate_restores_saved_state_while_logic_is_paused() {
    let mut world = World::new();

    let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(3, 4), (4, 4)]);
    blueprint.progress = 0.25;
    blueprint.delivered_materials.insert(ResourceType::Wood, 1);
    let blueprint_entity = world
        .spawn((blueprint, Transform::from_xyz(3.0, 4.0, 0.0)))
        .id();

    let mut floor_site = floor_site(FloorConstructionPhase::Curing);
    floor_site.tiles_total = 3;
    floor_site.curing_remaining_secs = 42.0;
    let floor_site_entity = world.spawn(floor_site).id();
    let mut floor_tile = FloorTileBlueprint::new(floor_site_entity, (5, 6));
    floor_tile.state = FloorTileState::Pouring { progress: 73 };
    floor_tile.bones_delivered = 2;
    let floor_tile_entity = world.spawn(floor_tile).id();

    let mut wall_site = WallConstructionSite::new(
        TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
        Vec2::ZERO,
        4,
    );
    wall_site.phase = WallConstructionPhase::Coating;
    wall_site.tiles_framed = 4;
    wall_site.tiles_coated = 2;
    let wall_site_entity = world.spawn(wall_site).id();
    let mut wall_tile = WallTileBlueprint::new(wall_site_entity, (7, 8));
    wall_tile.state = WallTileState::Coating { progress: 61 };
    let wall_tile_entity = world.spawn(wall_tile).id();

    rehydrate_construction_shells(&mut world, &BlueprintSpriteHandles::default());
    world.flush();

    let blueprint_visual = world
        .get::<BlueprintVisualState>(blueprint_entity)
        .expect("Blueprint visual state should be restored before the next Logic run");
    assert_eq!(blueprint_visual.progress, 0.25);
    assert!(
        blueprint_visual
            .material_counts
            .contains(&(ResourceType::Wood, 1, 2))
    );
    assert_eq!(
        world
            .get::<BlueprintVisual>(blueprint_entity)
            .and_then(|visual| visual.last_delivered.get(&ResourceType::Wood)),
        Some(&1)
    );
    assert_eq!(
        world
            .get::<Sprite>(blueprint_entity)
            .and_then(|sprite| sprite.custom_size),
        Some(Vec2::splat(TILE_SIZE * 2.0))
    );
    assert_eq!(
        world.get::<Name>(blueprint_entity).map(Name::as_str),
        Some("Blueprint (Tank)")
    );

    let floor_site_visual = world
        .get::<FloorSiteVisualState>(floor_site_entity)
        .expect("floor site visual state should be restored");
    assert_eq!(
        floor_site_visual.phase,
        FloorConstructionPhaseMirror::Curing
    );
    assert_eq!(floor_site_visual.curing_remaining_secs, 42.0);
    assert_eq!(floor_site_visual.tiles_total, 3);
    let floor_tile_visual = world
        .get::<FloorTileVisualMirror>(floor_tile_entity)
        .expect("floor tile visual mirror should be restored");
    assert_eq!(floor_tile_visual.bones_delivered, 2);
    assert_eq!(
        floor_tile_visual.state,
        FloorTileStateMirror::Pouring { progress: 73 }
    );
    assert!(world.get::<Sprite>(floor_tile_entity).is_some());
    assert_eq!(
        world.get::<Name>(floor_site_entity).map(Name::as_str),
        Some("FloorConstructionSite")
    );
    assert_eq!(
        world.get::<Name>(floor_tile_entity).map(Name::as_str),
        Some("FloorTile(5,6)")
    );

    let wall_site_visual = world
        .get::<WallSiteVisualState>(wall_site_entity)
        .expect("wall site visual state should be restored");
    assert!(!wall_site_visual.phase_is_framing);
    assert_eq!(wall_site_visual.tiles_total, 4);
    assert_eq!(wall_site_visual.tiles_framed, 4);
    assert_eq!(wall_site_visual.tiles_coated, 2);
    let wall_tile_visual = world
        .get::<WallTileVisualMirror>(wall_tile_entity)
        .expect("wall tile visual mirror should be restored");
    assert_eq!(
        wall_tile_visual.state,
        WallTileStateMirror::Coating { progress: 61 }
    );
    assert!(world.get::<Sprite>(wall_tile_entity).is_some());
    assert_eq!(
        world.get::<Name>(wall_site_entity).map(Name::as_str),
        Some("WallConstructionSite")
    );
    assert_eq!(
        world.get::<Name>(wall_tile_entity).map(Name::as_str),
        Some("WallTile(7,8)")
    );

    let restored_sprite_count = world.query::<&Sprite>().iter(&world).count();
    rehydrate_construction_shells(&mut world, &BlueprintSpriteHandles::default());
    world.flush();
    assert_eq!(
        world.query::<&Sprite>().iter(&world).count(),
        restored_sprite_count
    );
}

#[test]
fn paused_visual_phase_rebuilds_construction_without_delivery_replay() {
    let mut app = App::new();
    app.init_resource::<Time<Virtual>>();
    app.init_resource::<LogicRunCount>();
    app.insert_resource(empty_material_icon_handles());
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.configure_sets(
        Update,
        (
            GameSystemSet::Logic.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
            GameSystemSet::Visual,
        )
            .chain(),
    );
    app.add_systems(Update, count_logic_run.in_set(GameSystemSet::Logic));
    app.add_systems(
        Update,
        (
            update_blueprint_visual_system,
            spawn_progress_bar_system,
            update_progress_bar_fill_system,
            spawn_material_display_system,
            material_delivery_vfx_system,
            manage_floor_curing_progress_bars_system,
            update_floor_curing_progress_bars_system,
            manage_wall_progress_bars_system,
            update_wall_progress_bars_system,
        )
            .chain()
            .in_set(GameSystemSet::Visual),
    );

    let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(3, 4), (4, 4)]);
    blueprint.progress = 0.25;
    blueprint.delivered_materials.insert(ResourceType::Wood, 1);
    let blueprint_entity = app
        .world_mut()
        .spawn((blueprint, Transform::from_xyz(3.0, 4.0, 0.0)))
        .id();

    let mut floor_site = floor_site(FloorConstructionPhase::Curing);
    floor_site.curing_remaining_secs = 42.0;
    let floor_site_entity = app
        .world_mut()
        .spawn((floor_site, Transform::from_xyz(5.0, 6.0, 0.0)))
        .id();

    let mut wall_site = WallConstructionSite::new(
        TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
        Vec2::ZERO,
        3,
    );
    wall_site.tiles_framed = 1;
    let wall_site_entity = app
        .world_mut()
        .spawn((wall_site, Transform::from_xyz(7.0, 8.0, 0.0)))
        .id();

    rehydrate_construction_shells(app.world_mut(), &BlueprintSpriteHandles::default());
    app.world_mut().flush();
    app.update();
    app.update();

    assert_eq!(app.world().resource::<LogicRunCount>().0, 0);
    let visual = app
        .world()
        .get::<BlueprintVisual>(blueprint_entity)
        .expect("load rehydration should attach BlueprintVisual before Visual runs");
    assert_eq!(visual.state, BlueprintState::Building);
    assert_eq!(
        visual.last_delivered.get(&ResourceType::Wood),
        Some(&1),
        "saved deliveries must not be treated as new deliveries"
    );
    assert!(
        app.world()
            .get::<BlueprintProgressBars>(blueprint_entity)
            .is_some(),
        "the paused Visual phase should create the progress bar"
    );
    assert_eq!(component_count::<MaterialIcon>(app.world_mut()), 1);
    assert_eq!(component_count::<MaterialCounter>(app.world_mut()), 1);
    assert_eq!(component_count::<DeliveryPopup>(app.world_mut()), 0);
    assert!(
        app.world()
            .get::<FloorSiteVisualState>(floor_site_entity)
            .is_some(),
        "the paused Visual phase must receive a rehydrated floor site mirror"
    );
    assert_eq!(
        component_count::<FloorCuringProgressBar>(app.world_mut()),
        2
    );
    assert!(
        app.world()
            .get::<WallSiteVisualState>(wall_site_entity)
            .is_some(),
        "the paused Visual phase must receive a rehydrated wall site mirror"
    );
    assert_eq!(
        component_count::<WallConstructionProgressBar>(app.world_mut()),
        2
    );
}

pub(super) fn floor_site(phase: FloorConstructionPhase) -> FloorConstructionSite {
    let mut site = FloorConstructionSite::new(
        TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
        Vec2::ZERO,
        1,
    );
    site.phase = phase;
    site
}

#[test]
fn construction_runtime_rehydrate_rebuilds_indexes_counters_and_curing_cache() {
    let mut world = World::new();
    world.insert_resource(TileSiteIndex::default());
    world.insert_resource(WorldMap::default());
    let original_obstacle_version = world.resource::<WorldMap>().obstacle_version;
    let original_obstacle_count = world
        .resource::<WorldMap>()
        .obstacles
        .iter()
        .filter(|blocked| **blocked)
        .count();

    let mut floor_site = FloorConstructionSite::new(
        TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
        Vec2::ZERO,
        2,
    );
    floor_site.phase = FloorConstructionPhase::Curing;
    floor_site.tiles_reinforced = 0;
    floor_site.tiles_poured = 0;
    let floor_site_entity = world.spawn(floor_site).id();
    for grid_pos in [(3, 4), (4, 4)] {
        let mut tile = FloorTileBlueprint::new(floor_site_entity, grid_pos);
        tile.state = FloorTileState::Complete;
        world.spawn(tile);
    }

    let mut wall_site = WallConstructionSite::new(
        TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
        Vec2::ZERO,
        2,
    );
    wall_site.tiles_framed = 0;
    let wall_site_entity = world.spawn(wall_site).id();
    for grid_pos in [(7, 8), (8, 8)] {
        let mut tile = WallTileBlueprint::new(wall_site_entity, grid_pos);
        tile.state = WallTileState::FramedProvisional;
        world.spawn(tile);
    }

    rehydrate_construction_runtime(&mut world);

    let index = world.resource::<TileSiteIndex>();
    assert_eq!(index.floor_tiles_by_site[&floor_site_entity].len(), 2);
    assert_eq!(index.wall_tiles_by_site[&wall_site_entity].len(), 2);
    let floor = world
        .get::<FloorConstructionSite>(floor_site_entity)
        .expect("floor site remains durable during curing");
    assert_eq!(floor.tiles_reinforced, 2);
    assert_eq!(floor.tiles_poured, 2);
    assert_eq!(floor.phase, FloorConstructionPhase::Curing);
    assert!(world.get::<CuringFootprint>(floor_site_entity).is_some());
    let wall = world
        .get::<WallConstructionSite>(wall_site_entity)
        .expect("wall site remains durable during coating");
    assert_eq!(wall.tiles_framed, 2);
    assert_eq!(wall.tiles_coated, 0);
    assert_eq!(wall.phase, WallConstructionPhase::Coating);
    assert_eq!(
        world.resource::<WorldMap>().obstacle_version,
        original_obstacle_version
    );
    assert_eq!(
        world
            .resource::<WorldMap>()
            .obstacles
            .iter()
            .filter(|blocked| **blocked)
            .count(),
        original_obstacle_count,
        "construction cache rehydration must not reserve the durable map again",
    );
}
