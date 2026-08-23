use super::*;

#[test]
fn durable_topology_validation_rejects_blueprint_and_wall_map_mismatches() {
    let mut blueprint_world = candidate_world();
    blueprint_world.spawn((
        Blueprint::new(BuildingType::Tank, vec![(4, 5)]),
        Transform::default(),
    ));
    assert!(
        validate_durable_topology_candidate(&blueprint_world)
            .unwrap_err()
            .contains("is not owned by it in WorldMap.buildings")
    );

    let mut wall_world = candidate_world();
    let site = wall_world
        .spawn(WallConstructionSite::new(
            hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            Vec2::ZERO,
            1,
        ))
        .id();
    wall_world.spawn(WallTileBlueprint::new(site, (6, 7)));
    assert!(
        validate_durable_topology_candidate(&wall_world)
            .unwrap_err()
            .contains("has WorldMap owner")
    );
}

#[test]
fn construction_validation_accepts_bridge_and_unspawned_wall_boundary() {
    let mut world = candidate_world();
    let bridge = world
        .spawn(Blueprint::new(BuildingType::Bridge, vec![(1, 2), (2, 2)]))
        .id();
    let wall_site = world
        .spawn(WallConstructionSite::new(
            hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            Vec2::ZERO,
            1,
        ))
        .id();
    let mut wall_tile = WallTileBlueprint::new(wall_site, (3, 3));
    wall_tile.state = WallTileState::FramedProvisional;
    world.spawn(wall_tile);
    {
        let mut map = world.resource_mut::<WorldMap>();
        map.set_building((1, 2), bridge);
        map.set_building((2, 2), bridge);
        map.set_building((3, 3), wall_site);
    }

    validate_construction_links(&world).unwrap();
    let map = world.resource::<WorldMap>();
    assert!(!map.has_raw_obstacle(1, 2));
    assert!(!map.bridged_tiles.contains(&(1, 2)));
}

#[test]
fn construction_validation_rejects_reverse_footprint_extras() {
    let mut blueprint_world = candidate_world();
    let blueprint = blueprint_world
        .spawn(Blueprint::new(BuildingType::Tank, vec![(1, 1)]))
        .id();
    {
        let mut map = blueprint_world.resource_mut::<WorldMap>();
        map.set_building((1, 1), blueprint);
        map.set_building((2, 1), blueprint);
    }
    assert!(
        validate_construction_links(&blueprint_world)
            .unwrap_err()
            .contains("outside occupied_grids")
    );

    let mut wall_world = candidate_world();
    let site = wall_world
        .spawn(WallConstructionSite::new(
            hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            Vec2::ZERO,
            1,
        ))
        .id();
    wall_world.spawn(WallTileBlueprint::new(site, (4, 4)));
    {
        let mut map = wall_world.resource_mut::<WorldMap>();
        map.set_building((4, 4), site);
        map.set_building((5, 4), site);
    }
    assert!(
        validate_construction_links(&wall_world)
            .unwrap_err()
            .contains("outside its tile footprint")
    );
}

#[test]
fn durable_topology_validation_rejects_stalled_construction_phase_mixtures() {
    let mut floor_world = candidate_world();
    let floor_site = floor_world
        .spawn(FloorConstructionSite::new(
            hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            Vec2::ZERO,
            2,
        ))
        .id();
    let mut early_floor = FloorTileBlueprint::new(floor_site, (1, 1));
    early_floor.state = FloorTileState::WaitingBones;
    floor_world.spawn(early_floor);
    let mut advanced_floor = FloorTileBlueprint::new(floor_site, (2, 1));
    advanced_floor.state = FloorTileState::WaitingMud;
    floor_world.spawn(advanced_floor);
    assert!(
        validate_durable_topology_candidate(&floor_world)
            .unwrap_err()
            .contains("tile states incompatible with Reinforcing")
    );

    let mut wall_world = candidate_world();
    let wall_site = wall_world
        .spawn(WallConstructionSite::new(
            hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            Vec2::ZERO,
            2,
        ))
        .id();
    let provisional_wall = wall_world
        .spawn((
            Building {
                kind: BuildingType::Wall,
                is_provisional: true,
            },
            ProvisionalWall::default(),
        ))
        .id();
    let mut advanced_wall = WallTileBlueprint::new(wall_site, (3, 3));
    advanced_wall.state = WallTileState::WaitingMud;
    advanced_wall.spawned_wall = Some(provisional_wall);
    wall_world.spawn(advanced_wall);
    wall_world.spawn(WallTileBlueprint::new(wall_site, (4, 3)));
    {
        let mut map = wall_world.resource_mut::<WorldMap>();
        map.set_building((3, 3), provisional_wall);
        map.set_building((4, 3), wall_site);
    }
    assert!(
        validate_durable_topology_candidate(&wall_world)
            .unwrap_err()
            .contains("tile states incompatible with Framing")
    );
}

