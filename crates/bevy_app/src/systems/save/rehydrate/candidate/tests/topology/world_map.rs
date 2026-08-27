use super::*;

#[test]
fn durable_topology_validation_rejects_invalid_world_map_shape() {
    let mut world = candidate_world();
    world.resource_mut::<WorldMap>().obstacles.pop();

    assert!(
        validate_durable_topology_candidate(&world)
            .unwrap_err()
            .contains("WorldMap.obstacles has length")
    );
}

#[test]
fn world_map_validation_accepts_legacy_completed_floor_without_floor_lookup() {
    let mut world = candidate_world();
    let grid = (6, 7);
    let floor = world
        .spawn((
            Building {
                kind: BuildingType::Floor,
                is_provisional: false,
            },
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
        ))
        .id();
    world.resource_mut::<WorldMap>().set_building(grid, floor);

    validate_world_map_candidate(&world).unwrap();
    assert!(world.resource::<WorldMap>().floors.is_empty());
}

#[test]
fn world_map_validation_rejects_provisional_or_noncanonical_completed_floor() {
    let mut provisional = candidate_world();
    provisional.spawn((
        Building {
            kind: BuildingType::Floor,
            is_provisional: true,
        },
        Transform::default(),
    ));
    assert!(
        validate_world_map_candidate(&provisional)
            .unwrap_err()
            .contains("cannot be provisional")
    );

    let mut noncanonical = candidate_world();
    noncanonical.spawn((
        Building {
            kind: BuildingType::Floor,
            is_provisional: false,
        },
        Transform::from_xyz(1.0, 1.0, 0.0),
    ));
    assert!(
        validate_world_map_candidate(&noncanonical)
            .unwrap_err()
            .contains("non-canonical position")
    );
}

#[test]
fn world_map_validation_rejects_wrong_completed_floor_lookup() {
    let mut world = candidate_world();
    let actual_grid = (8, 9);
    let floor = world
        .spawn((
            Building {
                kind: BuildingType::Floor,
                is_provisional: false,
            },
            Transform::from_translation(
                WorldMap::grid_to_world(actual_grid.0, actual_grid.1).extend(0.0),
            ),
        ))
        .id();
    world
        .resource_mut::<WorldMap>()
        .set_floor((actual_grid.0 + 1, actual_grid.1), floor);

    let error = validate_world_map_candidate(&world).unwrap_err();

    assert!(error.contains("is not the completed Floor at that grid"));
}

#[test]
fn durable_topology_validation_accepts_sparse_tile_anchors() {
    let world = candidate_world();

    validate_durable_topology_candidate(&world).unwrap();
}

#[test]
fn durable_topology_validation_accepts_complete_unique_tile_anchors() {
    let mut world = candidate_world();
    let tile_count = (MAP_WIDTH * MAP_HEIGHT) as usize;
    let anchors: Vec<_> = (0..tile_count)
        .map(|_| world.spawn((Tile, Transform::default())).id())
        .collect();
    world.resource_mut::<WorldMap>().tile_entities = anchors.into_iter().map(Some).collect();

    validate_durable_topology_candidate(&world).unwrap();
}

#[test]
fn legacy_tile_anchors_normalize_to_sparse_shape() {
    let mut world = candidate_world();
    let tile_count = (MAP_WIDTH * MAP_HEIGHT) as usize;
    let anchors: Vec<_> = (0..tile_count)
        .map(|_| world.spawn((Tile, Transform::default())).id())
        .collect();
    world.resource_mut::<WorldMap>().tile_entities = anchors.into_iter().map(Some).collect();

    crate::systems::save::rehydrate::normalize_legacy_tile_anchors(&mut world);

    assert!(
        world
            .resource::<WorldMap>()
            .tile_entities
            .iter()
            .all(Option::is_none)
    );
    assert_eq!(
        world
            .query_filtered::<Entity, With<Tile>>()
            .iter(&world)
            .count(),
        0
    );
}

#[test]
fn durable_topology_validation_rejects_mixed_tile_anchor_shape() {
    let mut world = candidate_world();
    let anchor = world.spawn((Tile, Transform::default())).id();
    world.resource_mut::<WorldMap>().tile_entities[0] = Some(anchor);

    assert!(
        validate_durable_topology_candidate(&world)
            .unwrap_err()
            .contains("mixed sparse/legacy shape")
    );
}

#[test]
fn durable_topology_validation_rejects_orphan_tile_entity() {
    let mut world = candidate_world();
    world.spawn((Tile, Transform::default()));

    assert!(
        validate_durable_topology_candidate(&world)
            .unwrap_err()
            .contains("orphan Tile entities")
    );
}