#[test]
fn durable_topology_validation_rejects_non_provisional_spawned_wall() {
    let mut world = candidate_world();
    let site = world
        .spawn(WallConstructionSite::new(
            hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            Vec2::ZERO,
            1,
        ))
        .id();
    let permanent_wall = world
        .spawn(Building {
            kind: BuildingType::Wall,
            is_provisional: false,
        })
        .id();
    let mut tile = WallTileBlueprint::new(site, (5, 5));
    tile.state = WallTileState::FramedProvisional;
    tile.spawned_wall = Some(permanent_wall);
    world.spawn(tile);
    world
        .resource_mut::<WorldMap>()
        .set_building((5, 5), permanent_wall);

    assert!(
        validate_durable_topology_candidate(&world)
            .unwrap_err()
            .contains("is not a provisional Wall")
    );
}

#[test]
fn construction_validation_requires_completed_wall_to_be_permanent() {
    let mut valid_world = candidate_world();
    let site = valid_world
        .spawn({
            let mut site = WallConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                Vec2::ZERO,
                1,
            );
            site.phase = WallConstructionPhase::Coating;
            site
        })
        .id();
    let permanent_wall = valid_world
        .spawn(Building {
            kind: BuildingType::Wall,
            is_provisional: false,
        })
        .id();
    let mut tile = WallTileBlueprint::new(site, (7, 7));
    tile.state = WallTileState::Complete;
    tile.spawned_wall = Some(permanent_wall);
    valid_world.spawn(tile.clone());
    valid_world
        .resource_mut::<WorldMap>()
        .set_building((7, 7), permanent_wall);
    validate_construction_links(&valid_world).unwrap();

    let mut invalid_world = candidate_world();
    let site = invalid_world
        .spawn({
            let mut site = WallConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                Vec2::ZERO,
                1,
            );
            site.phase = WallConstructionPhase::Coating;
            site
        })
        .id();
    let provisional_wall = invalid_world
        .spawn((
            Building {
                kind: BuildingType::Wall,
                is_provisional: true,
            },
            ProvisionalWall::default(),
        ))
        .id();
    tile.parent_site = site;
    tile.spawned_wall = Some(provisional_wall);
    invalid_world.spawn(tile);
    invalid_world
        .resource_mut::<WorldMap>()
        .set_building((7, 7), provisional_wall);

    assert!(
        validate_construction_links(&invalid_world)
            .unwrap_err()
            .contains("is still provisional")
    );
}

#[test]
fn durable_topology_validation_rejects_missing_or_out_of_bounds_natural_obstacles() {
    let mut missing_position = candidate_world();
    missing_position.spawn((Tree, TreeVariant(0), Transform::default()));
    assert!(
        validate_durable_topology_candidate(&missing_position)
            .unwrap_err()
            .contains("has no ObstaclePosition")
    );

    let mut out_of_bounds = candidate_world();
    out_of_bounds.spawn((
        Rock,
        ObstaclePosition(MAP_WIDTH, MAP_HEIGHT - 1),
        Transform::default(),
    ));
    assert!(
        validate_durable_topology_candidate(&out_of_bounds)
            .unwrap_err()
            .contains("out-of-bounds ObstaclePosition")
    );
}

#[test]
fn durable_topology_validation_rejects_orphan_construction_tile() {
    let mut world = candidate_world();
    let missing_site = world.spawn_empty().id();
    world.despawn(missing_site);
    world.spawn(FloorTileBlueprint::new(missing_site, (4, 5)));

    assert!(
        validate_durable_topology_candidate(&world)
            .unwrap_err()
            .contains("references missing parent site")
    );
}
